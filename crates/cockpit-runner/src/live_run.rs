use cockpit_agent_runtime::{HumanAgentDriver, HumanTurnEvidence, LocalMcpServer};
use cockpit_recording::Recording;
use cockpit_scenario::load_scenario;
use cockpit_simulation_core::{Simulation, clock::RunStatus};
use serde::Serialize;
use serde_json::Value;

/// Configuration for a live-agent run. Every human's decision each tick must
/// come from a real backend turn; there is no fallback, retry, or circuit
/// breaker. A backend failure aborts the run immediately.
#[derive(Debug, Clone)]
pub struct LiveRunConfig {
    pub scenario_path: String,
    pub ticks: u64,
    pub timeout_ms: u64,
}

/// Per-tick, per-human disposition evidence for a live run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveTickEvidence {
    pub tick: u64,
    pub snapshot_hash: String,
    pub humans: Vec<HumanTurnEvidence>,
}

/// Result of a live-agent run. `ticks` is the number of ticks committed
/// before either completing the requested tick count or the run being aborted
/// by a fatal backend error (in which case `error` is set and `ticks` is the
/// count of ticks successfully committed beforehand).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRunReport {
    pub run_id: String,
    pub scenario_hash: String,
    pub ticks: usize,
    pub final_snapshot_hash: Option<String>,
    pub tick_evidence: Vec<LiveTickEvidence>,
    pub backend: &'static str,
    pub evaluation: Value,
    /// Set when the run was aborted by a mandatory backend failure. `None`
    /// means every requested tick completed with a real backend decision for
    /// every human.
    pub error: Option<String>,
    /// Complete immutable input for the independent evaluator. It is omitted
    /// from the normal run summary and written only when the CLI explicitly
    /// requests a recording artifact.
    #[serde(skip)]
    pub recording: Recording,
}

/// Drive a live-agent run for `config.ticks` ticks.
///
/// Every tick, every human's decision must come from a real backend (hermes,
/// etc.) turn. If any human's backend turn fails, times out, or returns
/// invalid output, the run stops immediately: the offending tick is not
/// committed, and `LiveRunReport::error` carries the reason. This is a
/// deliberate departure from advisory/fallback behavior: the backend is now a
/// required dependency for a live run, not an optional enhancement.
pub async fn run_live(config: LiveRunConfig) -> anyhow::Result<LiveRunReport> {
    let scenario = load_scenario(&config.scenario_path)?;
    let run_id = format!("live-run-{}", scenario.id);
    let mut simulation = Simulation::new(run_id.clone(), scenario.clone());
    simulation.start()?;
    let mut recording = Recording::new(run_id.clone(), &scenario);

    let mut driver = HumanAgentDriver::new();
    let mut server = LocalMcpServer::default();
    let mut backend = backend_impl::backend_session(&scenario, config.timeout_ms)?;

    let mut evidence = Vec::with_capacity(config.ticks as usize);
    let mut run_error: Option<String> = None;

    for _ in 0..config.ticks {
        if simulation.status != RunStatus::Running {
            break;
        }
        let step_result = driver
            .step_with_tools(&mut simulation, &mut backend, &mut server)
            .await;

        match step_result {
            Ok((step, humans)) => {
                evidence.push(LiveTickEvidence {
                    tick: step.tick,
                    snapshot_hash: step.snapshot_hash.clone(),
                    humans: humans.clone(),
                });
                recording.push(step);
                recording.push_human_turns(humans);
            }
            Err(error) => {
                simulation.fail();
                run_error = Some(error.to_string());
                break;
            }
        }
    }

    let execution_passed = run_error.is_none();
    let evaluation = serde_json::json!({
        "status": "pending",
        "passed": execution_passed,
        "score": if execution_passed { 1.0 } else { 0.0 },
        "evidenceEventIds": [],
        "firstFailureTick": if execution_passed { None } else { Some(simulation.snapshot.tick) },
        "explanation": if execution_passed {
            "live run completed with mandatory backend decisions"
        } else {
            "mandatory agent execution failed"
        },
        "executionPassed": execution_passed,
        "evaluator": "cockpit-evaluator",
        "recordingRunId": recording.run_id.clone(),
        "executionError": run_error.clone()
    });

    Ok(LiveRunReport {
        run_id,
        scenario_hash: scenario.scenario_hash,
        ticks: recording.ticks.len(),
        final_snapshot_hash: recording.final_snapshot_hash().map(str::to_string),
        tick_evidence: evidence,
        backend: backend.label(),
        evaluation,
        error: run_error,
        recording,
    })
}

