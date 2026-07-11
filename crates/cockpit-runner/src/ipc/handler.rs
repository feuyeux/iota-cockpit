use std::{fs, path::Path};

use cockpit_agent_runtime::{LocalMcpServer, RuleAgent};
use cockpit_evaluation::evaluate_smoke_shutdown;
use cockpit_recording::{Recording, RecordingStore, diff_recordings, replay_recording};
use cockpit_scenario::load_scenario;
use cockpit_simulation_core::{Simulation, SimulationError, clock::RunStatus};
use serde_json::{Value, json};

use super::proto::{
    IPC_VERSION, IpcError, RunnerCommand, RunnerEvent, RunnerRequest, RunnerResponse,
};

type HandlerResult = Result<Value, Box<IpcError>>;
pub const MAX_EVENT_HISTORY: usize = 2_048;

fn read_recording(path: &str) -> Result<Recording, Box<IpcError>> {
    let bytes = fs::read(Path::new(path)).map_err(|error| {
        Box::new(IpcError {
            code: "RECORDING_READ_FAILED".to_string(),
            message: error.to_string(),
            details: None,
            run_id: None,
            tick: None,
            correlation_id: "recording-diff".to_string(),
        })
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|error| Box::new(RunnerHandler::serialization_error(error.to_string())))
}

pub struct RunnerHandler {
    session_token: String,
    simulation: Option<Simulation>,
    recording: Option<Recording>,
    server: LocalMcpServer,
    agent: RuleAgent,
    events: Vec<RunnerEvent>,
    next_cursor: u64,
    recording_store: Option<RecordingStore>,
}

impl RunnerHandler {
    pub fn new(session_token: impl Into<String>) -> Self {
        Self {
            session_token: session_token.into(),
            simulation: None,
            recording: None,
            server: LocalMcpServer::default(),
            agent: RuleAgent::default(),
            events: Vec::new(),
            next_cursor: 0,
            recording_store: None,
        }
    }

    pub fn new_persistent(
        session_token: impl Into<String>,
        database_path: &str,
    ) -> Result<Self, String> {
        let mut handler = Self::new(session_token);
        handler.recording_store =
            Some(RecordingStore::open(database_path).map_err(|error| error.to_string())?);
        Ok(handler)
    }

    pub fn dispatch(&mut self, request: RunnerRequest) -> RunnerResponse {
        let correlation_id = request.correlation_id.clone();
        if request.version != IPC_VERSION {
            return self.error_response(
                correlation_id,
                "IPC_VERSION_UNSUPPORTED",
                format!("supported IPC version is {IPC_VERSION}"),
            );
        }
        if request.session_token != self.session_token {
            return self.error_response(
                correlation_id,
                "SESSION_UNAUTHORIZED",
                "session token is invalid".to_string(),
            );
        }

        let result = match request.command {
            RunnerCommand::ValidateScenario { path } => self.validate(&path),
            RunnerCommand::CreateSimulationRun { path } => self.create_run(&path),
            RunnerCommand::ResumeSimulation {
                scenario_path,
                run_id,
            } => self.resume_run(&scenario_path, &run_id),
            RunnerCommand::StartSimulation => self.start(),
            RunnerCommand::PauseSimulation => self.pause(),
            RunnerCommand::StepSimulation => self.step(),
            RunnerCommand::StopSimulation => self.stop(),
            RunnerCommand::ApproveAction { request_id } => self.approve_action(&request_id),
            RunnerCommand::RejectAction { request_id, reason } => {
                self.reject_action(&request_id, reason.as_deref())
            }
            RunnerCommand::CancelAgentTurn => self.cancel_agent_turn(),
            RunnerCommand::SetApprovalRequired { required } => self.set_approval_required(required),
            RunnerCommand::GetSimulationSnapshot => self.snapshot(),
            RunnerCommand::GetSimulationEvents { cursor } => Ok(json!({
                "events": self.events_after(cursor),
                "nextCursor": self.next_cursor,
                "firstAvailableCursor": self.events.first().map(RunnerEvent::cursor).unwrap_or(self.next_cursor),
                "resetRequired": self.cursor_reset_required(cursor)
            })),
            RunnerCommand::GetAgentTrace => Ok(json!({
                "events": self
                    .events
                    .iter()
                    .filter(|event| matches!(event, RunnerEvent::SimulationToolCall { .. }))
                    .collect::<Vec<_>>()
            })),
            RunnerCommand::StartReplay {
                scenario_path,
                recording_path,
            } => self.start_replay(&scenario_path, &recording_path),
            RunnerCommand::DiffRecordings {
                source_recording_path,
                candidate_recording_path,
            } => self.diff_recordings(&source_recording_path, &candidate_recording_path),
        };

        match result {
            Ok(result) => RunnerResponse {
                version: IPC_VERSION,
                correlation_id,
                ok: true,
                result: Some(result),
                error: None,
            },
            Err(error) => RunnerResponse {
                version: IPC_VERSION,
                correlation_id: error.correlation_id.clone(),
                ok: false,
                result: None,
                error: Some(*error),
            },
        }
    }

    fn validate(&self, path: &str) -> HandlerResult {
        let scenario =
            load_scenario(path).map_err(|error| Box::new(Self::simulation_error(error, None)))?;
        Ok(json!({
            "id": scenario.id,
            "path": path,
            "schemaVersion": scenario.schema_version,
            "scenarioHash": scenario.scenario_hash,
            "seed": scenario.seed,
            "agentId": scenario.agent.agent_id
        }))
    }

    fn create_run(&mut self, path: &str) -> HandlerResult {
        let scenario =
            load_scenario(path).map_err(|error| Box::new(Self::simulation_error(error, None)))?;
        let run_id = format!("run-{}", scenario.id);
        self.simulation = Some(Simulation::new(run_id.clone(), scenario.clone()));
        self.recording = Some(Recording::new(run_id.clone(), &scenario));
        self.server = LocalMcpServer::default();
        self.agent = RuleAgent::default();
        self.emit(RunnerEvent::SimulationStateChanged {
            cursor: 0,
            state: RunStatus::Ready,
            run_id: Some(run_id.clone()),
        });
        self.persist_recording()?;
        Ok(json!({
            "runId": run_id,
            "status": RunStatus::Ready,
            "scenarioHash": scenario.scenario_hash
        }))
    }

    fn start(&mut self) -> HandlerResult {
        let mut simulation = self
            .simulation
            .take()
            .ok_or_else(|| Box::new(Self::no_run_error()))?;
        let result = simulation.start();
        if let Err(error) = result {
            let ipc_error = Self::simulation_error(error, Some(&simulation));
            self.simulation = Some(simulation);
            return Err(Box::new(ipc_error));
        }
        let run_id = simulation.run_id().to_string();
        self.simulation = Some(simulation);
        self.emit(RunnerEvent::SimulationStateChanged {
            cursor: 0,
            state: RunStatus::Running,
            run_id: Some(run_id.clone()),
        });
        Ok(json!({ "runId": run_id, "status": RunStatus::Running }))
    }

    fn resume_run(&mut self, scenario_path: &str, run_id: &str) -> HandlerResult {
        let store = self.recording_store.as_ref().ok_or_else(|| {
            Box::new(IpcError {
                code: "RECORDING_STORE_UNAVAILABLE".to_string(),
                message: "persistent recording store is not configured".to_string(),
                details: None,
                run_id: Some(run_id.to_string()),
                tick: None,
                correlation_id: "resume".to_string(),
            })
        })?;
        let recording = store
            .load(run_id)
            .map_err(|error| Box::new(Self::serialization_error(error.to_string())))?;
        let scenario = load_scenario(scenario_path)
            .map_err(|error| Box::new(Self::simulation_error(error, None)))?;
        let mut simulation = Simulation::new(run_id.to_string(), scenario.clone());
        simulation
            .start()
            .map_err(|error| Box::new(Self::simulation_error(error, Some(&simulation))))?;
        let actions_by_tick = recording.recorded_actions_by_tick();
        let state_diffs_by_tick = recording.recorded_state_diffs_by_tick();
        self.events.clear();
        self.next_cursor = 0;
        self.recording = Some(Recording::new(run_id.to_string(), &scenario));
        for source_tick in &recording.ticks {
            let actions = actions_by_tick
                .get(&source_tick.tick)
                .cloned()
                .unwrap_or_default();
            let state_diffs = state_diffs_by_tick
                .get(&source_tick.tick)
                .cloned()
                .unwrap_or_default();
            let step = simulation
                .step_with_recorded_inputs(actions, state_diffs)
                .map_err(|error| Box::new(Self::simulation_error(error, Some(&simulation))))?;
            let snapshot = simulation.snapshot.clone();
            if let Some(target) = self.recording.as_mut() {
                target.push(step.clone());
            }
            self.emit(RunnerEvent::SimulationTickCommitted {
                cursor: 0,
                snapshot,
            });
            for event in step.events {
                self.emit(RunnerEvent::SimulationEvent { cursor: 0, event });
            }
            for trace in step.tool_calls {
                self.emit(RunnerEvent::SimulationToolCall { cursor: 0, trace });
            }
            for result in step.action_results {
                self.emit(RunnerEvent::SimulationActionResult { cursor: 0, result });
            }
        }
        self.simulation = Some(simulation);
        self.server = LocalMcpServer::default();
        self.agent = RuleAgent::default();
        Ok(json!({
            "runId": run_id,
            "tick": self.simulation.as_ref().map(|value| value.snapshot.tick).unwrap_or(0),
            "cursor": self.next_cursor,
            "status": RunStatus::Paused
        }))
    }

    fn pause(&mut self) -> HandlerResult {
        let mut simulation = self
            .simulation
            .take()
            .ok_or_else(|| Box::new(Self::no_run_error()))?;
        let result = simulation.pause();
        if let Err(error) = result {
            let ipc_error = Self::simulation_error(error, Some(&simulation));
            self.simulation = Some(simulation);
            return Err(Box::new(ipc_error));
        }
        let run_id = simulation.run_id().to_string();
        self.simulation = Some(simulation);
        self.emit(RunnerEvent::SimulationStateChanged {
            cursor: 0,
            state: RunStatus::Paused,
            run_id: Some(run_id.clone()),
        });
        Ok(json!({ "runId": run_id, "status": RunStatus::Paused }))
    }

    fn step(&mut self) -> HandlerResult {
        let mut simulation = self
            .simulation
            .take()
            .ok_or_else(|| Box::new(Self::no_run_error()))?;
        let result = self.agent.step(&mut simulation, &mut self.server);
        let step = match result {
            Ok(step) => step,
            Err(error) => {
                let ipc_error = Self::simulation_error(error, Some(&simulation));
                self.simulation = Some(simulation);
                return Err(Box::new(ipc_error));
            }
        };
        let tick = step.tick;
        let snapshot = simulation.snapshot.clone();
        let snapshot_hash = step.snapshot_hash.clone();
        if let Some(recording) = self.recording.as_mut() {
            recording.push(step.clone());
        }
        self.persist_recording()?;
        self.emit(RunnerEvent::SimulationTickCommitted {
            cursor: 0,
            snapshot,
        });
        for event in step.events {
            self.emit(RunnerEvent::SimulationEvent { cursor: 0, event });
        }
        for trace in step.tool_calls {
            self.emit(RunnerEvent::SimulationToolCall { cursor: 0, trace });
        }
        for result in step.action_results {
            self.emit(RunnerEvent::SimulationActionResult { cursor: 0, result });
        }
        if let Some(recording) = self.recording.as_ref() {
            let evaluation =
                evaluate_smoke_shutdown(recording, simulation.scenario.shutdown_deadline_ticks);
            self.emit(RunnerEvent::SimulationEvaluationUpdated {
                cursor: 0,
                evaluation: serde_json::to_value(evaluation).unwrap_or(Value::Null),
            });
        }
        let run_id = simulation.run_id().to_string();
        self.simulation = Some(simulation);
        Ok(json!({
            "runId": run_id,
            "tick": tick,
            "snapshotHash": snapshot_hash
        }))
    }

    fn stop(&mut self) -> HandlerResult {
        let mut simulation = self
            .simulation
            .take()
            .ok_or_else(|| Box::new(Self::no_run_error()))?;
        simulation.stop();
        let run_id = simulation.run_id().to_string();
        self.simulation = Some(simulation);
        self.emit(RunnerEvent::SimulationStateChanged {
            cursor: 0,
            state: RunStatus::Stopped,
            run_id: Some(run_id.clone()),
        });
        Ok(json!({ "runId": run_id, "status": RunStatus::Stopped }))
    }

    fn approve_action(&mut self, request_id: &str) -> HandlerResult {
        let mut simulation = self
            .simulation
            .take()
            .ok_or_else(|| Box::new(Self::no_run_error()))?;
        let result = self.server.approve_action(&mut simulation, request_id);
        self.simulation = Some(simulation);
        let result = result.map_err(|error| Box::new(Self::tool_error(error)))?;
        self.emit(RunnerEvent::SimulationActionResult {
            cursor: 0,
            result: result.clone(),
        });
        serde_json::to_value(result)
            .map_err(|error| Box::new(Self::serialization_error(error.to_string())))
    }

    fn reject_action(&mut self, request_id: &str, reason: Option<&str>) -> HandlerResult {
        let simulation = self
            .simulation
            .as_ref()
            .ok_or_else(|| Box::new(Self::no_run_error()))?;
        let result = self
            .server
            .reject_action(simulation, request_id, false)
            .map_err(|error| Box::new(Self::tool_error(error)))?;
        self.emit(RunnerEvent::SimulationActionResult {
            cursor: 0,
            result: result.clone(),
        });
        Ok(json!({
            "result": result,
            "reason": reason
        }))
    }

    fn cancel_agent_turn(&mut self) -> HandlerResult {
        let simulation = self
            .simulation
            .as_ref()
            .ok_or_else(|| Box::new(Self::no_run_error()))?;
        let results = self.server.cancel_pending_actions(simulation);
        for result in &results {
            self.emit(RunnerEvent::SimulationActionResult {
                cursor: 0,
                result: result.clone(),
            });
        }
        Ok(json!({ "cancelled": true, "count": results.len() }))
    }

    fn set_approval_required(&mut self, required: bool) -> HandlerResult {
        self.server.set_approval_required(required);
        Ok(json!({ "approvalRequired": required }))
    }

    fn persist_recording(&mut self) -> HandlerResult {
        let Some(recording) = self.recording.as_ref() else {
            return Ok(Value::Null);
        };
        let Some(store) = self.recording_store.as_mut() else {
            return Ok(Value::Null);
        };
        store
            .save(recording)
            .map_err(|error| Box::new(Self::serialization_error(error.to_string())))?;
        Ok(Value::Null)
    }

    fn snapshot(&self) -> HandlerResult {
        let simulation = self
            .simulation
            .as_ref()
            .ok_or_else(|| Box::new(Self::no_run_error()))?;
        serde_json::to_value(&simulation.snapshot)
            .map_err(|error| Box::new(Self::serialization_error(error.to_string())))
    }

    fn start_replay(&mut self, scenario_path: &str, recording_path: &str) -> HandlerResult {
        let scenario = load_scenario(scenario_path)
            .map_err(|error| Box::new(Self::simulation_error(error, None)))?;
        let bytes = fs::read(Path::new(recording_path)).map_err(|error| {
            Box::new(IpcError {
                code: "RECORDING_READ_FAILED".to_string(),
                message: error.to_string(),
                details: None,
                run_id: None,
                tick: None,
                correlation_id: "replay".to_string(),
            })
        })?;
        let recording: Recording = serde_json::from_slice(&bytes)
            .map_err(|error| Box::new(Self::serialization_error(error.to_string())))?;
        let replay = replay_recording("replay-run", scenario.clone(), &recording)
            .map_err(|error| Box::new(Self::simulation_error(error, None)))?;
        let mut simulation = Simulation::new(replay.run_id.clone(), scenario);
        simulation
            .start()
            .map_err(|error| Box::new(Self::simulation_error(error, Some(&simulation))))?;
        let actions_by_tick = recording.recorded_actions_by_tick();
        let state_diffs_by_tick = recording.recorded_state_diffs_by_tick();
        self.events.clear();
        self.next_cursor = 0;
        self.emit(RunnerEvent::SimulationStateChanged {
            cursor: 0,
            state: RunStatus::Replaying,
            run_id: Some(replay.run_id.clone()),
        });
        for source_tick in &recording.ticks {
            let actions = actions_by_tick
                .get(&source_tick.tick)
                .cloned()
                .unwrap_or_default();
            let state_diffs = state_diffs_by_tick
                .get(&source_tick.tick)
                .cloned()
                .unwrap_or_default();
            let step = simulation
                .step_with_recorded_inputs(actions, state_diffs)
                .map_err(|error| Box::new(Self::simulation_error(error, Some(&simulation))))?;
            let snapshot = simulation.snapshot.clone();
            self.emit(RunnerEvent::SimulationTickCommitted {
                cursor: 0,
                snapshot,
            });
            for event in step.events {
                self.emit(RunnerEvent::SimulationEvent { cursor: 0, event });
            }
            for trace in step.tool_calls {
                self.emit(RunnerEvent::SimulationToolCall { cursor: 0, trace });
            }
            for result in step.action_results {
                self.emit(RunnerEvent::SimulationActionResult { cursor: 0, result });
            }
        }
        simulation.status = RunStatus::Completed;
        self.simulation = Some(simulation);
        self.recording = Some(replay.clone());
        self.emit(RunnerEvent::SimulationStateChanged {
            cursor: 0,
            state: RunStatus::Completed,
            run_id: Some(replay.run_id.clone()),
        });
        Ok(json!({
            "runId": replay.run_id,
            "ticks": replay.ticks.len(),
            "finalSnapshotHash": replay.final_snapshot_hash()
        }))
    }

    fn diff_recordings(
        &self,
        source_recording_path: &str,
        candidate_recording_path: &str,
    ) -> HandlerResult {
        let source = read_recording(source_recording_path)?;
        let candidate = read_recording(candidate_recording_path)?;
        serde_json::to_value(diff_recordings(&source, &candidate))
            .map_err(|error| Box::new(Self::serialization_error(error.to_string())))
    }

    fn events_after(&self, cursor: Option<u64>) -> Vec<RunnerEvent> {
        let cursor = cursor.unwrap_or(0);
        self.events
            .iter()
            .filter(|event| event.cursor() > cursor)
            .cloned()
            .collect()
    }

    fn cursor_reset_required(&self, cursor: Option<u64>) -> bool {
        let Some(cursor) = cursor else {
            return false;
        };
        let Some(first) = self.events.first().map(RunnerEvent::cursor) else {
            return false;
        };
        cursor.saturating_add(1) < first
    }

    fn emit(&mut self, event: RunnerEvent) {
        self.next_cursor += 1;
        let cursor = self.next_cursor;
        let event = match event {
            RunnerEvent::SimulationStateChanged { state, run_id, .. } => {
                RunnerEvent::SimulationStateChanged {
                    cursor,
                    state,
                    run_id,
                }
            }
            RunnerEvent::SimulationTickCommitted { snapshot, .. } => {
                RunnerEvent::SimulationTickCommitted { cursor, snapshot }
            }
            RunnerEvent::SimulationEvent { event, .. } => {
                RunnerEvent::SimulationEvent { cursor, event }
            }
            RunnerEvent::SimulationToolCall { trace, .. } => {
                RunnerEvent::SimulationToolCall { cursor, trace }
            }
            RunnerEvent::SimulationActionResult { result, .. } => {
                RunnerEvent::SimulationActionResult { cursor, result }
            }
            RunnerEvent::SimulationEvaluationUpdated { evaluation, .. } => {
                RunnerEvent::SimulationEvaluationUpdated { cursor, evaluation }
            }
            RunnerEvent::SimulationError { error, .. } => {
                RunnerEvent::SimulationError { cursor, error }
            }
        };
        self.events.push(event);
        if self.events.len() > MAX_EVENT_HISTORY {
            let excess = self.events.len() - MAX_EVENT_HISTORY;
            self.events.drain(..excess);
        }
    }

    fn error_response(
        &self,
        correlation_id: String,
        code: &str,
        message: String,
    ) -> RunnerResponse {
        RunnerResponse {
            version: IPC_VERSION,
            correlation_id: correlation_id.clone(),
            ok: false,
            result: None,
            error: Some(IpcError {
                code: code.to_string(),
                message,
                details: None,
                run_id: None,
                tick: None,
                correlation_id,
            }),
        }
    }

    fn no_run_error() -> IpcError {
        IpcError {
            code: "RUN_NOT_CREATED".to_string(),
            message: "create a simulation run first".to_string(),
            details: None,
            run_id: None,
            tick: None,
            correlation_id: "runner".to_string(),
        }
    }

    fn simulation_error(error: SimulationError, simulation: Option<&Simulation>) -> IpcError {
        IpcError {
            code: "SIMULATION_ERROR".to_string(),
            message: error.to_string(),
            details: None,
            run_id: simulation.map(|value| value.run_id().to_string()),
            tick: simulation.map(|value| value.snapshot.tick),
            correlation_id: "runner".to_string(),
        }
    }

    fn serialization_error(message: String) -> IpcError {
        IpcError {
            code: "SERIALIZATION_ERROR".to_string(),
            message,
            details: None,
            run_id: None,
            tick: None,
            correlation_id: "runner".to_string(),
        }
    }

    fn tool_error(error: cockpit_agent_runtime::ToolError) -> IpcError {
        IpcError {
            code: error.code,
            message: error.message,
            details: None,
            run_id: None,
            tick: None,
            correlation_id: "runner".to_string(),
        }
    }
}
