//! Explicit transaction orchestration for one world tick.

use std::{collections::BTreeSet, time::Instant};

use super::{
    ActionStatus, Observation, RunStatus, Simulation, SimulationError, SimulationResult,
    StepRecord, TickPhaseHash,
};
use crate::{ActionResult, EventEnvelope, StateDiff};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TickPhase {
    DigitalTwin,
    Faults,
    Influences,
    PendingActions,
    HumanStateDeltas,
    StateDiffs,
    Perception,
    ActionResultEvents,
    Finalize,
}

pub const TICK_PHASE_ORDER: &[TickPhase] = &[
    TickPhase::DigitalTwin,
    TickPhase::Faults,
    TickPhase::Influences,
    TickPhase::PendingActions,
    TickPhase::HumanStateDeltas,
    TickPhase::StateDiffs,
    TickPhase::Perception,
    TickPhase::ActionResultEvents,
    TickPhase::Finalize,
];

/// Non-persistent wall-clock sample for a completed tick phase. It is not
/// included in `StepRecord` because timing would make deterministic recording
/// content machine-dependent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickPhaseTiming {
    pub phase: TickPhase,
    pub elapsed_nanos: u128,
}

pub(super) struct TickTransaction<'a> {
    simulation: &'a mut Simulation,
    tick: u64,
    observation: Observation,
    pending_state_diffs: Option<Vec<StateDiff>>,
    state_diffs: Vec<StateDiff>,
    events: Vec<EventEnvelope>,
    action_write_set: BTreeSet<String>,
    action_results: Vec<ActionResult>,
    phase_hashes: Vec<TickPhaseHash>,
    phase_timings: Vec<TickPhaseTiming>,
}

impl<'a> TickTransaction<'a> {
    pub(super) fn new(
        simulation: &'a mut Simulation,
        observation: Observation,
        state_diffs: Vec<StateDiff>,
    ) -> Self {
        simulation.last_phase_timings.clear();
        let tick = simulation.snapshot.tick;
        let events = simulation.pending_lifecycle_events.clone();
        Self {
            simulation,
            tick,
            observation,
            pending_state_diffs: Some(state_diffs),
            state_diffs: Vec::new(),
            events,
            action_write_set: BTreeSet::new(),
            action_results: Vec::new(),
            phase_hashes: Vec::with_capacity(TICK_PHASE_ORDER.len()),
            phase_timings: Vec::with_capacity(TICK_PHASE_ORDER.len()),
        }
    }

    pub(super) fn commit(mut self) -> SimulationResult<StepRecord> {
        if matches!(self.simulation.status, RunStatus::Ready | RunStatus::Paused) {
            self.simulation.status = RunStatus::Running;
        }
        if self.simulation.status != RunStatus::Running
            && self.simulation.status != RunStatus::Degraded
        {
            return Err(SimulationError::InvalidRunState);
        }
        for phase in TICK_PHASE_ORDER {
            if *phase == TickPhase::Finalize {
                return self.finalize();
            }
            self.apply_and_hash(*phase)?;
        }
        unreachable!("the finalize phase always returns the step record")
    }

    fn apply_and_hash(&mut self, phase: TickPhase) -> SimulationResult<()> {
        let started = Instant::now();
        let input_snapshot_hash = self.simulation.snapshot.content_hash()?;
        let input_event_hash = event_hash(&self.events)?;
        self.apply(phase)?;
        let output_snapshot_hash = self.simulation.snapshot.content_hash()?;
        let output_event_hash = event_hash(&self.events)?;
        self.phase_hashes.push(TickPhaseHash {
            phase,
            input_snapshot_hash,
            output_snapshot_hash,
            input_event_hash,
            output_event_hash,
        });
        self.phase_timings.push(TickPhaseTiming {
            phase,
            elapsed_nanos: started.elapsed().as_nanos(),
        });
        Ok(())
    }

