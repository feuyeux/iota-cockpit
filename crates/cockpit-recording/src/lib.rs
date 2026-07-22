use std::collections::BTreeMap;

use cockpit_agent::{LocalMcpServer, RuleAgent};
use cockpit_world::{
    ActionResult, PluginFailureRecord, ScriptedAgent,
    action::{ActionRequest, ActionStatus},
    clock::ClockConfig,
    error::SimulationResult,
    event::{EventEnvelope, ToolCallTrace},
    simulation::{Simulation, SimulationScenario, StepRecord},
    state_patch::StateDiff,
};
use serde::{Deserialize, Serialize};

pub mod diff;
pub mod queue;
pub mod replay;
pub mod replica;
pub mod store;

/// Current recording schema version understood by this build. Version 2 adds
/// an optional durable world-plus-agent checkpoint for live restart recovery.
pub const CURRENT_SCHEMA_VERSION: u32 = 2;
/// Current runtime contract version. Version 8 adds persisted live tick
/// failure semantics, so best-effort recordings replay each skipped human
/// through the same transaction boundary.
pub const CURRENT_RUNTIME_CONTRACT_VERSION: u32 = 8;
/// Current world-model version. Version 8 adds humidity-limited evaporative
/// heat loss to two-node occupant thermoregulation; replay rejects prior
/// physiology behavior rather than claiming deterministic equivalence.
pub const CURRENT_WORLD_MODEL_VERSION: u32 = 8;

pub use diff::{RecordingDiff, RecordingMetrics, TickDiff, diff_recordings};
pub use queue::{
    AsyncRecordingSink, RecordingQueue, RecordingQueueHealth, RecordingQueueOutcome,
    RecordingQueuePolicy,
};
pub use replay::replay_recording;
pub use replica::{AuthenticatedReplicaStore, PayloadRestoreEvidence};
pub use store::{
    PayloadGcReport, PayloadStore, RecordingStore, RecordingStoreError,
    serialize_redacted_recording,
};

/// A redacted, time-windowed projection of durable evidence for operator
/// reconnect. It deliberately excludes snapshots and hidden model material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RecordedAuditEvent {
    WorldEvent {
        tick: u64,
        event: EventEnvelope,
    },
    ToolCall {
        tick: u64,
        trace: ToolCallTrace,
    },
    ActionResult {
        tick: u64,
        result: ActionResult,
    },
    PluginFailure {
        tick: u64,
        failure: PluginFailureRecord,
    },
    HumanTurn {
        tick: u64,
        backend: String,
        evidence: cockpit_agent::HumanTurnEvidence,
    },
    Error {
        tick: u64,
        message: String,
    },
}

/// A deterministic, recording-global position for redacted audit evidence.
/// Unlike simulator event cursors, this survives sidecar restarts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequencedRecordedAuditEvent {
    pub sequence: u64,
    #[serde(flatten)]
    pub event: RecordedAuditEvent,
}

/// A bounded durable-audit page. The store computes continuation against the
/// active recording generation, so callers do not need to load an entire long
/// recording merely to recover a UI cursor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedAuditPage {
    pub events: Vec<SequencedRecordedAuditEvent>,
    pub total_events: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub next_sequence: Option<u64>,
    pub truncated: bool,
}

/// Query parameters for one bounded durable-audit page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedAuditPageRequest {
    pub start_tick: u64,
    pub end_tick: u64,
    pub offset: Option<usize>,
    pub limit: usize,
    pub after_sequence: Option<u64>,
    pub tail_limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub schema_version: u32,
    pub runtime_contract_version: u32,
    pub world_model_version: u32,
    pub application_commit: String,
    pub plugin_hashes: Vec<String>,
    pub run_id: String,
    pub scenario_id: String,
    pub scenario_hash: String,
    pub seed: u64,
    pub clock: ClockConfig,
    pub ticks: Vec<StepRecord>,
    /// Per-tick human decisions for a live run, in driver order. Free-form
    /// narrative and utterance text is redacted; typed actions and state deltas
    /// remain available for deterministic replay without another model call.
    #[serde(default)]
    pub human_turns: Vec<Vec<cockpit_agent::HumanTurnEvidence>>,
    #[serde(default)]
    pub provenance: RunProvenance,
    /// Latest restartable world-plus-agent control-plane checkpoint for live
    /// runs. Evaluators may inspect it but never mutate it.
    #[serde(default)]
    pub open_world_checkpoint: Option<cockpit_agent::OpenWorldCheckpoint>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProvenance {
    pub suite_id: Option<String>,
    pub suite_version: Option<String>,
    pub split: Option<String>,
    pub backend: Option<String>,
    pub variant_hash: Option<String>,
    pub prompt_template_hash: Option<String>,
    pub skill_hash: Option<String>,
    /// Canonical hash of the deterministic baseline policy, when a RuleAgent
    /// produced the recording. This distinguishes policy changes from world
    /// model or scenario changes during replay analysis.
    pub rule_policy_hash: Option<String>,
    /// Explicit live human failure semantics used to produce the recording.
    /// Missing values in older strict recordings are interpreted as `Strict`.
    #[serde(default)]
    pub live_tick_mode: Option<cockpit_agent::LiveTickMode>,
}

