use std::time::{Duration, Instant};

use cockpit_agent::{
    HumanAgentDriver, HumanBackend, HumanTurnContext, LiveTickMode, LocalMcpServer, RuleAgent,
};
use cockpit_scenario::load_scenario;
use cockpit_world::{EventEnvelope, EventPayload, Simulation, TICK_PHASE_ORDER, TickPhase};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub scenario_path: String,
    pub ticks: u64,
    pub active_entities: u64,
    pub events_per_minute: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkReport {
    pub scenario_id: String,
    pub scenario_hash: String,
    pub seed: u64,
    pub ticks: u64,
    pub active_entities: u64,
    pub events_per_minute: u64,
    pub average_tick_ms: f64,
    pub p50_tick_ms: f64,
    pub p95_tick_ms: f64,
    pub p99_tick_ms: f64,
    pub peak_tick_ms: f64,
    pub phase_metrics: Vec<PhaseBenchmarkMetrics>,
    pub recording_bytes: usize,
    pub synthetic_event_count: u64,
    pub synthetic_workload_hash: String,
    /// Peak resident set size in bytes, when the platform exposes it without
    /// extra dependencies; `None` means it was not captured on this OS.
    pub peak_memory_bytes: Option<u64>,
    /// How `peak_memory_bytes` was obtained (or why it is absent).
    pub peak_memory_source: String,
    /// Target triple the benchmark ran on, for cross-platform acceptance.
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseBenchmarkMetrics {
    pub phase: TickPhase,
    pub samples: u64,
    pub average_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub peak_ms: f64,
}

#[derive(Debug, Clone)]
pub struct LiveBenchmarkConfig {
    pub scenario_path: String,
    pub human_count: usize,
    pub ticks: u64,
    pub backend_delay_ms: u64,
    pub failing_human_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveBenchmarkReport {
    pub human_count: usize,
    pub requested_ticks: u64,
    pub backend_delay_ms: u64,
    pub failing_human_id: Option<String>,
    pub strict: LiveBenchmarkModeMetrics,
    pub best_effort: LiveBenchmarkModeMetrics,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveBenchmarkModeMetrics {
    pub mode: LiveTickMode,
    pub samples: u64,
    pub committed_ticks: u64,
    pub failed_ticks: u64,
    pub failed_human_turns: u64,
    pub average_tick_ms: f64,
    pub p95_tick_ms: f64,
    pub peak_tick_ms: f64,
}

/// Measure serialized N-human live ticks with a deterministic backend shim.
/// This is an offline capacity benchmark, not a model-quality score: the shim
/// makes backend latency and one failing human explicit so strict failure
/// propagation can be compared with best-effort commit semantics.
pub async fn run_live(config: LiveBenchmarkConfig) -> anyhow::Result<LiveBenchmarkReport> {
    if config.human_count == 0 || config.ticks == 0 {
        anyhow::bail!("live benchmark requires at least one human and one tick");
    }
    let mut scenario = load_scenario(&config.scenario_path)?;
    let template = scenario
        .humans
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("scenario has no humans"))?;
    scenario.humans = (0..config.human_count)
        .map(|index| {
            let mut human = template.clone();
            human.id = format!("benchmark-human-{index}");
            human
        })
        .collect();
    let failing_human_id = config.failing_human_id.clone();
    let strict = run_live_mode(
        &scenario,
        config.ticks,
        config.backend_delay_ms,
        failing_human_id.clone(),
        LiveTickMode::Strict,
    )
    .await;
    let best_effort = run_live_mode(
        &scenario,
        config.ticks,
        config.backend_delay_ms,
        failing_human_id.clone(),
        LiveTickMode::BestEffort,
    )
    .await;
    Ok(LiveBenchmarkReport {
        human_count: config.human_count,
        requested_ticks: config.ticks,
        backend_delay_ms: config.backend_delay_ms,
        failing_human_id,
        strict,
        best_effort,
    })
}

async fn run_live_mode(
    scenario: &cockpit_world::SimulationScenario,
    ticks: u64,
    backend_delay_ms: u64,
    failing_human_id: Option<String>,
    mode: LiveTickMode,
) -> LiveBenchmarkModeMetrics {
    let mut simulation = Simulation::new(format!("live-benchmark-{mode:?}"), scenario.clone());
    let _ = simulation.start();
    let mut driver = HumanAgentDriver::new();
    let mut backend = BenchmarkBackend {
        delay_ms: backend_delay_ms,
        failing_human_id,
    };
    let mut server = LocalMcpServer::default();
    let mut samples = Vec::with_capacity(ticks as usize);
    let mut committed_ticks = 0;
    let mut failed_ticks = 0;
    let mut failed_human_turns = 0;
    for _ in 0..ticks {
        let started = Instant::now();
        match driver
            .step_with_tools_mode(&mut simulation, &mut backend, &mut server, mode)
            .await
        {
            Ok((_step, evidence)) => {
                committed_ticks += 1;
                failed_human_turns += evidence
                    .iter()
                    .filter(|turn| {
                        !matches!(
                            turn.disposition,
                            cockpit_agent::HumanTurnDisposition::Completed
                        )
                    })
                    .count() as u64;
            }
            Err(_error) => {
                failed_ticks += 1;
                failed_human_turns += 1;
                samples.push(started.elapsed());
                break;
            }
        }
        samples.push(started.elapsed());
    }
    let mut nanos = samples.iter().map(Duration::as_nanos).collect::<Vec<_>>();
    nanos.sort_unstable();
    let percentile = |percent: usize| -> f64 {
        let index =
            ((nanos.len().saturating_sub(1)) * percent / 100).min(nanos.len().saturating_sub(1));
        nanos.get(index).copied().unwrap_or_default() as f64 / 1_000_000.0
    };
    LiveBenchmarkModeMetrics {
        mode,
        samples: nanos.len() as u64,
        committed_ticks,
        failed_ticks,
        failed_human_turns,
        average_tick_ms: if nanos.is_empty() {
            0.0
        } else {
            nanos.iter().sum::<u128>() as f64 / nanos.len() as f64 / 1_000_000.0
        },
        p95_tick_ms: percentile(95),
        peak_tick_ms: nanos.last().copied().unwrap_or_default() as f64 / 1_000_000.0,
    }
}

struct BenchmarkBackend {
    delay_ms: u64,
    failing_human_id: Option<String>,
}

impl HumanBackend for BenchmarkBackend {
    async fn run_turn(&mut self, context: &HumanTurnContext) -> Result<String, String> {
        tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        if self
            .failing_human_id
            .as_deref()
            .is_some_and(|human_id| human_id == context.human_id)
        {
            return Err("deterministic benchmark backend failure".to_string());
        }
        Ok(r#"{"type":"final","utterance":null,"internalStateDelta":{"stress":null,"attention":null},"narrative":"benchmark"}"#.to_string())
    }
}

pub fn run(config: BenchmarkConfig) -> anyhow::Result<BenchmarkReport> {
    let scenario = load_scenario(&config.scenario_path)?;
    let mut samples = Vec::with_capacity(config.ticks as usize);
    let mut phase_samples = vec![Vec::with_capacity(config.ticks as usize); TICK_PHASE_ORDER.len()];
    let mut workload_hasher = Sha256::new();
    let mut synthetic_event_count = 0_u64;
    let mut simulation = Simulation::new("benchmark-run", scenario.clone());
    simulation.start()?;
    let mut agent = RuleAgent::default();
    let mut server = LocalMcpServer::default();
    let mut recording = cockpit_recording::Recording::new("benchmark-run", &scenario);

    for _ in 0..config.ticks {
        let tick_started = Instant::now();
        let mut step = agent.step(&mut simulation, &mut server)?;
        for timing in simulation.last_phase_timings() {
            let index = TICK_PHASE_ORDER
                .iter()
                .position(|phase| *phase == timing.phase)
                .expect("timing phase must be in the fixed phase order");
            phase_samples[index].push(Duration::from_nanos(
                timing.elapsed_nanos.min(u64::MAX as u128) as u64,
            ));
        }
        let synthetic_events = synthetic_event_work(
            simulation.snapshot.tick,
            config.active_entities,
            config.events_per_minute,
        );
        for event in synthetic_events {
            workload_hasher.update(serde_json::to_vec(&event)?);
            step.events.push(event);
            synthetic_event_count += 1;
        }
        let elapsed = tick_started.elapsed();
        samples.push(elapsed);
        recording.push(step);
    }
    let mut nanos: Vec<u128> = samples.iter().map(Duration::as_nanos).collect();
    nanos.sort_unstable();
    let average_tick_ms = nanos.iter().sum::<u128>() as f64 / nanos.len() as f64 / 1_000_000.0;
    let percentile = |percent: usize| -> f64 {
        let index = ((nanos.len() - 1) * percent / 100).min(nanos.len() - 1);
        nanos[index] as f64 / 1_000_000.0
    };
    let recording_bytes = serde_json::to_vec(&recording)?.len();
    let peak_memory_bytes = crate::memory::peak_resident_bytes();

    Ok(BenchmarkReport {
        scenario_id: scenario.id,
        scenario_hash: scenario.scenario_hash,
        seed: scenario.seed,
        ticks: config.ticks,
        active_entities: config.active_entities,
        events_per_minute: config.events_per_minute,
        average_tick_ms,
        p50_tick_ms: percentile(50),
        p95_tick_ms: percentile(95),
        p99_tick_ms: percentile(99),
        peak_tick_ms: nanos.last().copied().unwrap_or_default() as f64 / 1_000_000.0,
        phase_metrics: TICK_PHASE_ORDER
            .iter()
            .copied()
            .zip(phase_samples)
            .map(|(phase, samples)| phase_metrics(phase, &samples))
            .collect(),
        recording_bytes,
        synthetic_event_count,
        synthetic_workload_hash: format!("sha256:{:x}", workload_hasher.finalize()),
        peak_memory_bytes,
        peak_memory_source: crate::memory::peak_memory_source().to_string(),
        target: option_env!("COCKPIT_TARGET")
            .unwrap_or("unknown-target")
            .to_string(),
    })
}

fn phase_metrics(phase: TickPhase, samples: &[Duration]) -> PhaseBenchmarkMetrics {
    let mut nanos = samples.iter().map(Duration::as_nanos).collect::<Vec<_>>();
    nanos.sort_unstable();
    let percentile = |percent: usize| -> f64 {
        let index =
            ((nanos.len().saturating_sub(1)) * percent / 100).min(nanos.len().saturating_sub(1));
        nanos.get(index).copied().unwrap_or_default() as f64 / 1_000_000.0
    };
    PhaseBenchmarkMetrics {
        phase,
        samples: nanos.len() as u64,
        average_ms: if nanos.is_empty() {
            0.0
        } else {
            nanos.iter().sum::<u128>() as f64 / nanos.len() as f64 / 1_000_000.0
        },
        p95_ms: percentile(95),
        p99_ms: percentile(99),
        peak_ms: nanos.last().copied().unwrap_or_default() as f64 / 1_000_000.0,
    }
}

fn synthetic_event_work(
    tick: u64,
    active_entities: u64,
    events_per_minute: u64,
) -> Vec<EventEnvelope> {
    let events_this_tick = (events_per_minute / 60).max(1);
    let mut events = Vec::with_capacity(events_this_tick as usize);
    for sequence in 0..events_this_tick {
        let entity = (tick.wrapping_mul(events_this_tick) + sequence) % active_entities.max(1);
        events.push(EventEnvelope {
            event_id: format!("benchmark-{tick}-{sequence}"),
            event_type: "SyntheticWorkloadEvent".to_string(),
            run_id: "benchmark-run".to_string(),
            tick,
            source: "benchmark".to_string(),
            priority: 0,
            sequence,
            correlation_id: format!("benchmark-{tick}"),
            payload: EventPayload {
                message: "synthetic capacity workload".to_string(),
                target: Some(format!("entity-{entity}")),
                value: Some(entity as f64),
                error_code: None,
            },
        });
    }
    events
}