/// Replay a previously recorded live run deterministically, without any real
/// backend, by feeding the recorded per-human decisions back through the same
/// [`HumanAgentDriver`] via a `RecordedHumanBackend`. Returns the replayed
/// recording, whose final snapshot hash must match the original for a
/// deterministic run.
pub async fn replay_live(
    scenario: cockpit_simulation_core::SimulationScenario,
    source: &Recording,
) -> anyhow::Result<Recording> {
    use cockpit_agent_runtime::RecordedHumanBackend;

    if source.runtime_contract_version != cockpit_recording::CURRENT_RUNTIME_CONTRACT_VERSION {
        anyhow::bail!(
            "live recording runtime contract version {} is incompatible with {}",
            source.runtime_contract_version,
            cockpit_recording::CURRENT_RUNTIME_CONTRACT_VERSION
        );
    }

    let run_id = format!("replay-{}", scenario.id);
    let mut simulation = Simulation::new(run_id.clone(), scenario.clone());
    simulation.start()?;
    let mut recording = Recording::new(run_id, &scenario);
    let mut driver = HumanAgentDriver::new();
    let mut server = LocalMcpServer::default();
    let mut backend = RecordedHumanBackend::from_tick_evidence(&source.human_turns);

    for _ in 0..source.ticks.len() {
        if simulation.status != RunStatus::Running {
            break;
        }
        let (step, humans) = driver
            .step_with_tools(&mut simulation, &mut backend, &mut server)
            .await
            .map_err(|error| anyhow::anyhow!("live replay diverged: {error}"))?;
        recording.push(step);
        recording.push_human_turns(humans);
    }
    Ok(recording)
}

// The backend session abstraction lets the deterministic default build exercise
// the full per-human driver/recording path without the external iota-core
// process, while the `live-acp` feature swaps in the real ACP backend. Both
// paths honor the mandatory-backend contract identically: `run_live` never
// substitutes a value when a backend call fails, regardless of which backend
// is configured. The synthetic backend is an explicit, always-on stand-in for
// offline/default-build development (documented as such via its `"synthetic"`
// label in every report), not a silent fallback used when a *real* backend
// fails; enabling `live-acp` is what opts a run into calling a real backend at
// all.
#[cfg(not(feature = "live-acp"))]
pub(crate) mod backend_impl {
    use std::collections::BTreeSet;

    use cockpit_agent_runtime::{HumanBackend, HumanTurnContext, OpenWorldRuntime};
    use cockpit_simulation_core::SimulationScenario;
    use tokio_util::sync::CancellationToken;

    /// Synthetic backend session used when the real ACP backend is not compiled
    /// in. It deterministically exercises observation, run-status, action, and
    /// final outputs so the same tool-loop/recording/replay path runs offline.
    pub struct BackendSession {
        cancellation: CancellationToken,
        handled_alerts: BTreeSet<String>,
    }

