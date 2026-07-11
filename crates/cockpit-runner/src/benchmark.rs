use std::time::{Duration, Instant};

use cockpit_agent_runtime::{LocalMcpServer, RuleAgent};
use cockpit_scenario::load_scenario;
use cockpit_simulation_core::Simulation;
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
    pub recording_bytes: usize,
    pub synthetic_workload_hash: String,
}

pub fn run(config: BenchmarkConfig) -> anyhow::Result<BenchmarkReport> {
    let scenario = load_scenario(&config.scenario_path)?;
    let mut samples = Vec::with_capacity(config.ticks as usize);
    let mut workload_hasher = Sha256::new();
    let mut simulation = Simulation::new("benchmark-run", scenario.clone());
    simulation.start()?;
    let mut agent = RuleAgent::default();
    let mut server = LocalMcpServer::default();
    let mut recording = cockpit_recording::Recording::new("benchmark-run", &scenario);

    for _ in 0..config.ticks {
        let tick_started = Instant::now();
        let step = agent.step(&mut simulation, &mut server)?;
        let synthetic_events = synthetic_event_work(
            simulation.snapshot.tick,
            config.active_entities,
            config.events_per_minute,
        );
        workload_hasher.update(&synthetic_events);
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
        recording_bytes,
        synthetic_workload_hash: format!("sha256:{:x}", workload_hasher.finalize()),
    })
}

fn synthetic_event_work(tick: u64, active_entities: u64, events_per_minute: u64) -> Vec<u8> {
    let events_this_tick = (events_per_minute / 60).max(1);
    let mut bytes = Vec::with_capacity((events_this_tick * 32) as usize);
    for sequence in 0..events_this_tick {
        let entity = (tick.wrapping_mul(events_this_tick) + sequence) % active_entities.max(1);
        bytes.extend_from_slice(format!("{tick}:{sequence}:entity-{entity};").as_bytes());
    }
    bytes
}
