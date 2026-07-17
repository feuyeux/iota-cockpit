//! Repeatable offline benchmark execution.
//!
//! This module deliberately labels its backend as a RuleAgent baseline. It is
//! useful for scenario and evaluator regression, but callers must use a live
//! backend run to make claims about an LLM.

use cockpit_evaluation::{
    AggregateEvaluationResult, BenchmarkSplit, EvaluationResult, ReleaseGate, ReleaseGateResult,
    aggregate,
};
use cockpit_recording::{RunProvenance, run_rule_agent_recording};
use cockpit_scenario::load_scenario;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct EvaluationConfig {
    pub scenario_path: String,
    pub runs: u64,
    pub ticks: Option<u64>,
    /// Deterministically moves every fault by at most this many ticks in either
    /// direction. Zero preserves the canonical scenario exactly.
    pub fault_jitter_ticks: u64,
    pub influence_jitter_ticks: u64,
    /// Deterministically remove the primary grant in selected variants. This
    /// exercises least-privilege and rejected-action behavior.
    pub capability_dropout_percent: u8,
    pub suite_id: String,
    pub suite_version: String,
    pub split: BenchmarkSplit,
    pub release_gate: Option<ReleaseGate>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentProvenance {
    pub suite_id: String,
    pub suite_version: String,
    pub split: BenchmarkSplit,
    pub backend: &'static str,
    pub runner_version: String,
    pub variant_dimensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationTrial {
    pub trial: u64,
    pub seed: u64,
    pub scenario_hash: String,
    pub fault_tick_offset: i64,
    pub influence_tick_offset: i64,
    pub capability_dropped: bool,
    pub result: EvaluationResult,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationReport {
    pub backend: &'static str,
    pub scenario_id: String,
    pub scenario_hash: String,
    pub fault_jitter_ticks: u64,
    pub trials: Vec<EvaluationTrial>,
    pub aggregate: AggregateEvaluationResult,
    pub release_gate: Option<ReleaseGateResult>,
    pub provenance: ExperimentProvenance,
}

pub fn run(config: EvaluationConfig) -> anyhow::Result<EvaluationReport> {
    let canonical = load_scenario(&config.scenario_path)?;
    let runs = config.runs.max(1);
    let ticks = config
        .ticks
        .unwrap_or(canonical.shutdown_deadline_ticks.saturating_add(1));
    let mut trials = Vec::with_capacity(runs as usize);
    let mut results = Vec::with_capacity(runs as usize);

    for trial in 0..runs {
        let mut scenario = canonical.clone();
        let offset = fault_offset(canonical.seed, trial, config.fault_jitter_ticks);
        let influence_offset = fault_offset(
            canonical.seed ^ 0x5bf0_3635,
            trial,
            config.influence_jitter_ticks,
        );
        scenario.seed = canonical.seed.wrapping_add(trial);
        for fault in &mut scenario.faults {
            fault.at_tick = fault.at_tick.saturating_add_signed(offset);
        }
        for influence in &mut scenario.influences {
            match &mut influence.schedule {
                cockpit_simulation_core::InfluenceSchedule::AtTick { tick } => {
                    *tick = tick.saturating_add_signed(influence_offset)
                }
                cockpit_simulation_core::InfluenceSchedule::Every { start, .. } => {
                    *start = start.saturating_add_signed(influence_offset)
                }
            }
        }
        let capability_dropped = config.capability_dropout_percent > 0
            && (splitmix64(canonical.seed ^ trial) % 100)
                < config.capability_dropout_percent as u64;
        if capability_dropped {
            scenario.agent.capabilities.clear();
            if let Some(agent) = scenario.agents.first_mut() {
                agent.capabilities.clear();
            }
            if let Some(human) = scenario.humans.first_mut() {
                human.action_capabilities.clear();
            }
        }
        scenario.scenario_hash = variant_hash(&scenario)?;
        let mut recording = run_rule_agent_recording(
            format!("evaluation-{}-{trial}", scenario.id),
            scenario.clone(),
            ticks,
        )?;
        recording.provenance = RunProvenance {
            suite_id: Some(config.suite_id.clone()),
            suite_version: Some(config.suite_version.clone()),
            split: Some(format!("{:?}", config.split)),
            backend: Some("rule-agent-baseline".to_string()),
            variant_hash: Some(scenario.scenario_hash.clone()),
            ..RunProvenance::default()
        };
        let result = cockpit_evaluation::evaluate_scenario(&recording, &scenario);
        results.push(result.clone());
        trials.push(EvaluationTrial {
            trial,
            seed: scenario.seed,
            scenario_hash: scenario.scenario_hash.clone(),
            fault_tick_offset: offset,
            influence_tick_offset: influence_offset,
            capability_dropped,
            result,
        });
    }

    let aggregate = aggregate(&results);
    Ok(EvaluationReport {
        backend: "rule-agent-baseline",
        scenario_id: canonical.id,
        scenario_hash: canonical.scenario_hash,
        fault_jitter_ticks: config.fault_jitter_ticks,
        trials,
        release_gate: config
            .release_gate
            .as_ref()
            .map(|gate| gate.evaluate(&aggregate)),
        aggregate,
        provenance: ExperimentProvenance {
            suite_id: config.suite_id,
            suite_version: config.suite_version,
            split: config.split,
            backend: "rule-agent-baseline",
            runner_version: env!("CARGO_PKG_VERSION").to_string(),
            variant_dimensions: vec![
                "faultTiming".to_string(),
                "influenceTiming".to_string(),
                "capabilityDropout".to_string(),
            ],
        },
    })
}

fn variant_hash(scenario: &cockpit_simulation_core::SimulationScenario) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(scenario)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn fault_offset(seed: u64, trial: u64, jitter: u64) -> i64 {
    if jitter == 0 {
        return 0;
    }
    // SplitMix64 is deterministic, dependency-free, and adequate for choosing
    // benchmark variants. It is not used for safety or cryptographic work.
    let value = splitmix64(seed.wrapping_add(trial.wrapping_mul(0x9e37_79b9_7f4a_7c15)));
    let width = jitter.saturating_mul(2).saturating_add(1);
    value.wrapping_rem(width) as i64 - jitter as i64
}

fn splitmix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    value
}

#[cfg(test)]
mod tests {
    use super::{EvaluationConfig, fault_offset, run};
    use cockpit_evaluation::BenchmarkSplit;

    #[test]
    fn variants_are_deterministic_and_reported_as_a_baseline() {
        assert_eq!(fault_offset(42, 0, 0), 0);
        assert_eq!(fault_offset(42, 3, 2), fault_offset(42, 3, 2));
        let report = run(EvaluationConfig {
            scenario_path: "../../scenarios/smoke-in-cockpit.yaml".to_string(),
            runs: 3,
            ticks: Some(36),
            fault_jitter_ticks: 2,
            influence_jitter_ticks: 1,
            capability_dropout_percent: 0,
            suite_id: "development".to_string(),
            suite_version: "1".to_string(),
            split: BenchmarkSplit::Development,
            release_gate: None,
        })
        .expect("evaluation runs");
        assert_eq!(report.backend, "rule-agent-baseline");
        assert_eq!(report.aggregate.runs, 3);
        assert_eq!(report.trials.len(), 3);
        assert!(
            report
                .trials
                .iter()
                .all(|trial| trial.scenario_hash != report.scenario_hash)
        );
        assert_ne!(
            report.trials[0].scenario_hash,
            report.trials[1].scenario_hash
        );
        assert_eq!(report.provenance.split, BenchmarkSplit::Development);
        assert!(report.aggregate.p95_first_applied_action_tick.is_some());
    }
}