    fn apply(&mut self, phase: TickPhase) -> SimulationResult<()> {
        match phase {
            TickPhase::DigitalTwin => self.simulation.apply_digital_twin(&mut self.events),
            TickPhase::Faults => {
                for fault in self.simulation.scenario.faults.clone() {
                    if fault.at_tick == self.tick {
                        self.simulation.apply_fault(&fault, &mut self.events);
                    }
                }
                Ok(())
            }
            TickPhase::Influences => {
                self.simulation
                    .apply_influences(self.tick, &mut self.events);
                Ok(())
            }
            TickPhase::PendingActions => {
                self.action_write_set = self.simulation.pending_action_write_set();
                self.simulation.apply_pending_actions(&mut self.events);
                Ok(())
            }
            TickPhase::HumanStateDeltas => {
                self.simulation
                    .apply_pending_human_state_deltas(&self.action_write_set, &mut self.events);
                Ok(())
            }
            TickPhase::StateDiffs => {
                let diffs = self
                    .pending_state_diffs
                    .take()
                    .expect("state diffs run once");
                self.state_diffs = self.simulation.apply_state_diffs(diffs, &mut self.events)?;
                Ok(())
            }
            TickPhase::Perception => {
                self.simulation.apply_perception(self.tick, &self.events);
                Ok(())
            }
            TickPhase::ActionResultEvents => {
                self.action_results = std::mem::take(&mut self.simulation.latest_results);
                for result in &self.action_results {
                    if result.status == ActionStatus::Rejected {
                        let error_code = result
                            .error_code
                            .as_ref()
                            .map(|code| code.stable_code().to_string());
                        self.events.push(self.simulation.event_with_error(
                            "ActionRejected",
                            "action-gateway",
                            Some(&result.request.target),
                            error_code,
                            "action rejected by the Action Gateway",
                        ));
                    }
                }
                self.observation.action_results = self
                    .action_results
                    .iter()
                    .map(|result| format!("{:?}:{}", result.status, result.request.request_id))
                    .collect();
                Ok(())
            }
            TickPhase::Finalize => unreachable!("finalize returns from commit"),
        }
    }

    fn finalize(mut self) -> SimulationResult<StepRecord> {
        let started = Instant::now();
        let input_snapshot_hash = self.simulation.snapshot.content_hash()?;
        let input_event_hash = event_hash(&self.events)?;
        self.simulation.snapshot.tick += 1;
        self.simulation.snapshot.version += 1;
        self.simulation.snapshot.sim_time_ms =
            self.simulation.snapshot.tick * self.simulation.scenario.clock.tick_ms;
        let snapshot_hash = self.simulation.snapshot.content_hash()?;
        let output_event_hash = event_hash(&self.events)?;
        self.phase_hashes.push(TickPhaseHash {
            phase: TickPhase::Finalize,
            input_snapshot_hash,
            output_snapshot_hash: snapshot_hash.clone(),
            input_event_hash,
            output_event_hash,
        });
        self.phase_timings.push(TickPhaseTiming {
            phase: TickPhase::Finalize,
            elapsed_nanos: started.elapsed().as_nanos(),
        });
        self.simulation.last_phase_timings = self.phase_timings;
        self.simulation.pending_lifecycle_events.clear();
        Ok(StepRecord {
            tick: self.tick,
            snapshot_hash,
            events: self.events,
            observation: self.observation,
            action_results: self.action_results,
            tool_calls: Vec::new(),
            errors: Vec::new(),
            fallback: None,
            state_diffs: self.state_diffs,
            plugin_failures: Vec::new(),
            phase_hashes: self.phase_hashes,
        })
    }
}

fn event_hash(events: &[EventEnvelope]) -> SimulationResult<String> {
    let bytes = serde_json::to_vec(events)
        .map_err(|error| SimulationError::Serialization(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(b"cockpit-world-tick-events-v1\0");
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::{TICK_PHASE_ORDER, TickPhase};

    #[test]
    fn phases_have_a_stable_commit_order() {
        assert_eq!(
            TICK_PHASE_ORDER,
            &[
                TickPhase::DigitalTwin,
                TickPhase::Faults,
                TickPhase::Influences,
                TickPhase::PendingActions,
                TickPhase::HumanStateDeltas,
                TickPhase::StateDiffs,
                TickPhase::Perception,
                TickPhase::ActionResultEvents,
                TickPhase::Finalize,
            ]
        );
    }
}