impl Recording {
    pub fn new(run_id: impl Into<String>, scenario: &SimulationScenario) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            runtime_contract_version: CURRENT_RUNTIME_CONTRACT_VERSION,
            world_model_version: CURRENT_WORLD_MODEL_VERSION,
            application_commit: option_env!("COCKPIT_APPLICATION_COMMIT")
                .unwrap_or("unknown")
                .to_string(),
            plugin_hashes: Vec::new(),
            run_id: run_id.into(),
            scenario_id: scenario.id.clone(),
            scenario_hash: scenario.scenario_hash.clone(),
            seed: scenario.seed,
            clock: scenario.clock,
            ticks: Vec::new(),
            human_turns: Vec::new(),
            provenance: RunProvenance::default(),
            open_world_checkpoint: None,
        }
    }

    pub fn push(&mut self, step: StepRecord) {
        self.ticks.push(step);
    }

    /// Record one tick's backend-authored human decisions (live runs only).
    pub fn push_human_turns(&mut self, turns: Vec<cockpit_agent::HumanTurnEvidence>) {
        self.human_turns.push(turns);
    }

    pub fn final_snapshot_hash(&self) -> Option<&str> {
        self.ticks.last().map(|tick| tick.snapshot_hash.as_str())
    }

    pub fn recorded_actions_by_tick(&self) -> BTreeMap<u64, Vec<ActionRequest>> {
        let mut actions = BTreeMap::new();
        for tick in &self.ticks {
            for result in &tick.action_results {
                if result.status == ActionStatus::Applied {
                    actions
                        .entry(result.tick)
                        .or_insert_with(Vec::new)
                        .push(result.request.clone());
                }
            }
        }
        actions
    }

    pub fn recorded_state_diffs_by_tick(&self) -> BTreeMap<u64, Vec<StateDiff>> {
        self.ticks
            .iter()
            .map(|tick| (tick.tick, tick.state_diffs.clone()))
            .collect()
    }

    fn audit_window(&self, start_tick: u64, end_tick: u64) -> Vec<SequencedRecordedAuditEvent> {
        self.ticks
            .iter()
            .flat_map(|step| {
                let backend = self
                    .provenance
                    .backend
                    .clone()
                    .unwrap_or_else(|| "recorded".to_string());
                let human_turns = self
                    .human_turns
                    .get(step.tick as usize)
                    .into_iter()
                    .flatten()
                    .cloned()
                    .map(move |evidence| RecordedAuditEvent::HumanTurn {
                        tick: step.tick,
                        backend: backend.clone(),
                        evidence,
                    });
                step.events
                    .iter()
                    .cloned()
                    .map(|event| RecordedAuditEvent::WorldEvent {
                        tick: step.tick,
                        event,
                    })
                    .chain(step.tool_calls.iter().cloned().map(|trace| {
                        RecordedAuditEvent::ToolCall {
                            tick: step.tick,
                            trace,
                        }
                    }))
                    .chain(step.action_results.iter().cloned().map(|result| {
                        RecordedAuditEvent::ActionResult {
                            tick: step.tick,
                            result,
                        }
                    }))
                    .chain(step.plugin_failures.iter().cloned().map(|failure| {
                        RecordedAuditEvent::PluginFailure {
                            tick: step.tick,
                            failure,
                        }
                    }))
                    .chain(
                        step.errors
                            .iter()
                            .cloned()
                            .map(|message| RecordedAuditEvent::Error {
                                tick: step.tick,
                                message,
                            }),
                    )
                    .chain(human_turns)
                    .collect::<Vec<_>>()
            })
            .enumerate()
            .filter_map(|(index, event)| {
                let tick = match &event {
                    RecordedAuditEvent::WorldEvent { tick, .. }
                    | RecordedAuditEvent::ToolCall { tick, .. }
                    | RecordedAuditEvent::ActionResult { tick, .. }
                    | RecordedAuditEvent::PluginFailure { tick, .. }
                    | RecordedAuditEvent::HumanTurn { tick, .. }
                    | RecordedAuditEvent::Error { tick, .. } => *tick,
                };
                (start_tick..=end_tick)
                    .contains(&tick)
                    .then_some(SequencedRecordedAuditEvent {
                        sequence: index as u64 + 1,
                        event,
                    })
            })
            .collect()
    }
}

pub fn run_scripted_recording(
    run_id: impl Into<String>,
    scenario: SimulationScenario,
    ticks: u64,
) -> SimulationResult<Recording> {
    let run_id = run_id.into();
    let mut simulation = Simulation::new(run_id.clone(), scenario.clone());
    simulation.start()?;
    let mut recording = Recording::new(run_id, &scenario);
    let mut agent = ScriptedAgent::default();
    for _ in 0..ticks {
        let step = simulation.step_with_scripted_agent(&mut agent)?;
        recording.push(step);
    }
    Ok(recording)
}

pub fn run_rule_agent_recording(
    run_id: impl Into<String>,
    scenario: SimulationScenario,
    ticks: u64,
) -> SimulationResult<Recording> {
    let run_id = run_id.into();
    let mut simulation = Simulation::new(run_id.clone(), scenario.clone());
    simulation.start()?;
    let mut recording = Recording::new(run_id, &scenario);
    let mut server = LocalMcpServer::default();
    let mut agent = RuleAgent::default();
    recording.provenance.rule_policy_hash = Some(agent.policy_hash());
    for _ in 0..ticks {
        recording.push(agent.step(&mut simulation, &mut server)?);
    }
    Ok(recording)
}