    impl HumanBackend for BackendSession {
        async fn run_turn(&mut self, context: &HumanTurnContext) -> Result<String, String> {
            if self.cancellation.is_cancelled() {
                return Err("backend turn cancelled".to_string());
            }
            if context.tool_history.is_empty() {
                return Ok(serde_json::json!({
                    "type": "toolCall",
                    "tool": "simulation.get_observation",
                    "arguments": {}
                })
                .to_string());
            }

            let observation = context
                .tool_history
                .iter()
                .find(|exchange| exchange.call.tool == "simulation.get_observation")
                .and_then(|exchange| {
                    serde_json::from_value::<cockpit_simulation_core::Observation>(
                        exchange.response.result.clone(),
                    )
                    .ok()
                });
            let action = observation.as_ref().and_then(|observation| {
                observation
                    .alerts
                    .iter()
                    .chain(context.delivered_perception.iter().map(|event| &event.kind))
                    .filter(|alert| !self.handled_alerts.contains(*alert))
                    .find_map(|alert| action_for_alert(alert).map(|action| (alert.clone(), action)))
                    .filter(|(_, action)| {
                        context
                            .action_capabilities
                            .iter()
                            .any(|capability| capability.as_str() == action.2)
                    })
            });

            if let Some((alert, (target, command, _))) = action {
                let status = context
                    .tool_history
                    .iter()
                    .find(|exchange| exchange.call.tool == "simulation.get_run_status");
                if status.is_none() {
                    return Ok(serde_json::json!({
                        "type": "toolCall",
                        "tool": "simulation.get_run_status",
                        "arguments": {}
                    })
                    .to_string());
                }
                let action_called = context
                    .tool_history
                    .iter()
                    .any(|exchange| exchange.call.tool == "simulation.request_action");
                if !action_called {
                    let status = &status.expect("status checked above").response.result;
                    let state_version = status
                        .get("stateVersion")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or_default();
                    let tick = status
                        .get("tick")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or_default();
                    self.handled_alerts.insert(alert);
                    return Ok(serde_json::json!({
                        "type": "toolCall",
                        "tool": "simulation.request_action",
                        "arguments": {
                            "target": target,
                            "command": command,
                            "expectedStateVersion": state_version,
                            "expiresAtTick": tick + 3
                        }
                    })
                    .to_string());
                }
                return Ok(serde_json::json!({
                    "type": "final",
                    "narrative": "recognized an actionable cockpit risk and used the authorized action tool"
                })
                .to_string());
            }

            let narrative = if context.persona.traits.neuroticism > 0.6 {
                "felt uneasy and watchful"
            } else {
                "monitored the cabin calmly"
            };
            Ok(serde_json::json!({
                "type": "final",
                "narrative": narrative
            })
            .to_string())
        }
    }

    fn action_for_alert(alert: &str) -> Option<(&'static str, &'static str, &'static str)> {
        Some(match alert {
            "SmokeDetected" => ("engine-1", "engineShutdown", "engine.shutdown"),
            "ThermalComfortRisk" => ("hvac-1", "climateComfortRestore", "climate.restoreComfort"),
            "WindshieldVisibilityRisk" => (
                "defogger-1",
                "windshieldDefogActivate",
                "visibility.activateDefog",
            ),
            "DriverFatigueRisk" => (
                "dms-1",
                "fatigueInterventionActivate",
                "driver.activateFatigueIntervention",
            ),
            "ChildPresenceHeatRisk" => (
                "occupant-radar-1",
                "childProtectionActivate",
                "occupant.activateChildProtection",
            ),
            "MedicalEmergencyRisk" => (
                "emergency-call-1",
                "medicalResponseActivate",
                "health.activateMedicalResponse",
            ),
            "MultiUserPrivacyConflict" => (
                "voice-array-1",
                "privacyModeActivate",
                "privacy.activateMode",
            ),
            "EvRangeRisk" => (
                "navigation-1",
                "chargingPlanAccept",
                "energy.acceptChargingPlan",
            ),
            "AdasTakeoverRequired" => (
                "adas-controller-1",
                "adasTakeoverAcknowledge",
                "adas.acknowledgeTakeover",
            ),
            "CyberControlAnomaly" => (
                "security-monitor-1",
                "cyberSafeModeActivate",
                "cybersecurity.enterSafeMode",
            ),
            _ => return None,
        })
    }

    impl BackendSession {
        pub fn label(&self) -> &'static str {
            "synthetic"
        }

        pub async fn warm(&mut self) -> Result<(), String> {
            Ok(())
        }

        pub async fn restore_backend_sessions(
            &mut self,
            _runtime: &OpenWorldRuntime,
        ) -> Result<(), String> {
            Ok(())
        }

        pub fn set_turn_cancellation(&mut self, cancellation: CancellationToken) {
            self.cancellation = cancellation;
        }
    }

    pub fn backend_session(
        _scenario: &SimulationScenario,
        _timeout_ms: u64,
    ) -> anyhow::Result<BackendSession> {
        Ok(BackendSession {
            cancellation: CancellationToken::new(),
            handled_alerts: BTreeSet::new(),
        })
    }
}

#[cfg(feature = "live-acp")]
pub(crate) mod backend_impl {
    use cockpit_agent_runtime::{
        BackendConversationUpdate, HumanBackend, HumanTurnContext, OpenWorldRuntime,
        acp_adapter::{AcpAdapterConfig, AcpAdapterError, AcpTurn, IotaCoreAcpAdapter},
        iota_core_adapter::{CockpitSkill, IotaCoreAdapter},
        live::validate_turn_output,
    };
    use cockpit_simulation_core::SimulationScenario;
    use std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
    };
    use tokio_util::sync::CancellationToken;

    fn load_skill(language: &str) -> anyhow::Result<CockpitSkill> {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        IotaCoreAdapter::new(workspace)
            .load_cockpit_skill_localized(language)
            .map_err(anyhow::Error::msg)
    }

    /// Live backend backed by real iota-core ACP adapters. Each human owns a
    /// distinct adapter, ACP conversation, and native MCP state file. The
    /// active adapter is parked and restored as the deterministic scheduler
    /// moves between humans, including dynamically spawned humans.
    pub struct BackendSession {
        adapter: IotaCoreAcpAdapter,
        adapter_config: AcpAdapterConfig,
        active_human_id: String,
        parked_adapters: BTreeMap<String, (IotaCoreAcpAdapter, AcpAdapterConfig)>,
        scenario: SimulationScenario,
        skill: CockpitSkill,
        cancellation: CancellationToken,
        conversation_update: Option<BackendConversationUpdate>,
    }

    /// How many times to retry a turn that failed solely because iota-core's
    /// persistent execution-lock (see
    /// [`AcpAdapterError::is_stale_execution_lock`]) reports the original
    /// request as still `running`. Follow-up attempts receive an opaque,
    /// unique request marker, so they do not collide with a stale row.
    const STALE_LOCK_MAX_ATTEMPTS: u32 = 3;
    const SESSION_INITIALIZATION_MAX_ATTEMPTS: u32 = 2;
    const SLOW_BACKEND_TURN_LOG_MS: u64 = 1_000;

    impl HumanBackend for BackendSession {
        fn prepare_native_tools(
            &mut self,
            simulation: &cockpit_simulation_core::Simulation,
            server: &cockpit_agent_runtime::LocalMcpServer,
            context: &HumanTurnContext,
        ) -> Result<(), String> {
            self.activate_human(&context.human_id)?;
            self.adapter
                .prepare_native_tools(simulation, server, context, &self.skill)
                .map_err(|error| error.to_string())
        }

        fn take_native_tool_calls(
            &mut self,
        ) -> Result<Vec<cockpit_agent_runtime::native_mcp::NativeMcpCall>, String> {
            self.adapter
                .take_native_tool_calls()
                .map_err(|error| error.to_string())
        }

        fn take_conversation_update(&mut self) -> Option<BackendConversationUpdate> {
            self.conversation_update.take()
        }

        async fn run_turn(&mut self, context: &HumanTurnContext) -> Result<String, String> {
            let mut last_error = None;
            for attempt in 1..=STALE_LOCK_MAX_ATTEMPTS {
                let turn = if attempt == 1 {
                    self.adapter
                        .execute_cancellable(context, &self.skill, &self.cancellation)
                        .await
                } else {
                    self.adapter
                        .execute_cancellable_after_stale_lock(
                            context,
                            &self.skill,
                            &self.cancellation,
                        )
                        .await
                };
                match turn {
                    Ok(turn) => {
                        let turn = if let Err(reason) = validate_turn_output(&turn.text) {
                            eprintln!(
                                "live backend returned malformed decision output; requesting format retry: human={} backend={} reason={}",
                                context.human_id, turn.backend, reason
                            );
                            self.adapter
                                .execute_cancellable_after_invalid_output(
                                    context,
                                    &self.skill,
                                    &self.cancellation,
                                )
                                .await
                                .map_err(|error| error.to_string())?
                        } else {
                            turn
                        };
                        if turn.elapsed_ms >= SLOW_BACKEND_TURN_LOG_MS {
                            eprintln!(
                                "live backend turn slow: human={} backend={} elapsed_ms={}",
                                context.human_id, turn.backend, turn.elapsed_ms
                            );
                        }
                        return Ok(self.complete_turn(turn));
                    }
                    Err(error) if error.is_session_initialization_failure() => {
                        // `session/new` failed before a prompt was submitted.
                        // Replacing the client is safe and avoids retaining a
                        // Hermes process whose ACP state is already invalid.
                        let mut session_error = error;
                        for session_attempt in 2..=SESSION_INITIALIZATION_MAX_ATTEMPTS {
                            if self.cancellation.is_cancelled() {
                                return Err("backend turn cancelled".to_string());
                            }
                            eprintln!(
                                "live backend session recovery: human={} attempt={}/{} error={}",
                                context.human_id,
                                session_attempt,
                                SESSION_INITIALIZATION_MAX_ATTEMPTS,
                                session_error
                            );
                            let mut replacement =
                                IotaCoreAcpAdapter::with_fresh_session(self.adapter_config.clone());
                            replacement.inherit_native_turn_generation(&self.adapter);
                            self.adapter = replacement;
                            if let Err(warm_error) = self.adapter.warm().await {
                                session_error = warm_error;
                                continue;
                            }
                            match self
                                .adapter
                                .execute_cancellable(context, &self.skill, &self.cancellation)
                                .await
                            {
                                Ok(turn) => {
                                    eprintln!(
                                        "live backend turn completed after session recovery: human={} backend={} elapsed_ms={}",
                                        context.human_id, turn.backend, turn.elapsed_ms
                                    );
                                    return Ok(self.complete_turn(turn));
                                }
                                Err(retry_error) => session_error = retry_error,
                            }
                        }
                        eprintln!(
                            "live backend session recovery failed: human={} error={}",
                            context.human_id, session_error
                        );
                        return Err(session_error.to_string());
                    }
                    Err(error) if error.is_stale_execution_lock() => {
                        last_error = Some(error);
                        if attempt < STALE_LOCK_MAX_ATTEMPTS {
                            // Start the retry from a fresh ACP client. The
                            // adapter also adds a fresh opaque marker, which
                            // avoids the stale request-hash row directly.
                            let mut replacement =
                                IotaCoreAcpAdapter::with_fresh_session(self.adapter_config.clone());
                            replacement.inherit_native_turn_generation(&self.adapter);
                            self.adapter = replacement;
                        }
                    }
                    Err(error) => {
                        if self.cancellation.is_cancelled() {
                            return Err("backend turn cancelled".to_string());
                        }
                        eprintln!(
                            "live backend turn failed: human={} backend={} error={}",
                            context.human_id,
                            self.label(),
                            error
                        );
                        return Err(error.to_string());
                    }
                }
            }
            let last_error = last_error.unwrap_or_else(|| {
                AcpAdapterError::Turn(
                    "stale-lock retry loop exhausted its attempts without recording a failure; \
                     this indicates a bug in the retry loop rather than a backend error"
                        .to_string(),
                )
            });
            Err(format!(
                "{last_error}. iota-core still rejected all recovery attempts due to an \
                 execution-lock collision. The cockpit retried with independent opaque request \
                 markers, so this is no longer recoverable by clicking Step again; inspect the \
                 upstream ACP/iota-core runtime."
            ))
        }
    }

    impl BackendSession {
        fn complete_turn(&mut self, turn: AcpTurn) -> String {
            let parsed = serde_json::from_str::<serde_json::Value>(&turn.text).ok();
            let response_kind = parsed
                .as_ref()
                .and_then(|value| value.get("type"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let tool_name = parsed
                .as_ref()
                .and_then(|value| value.get("tool"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            self.conversation_update = Some(BackendConversationUpdate {
                backend: turn.backend,
                backend_session_id: turn.session_id,
                response_kind,
                tool_name,
            });
            turn.text
        }

        fn activate_human(&mut self, human_id: &str) -> Result<(), String> {
            if self.active_human_id == human_id {
                return Ok(());
            }
            let (next_adapter, next_config) =
                if let Some(parked) = self.parked_adapters.remove(human_id) {
                    parked
                } else {
                    let mut config = self.adapter_config.clone();
                    config.native_mcp_state_path = Some(native_mcp_state_path());
                    let mut adapter = IotaCoreAcpAdapter::with_fresh_session(config.clone());
                    adapter
                        .initialize_native_mcp(&self.scenario, &self.skill)
                        .map_err(|error| error.to_string())?;
                    (adapter, config)
                };
            let previous_adapter = std::mem::replace(&mut self.adapter, next_adapter);
            let previous_config = std::mem::replace(&mut self.adapter_config, next_config);
            let previous_human = std::mem::replace(&mut self.active_human_id, human_id.to_string());
            self.parked_adapters
                .insert(previous_human, (previous_adapter, previous_config));
            Ok(())
        }

        pub fn label(&self) -> &'static str {
            "iota-core-acp"
        }

        pub async fn warm(&mut self) -> Result<(), String> {
            self.adapter
                .warm()
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        }

        pub async fn restore_backend_sessions(
            &mut self,
            runtime: &OpenWorldRuntime,
        ) -> Result<(), String> {
            let mut backend_session_owners = BTreeMap::<String, String>::new();
            for (human_id, session) in &runtime.sessions {
                let Some(backend_session_id) = session.backend_session_id.as_deref() else {
                    continue;
                };
                if let Some(previous_owner) =
                    backend_session_owners.insert(backend_session_id.to_string(), human_id.clone())
                {
                    return Err(format!(
                        "persisted ACP backend session is shared by humans {previous_owner} and {human_id}"
                    ));
                }
                if let Some(last_backend) = session
                    .acp_conversation
                    .last()
                    .map(|turn| turn.backend.as_str())
                    && last_backend != self.adapter_config.backend
                {
                    return Err(format!(
                        "persisted backend {last_backend} for human {human_id} does not match configured backend {}",
                        self.adapter_config.backend
                    ));
                }
                if human_id == &self.active_human_id {
                    self.adapter
                        .require_backend_session_restore(backend_session_id)
                        .map_err(|error| error.to_string())?;
                    continue;
                }
                let mut config = self.adapter_config.clone();
                config.native_mcp_state_path = Some(native_mcp_state_path());
                let mut adapter = IotaCoreAcpAdapter::with_fresh_session(config.clone());
                adapter
                    .initialize_native_mcp(&self.scenario, &self.skill)
                    .map_err(|error| error.to_string())?;
                adapter
                    .require_backend_session_restore(backend_session_id)
                    .map_err(|error| error.to_string())?;
                self.parked_adapters
                    .insert(human_id.clone(), (adapter, config));
            }
            // Fail the resume command now, before exposing a resumed run, if
            // the active backend cannot restore its exact native session.
            self.adapter
                .warm()
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        }

        pub fn set_turn_cancellation(&mut self, cancellation: CancellationToken) {
            self.cancellation = cancellation;
        }
    }

    impl Drop for BackendSession {
        fn drop(&mut self) {
            if let Some(path) = self.adapter_config.native_mcp_state_path.as_deref() {
                let _ = std::fs::remove_file(path);
            }
            for (_, config) in self.parked_adapters.values() {
                if let Some(path) = config.native_mcp_state_path.as_deref() {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    fn native_mcp_bridge_command() -> PathBuf {
        if let Some(command) = std::env::var_os("COCKPIT_RUNNER_BIN") {
            return PathBuf::from(command);
        }
        std::env::current_exe()
            .ok()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains("cockpit-runner"))
            })
            .unwrap_or_else(|| PathBuf::from("cockpit-runner"))
    }

    fn native_mcp_state_path() -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "cockpit-native-mcp-{}-{nonce}.json",
            std::process::id()
        ))
    }
    pub fn backend_session(
        scenario: &SimulationScenario,
        timeout_ms: u64,
    ) -> anyhow::Result<BackendSession> {
        let skill = load_skill(&scenario.language)?;
        let adapter_config = AcpAdapterConfig {
            cwd: Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
            timeout_ms,
            native_mcp_bridge_command: Some(native_mcp_bridge_command()),
            native_mcp_state_path: Some(native_mcp_state_path()),
            ..AcpAdapterConfig::default()
        };
        let active_human_id = scenario
            .humans
            .first()
            .map(|human| human.id.clone())
            .ok_or_else(|| anyhow::anyhow!("live scenario requires at least one human"))?;
        let mut adapter = IotaCoreAcpAdapter::with_fresh_session(adapter_config.clone());
        adapter
            .initialize_native_mcp(scenario, &skill)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(BackendSession {
            adapter,
            adapter_config,
            active_human_id,
            parked_adapters: BTreeMap::new(),
            scenario: scenario.clone(),
            skill,
            cancellation: CancellationToken::new(),
            conversation_update: None,
        })
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "live-acp"))]
    use super::*;

    /// A backend failure aborts the run immediately: the offending tick is not
    /// committed and the run's error is reported rather than silently
    /// substituted with a rule-based or synthetic decision.
    #[test]
    fn narrativeless_backend_output_is_normalized_by_the_decision_parser() {
        // Narrative prose does not influence simulation behavior, so a
        // backend response without it remains a valid decision with the
        // documented fixed placeholder.
        use cockpit_agent_runtime::live::parse_decision_for_tests as parse_decision;
        let decision =
            parse_decision(r#"{"utterance": "hi"}"#).expect("missing narrative is normalized");
        assert_eq!(decision.narrative, "implicit backend decision");
    }

    #[cfg(not(feature = "live-acp"))]
    #[tokio::test(flavor = "current_thread")]
    async fn live_run_records_per_human_disposition_evidence_per_tick() {
        let report = run_live(LiveRunConfig {
            scenario_path: "../../scenarios/smoke-in-cockpit.yaml".to_string(),
            ticks: 5,
            timeout_ms: 50,
        })
        .await
        .expect("live run completes with the synthetic backend");

        assert!(report.error.is_none(), "no backend failure expected");
        assert!(report.ticks > 0, "at least one tick is committed");
        assert_eq!(
            report.tick_evidence.len(),
            report.ticks,
            "every committed tick carries disposition evidence"
        );
        for tick in &report.tick_evidence {
            assert!(
                !tick.humans.is_empty(),
                "every tick records a decision for at least one human"
            );
        }
        assert!(report.final_snapshot_hash.is_some());
    }
}
