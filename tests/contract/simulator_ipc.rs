use cockpit_agent::GoalStatus;
use cockpit_plugin::{
    PLUGIN_API_VERSION, PluginExecutor, PluginFailurePolicy, PluginManifest, PluginPermission,
    PluginPolicy, StateDiff as PluginStateDiff,
};
use cockpit_recording::{RecordingStore, run_scripted_recording};
use cockpit_scenario::load_scenario;
use cockpit_simulator::ipc::{
    MAX_EVENT_HISTORY, SimulatorHandler,
    proto::{IPC_VERSION, SimulatorCommand, SimulatorRequest},
};
use cockpit_world::{DynamicEntity, HumanState, StatePatch, WorldSnapshot};
use serde_json::Value;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn request(command: SimulatorCommand) -> SimulatorRequest {
    SimulatorRequest {
        version: IPC_VERSION,
        session_token: "session-1".to_string(),
        correlation_id: "contract-correlation".to_string(),
        command,
    }
}

fn plugin_directory(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("cockpit-simulator-plugin-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("plugin directory");
    path
}

fn plugin_manifest() -> PluginManifest {
    let mut manifest = PluginManifest {
        id: "simulator-plugin".to_string(),
        version: "1.0.0".to_string(),
        api_contract: PLUGIN_API_VERSION,
        permissions: vec![PluginPermission::WorldWrite],
        schema: json!({"kind": "simulator-test"}),
        hash: String::new(),
        signature: None,
        command: None,
        filesystem_read_paths: Vec::new(),
        executable_sha256: None,
    };
    let bytes = serde_json::to_vec(&manifest).expect("manifest");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    manifest.hash = format!("sha256:{:x}", hasher.finalize());
    manifest
}

struct PluginExecutorStub {
    result: Result<Vec<PluginStateDiff>, String>,
}

impl PluginExecutor for PluginExecutorStub {
    fn tick(&mut self, _snapshot: &WorldSnapshot) -> Result<Vec<PluginStateDiff>, String> {
        self.result.clone()
    }
}

fn configure_simulator_plugin(
    handler: &mut SimulatorHandler,
    name: &str,
    policy: PluginFailurePolicy,
    result: Result<Vec<PluginStateDiff>, String>,
) -> PathBuf {
    let directory = plugin_directory(name);
    let manifest = plugin_manifest();
    std::fs::write(
        directory.join("plugin.json"),
        serde_json::to_vec(&manifest).expect("manifest bytes"),
    )
    .expect("manifest writes");
    let mut executors = BTreeMap::new();
    executors.insert(
        manifest.id.clone(),
        Box::new(PluginExecutorStub { result }) as Box<dyn PluginExecutor>,
    );
    let plugin_policy = PluginPolicy {
        allowed_permissions: [PluginPermission::WorldRead, PluginPermission::WorldWrite]
            .into_iter()
            .collect(),
        failure_policy: policy,
        require_signature: false,
        ..PluginPolicy::default()
    };
    assert!(
        handler
            .configure_plugins(&directory, plugin_policy, executors)
            .is_empty()
    );
    directory
}

fn plugin_diff(value: f64, version: u64) -> PluginStateDiff {
    PluginStateDiff {
        plugin_id: "simulator-plugin".to_string(),
        patch: StatePatch::CabinVisibility { value },
        expected_state_version: version,
    }
}

fn resign_manifest(manifest: &mut PluginManifest) {
    manifest.hash.clear();
    let bytes = serde_json::to_vec(manifest).expect("manifest serializes");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    manifest.hash = format!("sha256:{:x}", hasher.finalize());
}

#[test]
fn simulator_requires_version_and_session_token() {
    let mut handler = SimulatorHandler::new("session-1");
    let mut invalid = request(SimulatorCommand::GetSimulationSnapshot);
    invalid.version = IPC_VERSION + 1;
    let response = handler.dispatch(invalid);
    assert_eq!(
        response.error.as_ref().map(|error| error.code.as_str()),
        Some("IPC_VERSION_UNSUPPORTED")
    );

    let mut unauthorized = request(SimulatorCommand::GetSimulationSnapshot);
    unauthorized.session_token = "wrong".to_string();
    let response = handler.dispatch(unauthorized);
    assert_eq!(
        response.error.as_ref().map(|error| error.code.as_str()),
        Some("SESSION_UNAUTHORIZED")
    );
}

#[test]
fn simulator_rejects_rule_policy_selection_without_a_trusted_bundle() {
    let mut handler = SimulatorHandler::new("session-1");
    let response = handler.dispatch(request(SimulatorCommand::SelectRulePolicy {
        policy_id: "external-baseline".to_string(),
    }));
    assert_eq!(
        response.error.as_ref().map(|error| error.code.as_str()),
        Some("RULE_POLICY_BUNDLE_UNAVAILABLE")
    );
}

#[test]
fn simulator_reports_rule_policy_unavailable_without_a_trusted_bundle() {
    let mut handler = SimulatorHandler::new("session-1");
    let response = handler.dispatch(request(SimulatorCommand::ListRulePolicies));
    assert!(response.ok);
    let result = response.result.expect("policy status");
    assert_eq!(result.get("available"), Some(&Value::Bool(false)));
    assert_eq!(result.get("policies"), Some(&json!([])));
    assert!(result.get("selectedPolicyId").is_some_and(Value::is_null));
}

#[tokio::test]
async fn repeated_ready_live_create_reuses_the_existing_backend() {
    let mut handler = SimulatorHandler::new("session-1");
    let command = || SimulatorCommand::CreateLiveSimulationRun {
        path: "scenarios/smoke-in-cockpit.yaml".to_string(),
        timeout_ms: 1_000,
    };
    let first = handler.dispatch_async(request(command())).await;
    if !first.ok
        && first.error.as_ref().is_some_and(|error| {
            error.code == "LIVE_BACKEND_INIT_FAILED"
                && error.message.contains("Hermes ACP warm-up failed")
        })
    {
        return;
    }
    assert!(first.ok, "{first:?}");
    let events_after_first = handler
        .dispatch(request(SimulatorCommand::GetSimulationEvents {
            cursor: Some(0),
        }))
        .result
        .and_then(|value| value.get("events").and_then(Value::as_array).cloned())
        .expect("events after first create");

    let second = handler.dispatch_async(request(command())).await;
    assert!(second.ok, "{second:?}");
    assert_eq!(second.result, first.result);
    let events_after_second = handler
        .dispatch(request(SimulatorCommand::GetSimulationEvents {
            cursor: Some(0),
        }))
        .result
        .and_then(|value| value.get("events").and_then(Value::as_array).cloned())
        .expect("events after repeated create");
    assert_eq!(events_after_second, events_after_first);
}

#[tokio::test]
async fn simulator_live_ipc_keeps_one_backend_session_across_interactive_steps() {
    let mut handler = SimulatorHandler::new("session-1");
    let created = handler
        .dispatch_async(request(SimulatorCommand::CreateLiveSimulationRun {
            path: "scenarios/smoke-in-cockpit.yaml".to_string(),
            timeout_ms: 1_000,
        }))
        .await;
    if !created.ok
        && created.error.as_ref().is_some_and(|error| {
            error.code == "LIVE_BACKEND_INIT_FAILED"
                && error.message.contains("Hermes ACP warm-up failed")
        })
    {
        // The desktop package enables `live-acp`, which Cargo unifies across
        // the workspace. Contract tests must remain offline; integration
        // tests cover the opt-in real-backend failure path separately.
        return;
    }
    assert!(created.ok, "{created:?}");
    let backend = created
        .result
        .as_ref()
        .and_then(|value| value.get("backend"))
        .and_then(Value::as_str)
        .expect("backend label");
    assert!(matches!(backend, "synthetic" | "iota-core-acp"));
    if backend == "iota-core-acp" {
        // Feature-unified workspace tests must not call an external model.
        // The default-feature invocation below exercises the full two-step
        // interactive contract with the deterministic synthetic backend.
        return;
    }
    assert!(
        handler
            .dispatch(request(SimulatorCommand::StartSimulation))
            .ok
    );

    let expected_human_turns = [2, 0];
    for (expected_tick, expected_turns) in (0..2).zip(expected_human_turns) {
        let stepped = handler
            .dispatch_async(request(SimulatorCommand::StepLiveSimulation))
            .await;
        assert!(stepped.ok, "{stepped:?}");
        assert_eq!(
            stepped
                .result
                .as_ref()
                .and_then(|value| value.get("tick"))
                .and_then(Value::as_u64),
            Some(expected_tick)
        );
        assert_eq!(
            stepped
                .result
                .as_ref()
                .and_then(|value| value.get("humanTurns"))
                .and_then(Value::as_u64),
            Some(expected_turns)
        );
    }

    let snapshot = handler.dispatch(request(SimulatorCommand::GetSimulationSnapshot));
    assert_eq!(
        snapshot
            .result
            .as_ref()
            .and_then(|value| value.get("tick"))
            .and_then(Value::as_u64),
        Some(2)
    );
    let events = handler.dispatch(request(SimulatorCommand::GetSimulationEvents {
        cursor: Some(0),
    }));
    assert!(
        events
            .result
            .as_ref()
            .and_then(|value| value.get("events"))
            .and_then(Value::as_array)
            .is_some_and(|events| events
                .iter()
                .filter(|event| { event.get("type") == Some(&json!("SimulationTickCommitted")) })
                .count()
                == 2)
    );
    assert!(
        events
            .result
            .as_ref()
            .and_then(|value| value.get("events"))
            .and_then(Value::as_array)
            .is_some_and(|events| events
                .iter()
                .filter(|event| {
                    event.get("type") == Some(&json!("SimulationHumanTurn"))
                        && event.get("backend") == Some(&json!("synthetic"))
                })
                .count()
                == 2)
    );
}

#[test]
fn simulator_step_emits_snapshot_trace_evaluation_and_cursored_events() {
    let mut handler = SimulatorHandler::new("session-1");
    let response = handler.dispatch(request(SimulatorCommand::CreateSimulationRun {
        path: "scenarios/smoke-in-cockpit.yaml".to_string(),
    }));
    assert!(response.ok, "{response:?}");
    let response = handler.dispatch(request(SimulatorCommand::StartSimulation));
    assert!(response.ok, "{response:?}");

    for _ in 0..10 {
        let response = handler.dispatch(request(SimulatorCommand::StepSimulation));
        assert!(response.ok, "{response:?}");
    }

    let response = handler.dispatch(request(SimulatorCommand::GetSimulationEvents {
        cursor: Some(0),
    }));
    assert!(response.ok, "{response:?}");
    let events = response
        .result
        .expect("event result")
        .get("events")
        .cloned()
        .expect("events");
    let events = events.as_array().expect("event array");
    assert!(
        events.iter().any(|event| event.get("type")
            == Some(&Value::String("SimulationTickCommitted".to_string())))
    );
    assert!(
        events.iter().any(
            |event| event.get("type") == Some(&Value::String("SimulationToolCall".to_string()))
        )
    );
    assert!(events.iter().any(|event| event.get("type")
        == Some(&Value::String("SimulationEvaluationProgress".to_string()))));

    let cursor = events
        .last()
        .and_then(|event| event.get("cursor"))
        .and_then(Value::as_u64)
        .expect("cursor");
    let response = handler.dispatch(request(SimulatorCommand::GetSimulationEvents {
        cursor: Some(cursor),
    }));
    assert!(response.ok, "{response:?}");
    assert_eq!(
        response
            .result
            .as_ref()
            .and_then(|result| result.get("events"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}

#[test]
fn simulator_replay_emits_real_snapshots_and_terminal_state() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario");
    let recording = run_scripted_recording("source-run", scenario, 10).expect("recording");
    let path = std::env::temp_dir().join(format!("cockpit-replay-{}.json", uuid::Uuid::new_v4()));
    std::fs::write(
        &path,
        serde_json::to_vec(&recording).expect("recording json"),
    )
    .expect("recording file");

    let mut handler = SimulatorHandler::new("session-1");
    let response = handler.dispatch(request(SimulatorCommand::StartReplay {
        scenario_path: "scenarios/smoke-in-cockpit.yaml".to_string(),
        recording_path: path.to_string_lossy().to_string(),
    }));
    assert!(response.ok, "{response:?}");
    let events = handler.dispatch(request(SimulatorCommand::GetSimulationEvents {
        cursor: Some(0),
    }));
    let events = events
        .result
        .expect("event result")
        .get("events")
        .cloned()
        .expect("events");
    let events = events.as_array().expect("event array");
    assert!(events.iter().any(|event| {
        event.get("type") == Some(&Value::String("SimulationStateChanged".to_string()))
            && event.get("state") == Some(&Value::String("replaying".to_string()))
    }));
    assert!(events.iter().any(|event| {
        event.get("type") == Some(&Value::String("SimulationTickCommitted".to_string()))
            && event
                .get("tick")
                .and_then(Value::as_u64)
                .is_some_and(|tick| tick > 0)
    }));
    assert!(events.iter().any(|event| {
        event.get("type") == Some(&Value::String("SimulationStateChanged".to_string()))
            && event.get("state") == Some(&Value::String("completed".to_string()))
    }));
    let _ = std::fs::remove_file(path);
}

#[test]
fn simulator_exposes_recording_diff_report() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario");
    let recording = run_scripted_recording("source-run", scenario, 10).expect("recording");
    let source =
        std::env::temp_dir().join(format!("cockpit-diff-source-{}.json", uuid::Uuid::new_v4()));
    let candidate = std::env::temp_dir().join(format!(
        "cockpit-diff-candidate-{}.json",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(
        &source,
        serde_json::to_vec(&recording).expect("source json"),
    )
    .expect("source file");
    let mut changed = recording;
    changed.ticks[5].events.clear();
    std::fs::write(
        &candidate,
        serde_json::to_vec(&changed).expect("candidate json"),
    )
    .expect("candidate file");

    let mut handler = SimulatorHandler::new("session-1");
    let response = handler.dispatch(request(SimulatorCommand::DiffRecordings {
        source_recording_path: source.to_string_lossy().to_string(),
        candidate_recording_path: candidate.to_string_lossy().to_string(),
    }));
    assert!(response.ok, "{response:?}");
    assert_eq!(
        response
            .result
            .as_ref()
            .and_then(|report| report.get("equivalent"))
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        response
            .result
            .as_ref()
            .and_then(|report| report.get("firstDivergence"))
            .and_then(|difference| difference.get("eventsMatch"))
            .and_then(Value::as_bool),
        Some(false)
    );
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(candidate);
}

#[test]
fn simulator_bounds_event_history_and_marks_stale_cursors_for_reset() {
    let path = std::env::temp_dir().join(format!(
        "cockpit-event-history-{}.yaml",
        uuid::Uuid::new_v4()
    ));
    let scenario = std::fs::read_to_string("scenarios/smoke-in-cockpit.yaml")
        .expect("source scenario")
        .replace(
            "maxTicks: 80",
            &format!("maxTicks: {}", MAX_EVENT_HISTORY + 101),
        );
    std::fs::write(&path, scenario).expect("long-running scenario");
    let mut handler = SimulatorHandler::new("session-1");
    assert!(
        handler
            .dispatch(request(SimulatorCommand::CreateSimulationRun {
                path: path.to_string_lossy().to_string(),
            }))
            .ok
    );
    assert!(
        handler
            .dispatch(request(SimulatorCommand::StartSimulation))
            .ok
    );
    for _ in 0..(MAX_EVENT_HISTORY + 100) {
        assert!(
            handler
                .dispatch(request(SimulatorCommand::StepSimulation))
                .ok
        );
    }
    let response = handler.dispatch(request(SimulatorCommand::GetSimulationEvents {
        cursor: Some(0),
    }));
    assert!(response.ok, "{response:?}");
    let result = response.result.expect("event result");
    let events = result
        .get("events")
        .and_then(Value::as_array)
        .expect("events");
    assert!(events.len() <= MAX_EVENT_HISTORY);
    assert_eq!(
        result.get("resetRequired").and_then(Value::as_bool),
        Some(true)
    );
    assert!(
        result
            .get("firstAvailableCursor")
            .and_then(Value::as_u64)
            .is_some_and(|cursor| cursor > 1)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn recorded_audit_events_survive_handler_restart_and_are_tick_scoped() {
    let database = std::env::temp_dir().join(format!(
        "cockpit-recorded-audit-{}.db",
        uuid::Uuid::new_v4()
    ));
    let database_text = database.to_string_lossy().to_string();
    let mut handler =
        SimulatorHandler::new_persistent("session-1", &database_text).expect("persistent handler");
    assert!(
        handler
            .dispatch(request(SimulatorCommand::CreateSimulationRun {
                path: "scenarios/smoke-in-cockpit.yaml".to_string(),
            }))
            .ok
    );
    assert!(
        handler
            .dispatch(request(SimulatorCommand::StartSimulation))
            .ok
    );
    assert!(
        handler
            .dispatch(request(SimulatorCommand::StepSimulation))
            .ok
    );
    drop(handler);

    let mut restarted = SimulatorHandler::new_persistent("session-1", &database_text)
        .expect("restarted persistent handler");
    let response = restarted.dispatch(request(SimulatorCommand::GetRecordedAuditEvents {
        run_id: "run-smoke-in-cockpit".to_string(),
        start_tick: 0,
        end_tick: 0,
        offset: None,
        limit: None,
        after_sequence: None,
        tail_limit: None,
    }));
    assert!(response.ok, "{response:?}");
    let result = response.result.expect("audit response");
    let events = result
        .get("events")
        .and_then(Value::as_array)
        .expect("audit events");
    assert!(
        !events.is_empty(),
        "committed tick must have durable evidence"
    );
    assert_eq!(
        result.get("cursorScope").and_then(Value::as_str),
        Some("recordedTickWindow")
    );
    let first_page = restarted.dispatch(request(SimulatorCommand::GetRecordedAuditEvents {
        run_id: "run-smoke-in-cockpit".to_string(),
        start_tick: 0,
        end_tick: 0,
        offset: Some(0),
        limit: Some(1),
        after_sequence: None,
        tail_limit: None,
    }));
    let first_page = first_page.result.expect("first audit page");
    assert_eq!(first_page["events"].as_array().expect("events").len(), 1);
    let first_sequence = first_page["events"][0]["sequence"]
        .as_u64()
        .expect("recording-global sequence");
    let next_offset = first_page["nextOffset"].as_u64().expect("continuation");
    let second_page = restarted.dispatch(request(SimulatorCommand::GetRecordedAuditEvents {
        run_id: "run-smoke-in-cockpit".to_string(),
        start_tick: 0,
        end_tick: 0,
        offset: Some(next_offset as usize),
        limit: Some(1),
        after_sequence: None,
        tail_limit: None,
    }));
    let second_page = second_page.result.expect("second audit page");
    assert_eq!(second_page["offset"].as_u64(), Some(next_offset));
    let second_sequence = second_page["events"][0]["sequence"]
        .as_u64()
        .expect("next sequence");
    assert!(second_sequence > first_sequence);
    assert_eq!(
        second_page["totalEvents"].as_u64(),
        result["events"]
            .as_array()
            .map(|events| events.len() as u64)
    );
    let sequence_page = restarted.dispatch(request(SimulatorCommand::GetRecordedAuditEvents {
        run_id: "run-smoke-in-cockpit".to_string(),
        start_tick: 0,
        end_tick: 0,
        offset: None,
        limit: Some(1),
        after_sequence: Some(first_sequence),
        tail_limit: None,
    }));
    let sequence_page = sequence_page.result.expect("sequence audit page");
    assert_eq!(
        sequence_page["events"][0]["sequence"].as_u64(),
        Some(second_sequence)
    );
    assert_eq!(
        sequence_page["sequenceScope"].as_str(),
        Some("recordingGlobal")
    );
    let tail_page = restarted.dispatch(request(SimulatorCommand::GetRecordedAuditEvents {
        run_id: "run-smoke-in-cockpit".to_string(),
        start_tick: 0,
        end_tick: 0,
        offset: None,
        limit: Some(1),
        after_sequence: None,
        tail_limit: Some(1),
    }));
    let tail_page = tail_page.result.expect("tail-limited audit page");
    assert_eq!(tail_page["events"].as_array().map(Vec::len), Some(1));
    assert_eq!(tail_page["truncated"].as_bool(), Some(true));
    assert!(
        tail_page["totalEvents"]
            .as_u64()
            .is_some_and(|total| total > 1),
        "the complete window count remains available after tail truncation"
    );
    let _ = std::fs::remove_file(&database);
    let _ = std::fs::remove_dir_all(database.with_extension("payloads"));
}

#[test]
fn simulator_commits_plugin_diff_and_records_manifest_hash() {
    let database = std::env::temp_dir().join(format!(
        "cockpit-simulator-plugin-recording-{}.db",
        uuid::Uuid::new_v4()
    ));
    let mut handler = SimulatorHandler::new_persistent("session-1", &database.to_string_lossy())
        .expect("recording store");
    assert!(
        handler
            .dispatch(request(SimulatorCommand::CreateSimulationRun {
                path: "scenarios/smoke-in-cockpit.yaml".to_string(),
            }))
            .ok
    );
    let directory = configure_simulator_plugin(
        &mut handler,
        "accepted",
        PluginFailurePolicy::DisablePlugin,
        Ok(vec![plugin_diff(0.25, 0)]),
    );
    assert!(
        handler
            .dispatch(request(SimulatorCommand::StartSimulation))
            .ok
    );
    let response = handler.dispatch(request(SimulatorCommand::StepSimulation));
    assert!(response.ok, "{response:?}");
    let snapshot = handler.dispatch(request(SimulatorCommand::GetSimulationSnapshot));
    assert_eq!(
        snapshot
            .result
            .as_ref()
            .and_then(|value| value.get("environment"))
            .and_then(|value| value.get("visibility"))
            .and_then(Value::as_f64),
        Some(0.25)
    );
    let events = handler.dispatch(request(SimulatorCommand::GetSimulationEvents {
        cursor: Some(0),
    }));
    let events = events.result.expect("events");
    assert!(
        !events
            .get("events")
            .and_then(Value::as_array)
            .expect("event list")
            .iter()
            .any(|event| event.get("type") == Some(&json!("SimulationPluginFailure")))
    );
    let recording = RecordingStore::open(&database.to_string_lossy())
        .expect("open recording store")
        .load("run-smoke-in-cockpit")
        .expect("load recording");
    assert_eq!(recording.plugin_hashes.len(), 1);
    assert!(recording.plugin_hashes[0].starts_with("simulator-plugin@1.0.0:sha256:"));
    assert_eq!(recording.ticks[0].state_diffs.len(), 1);
    let _ = std::fs::remove_dir_all(directory);
    let _ = std::fs::remove_file(database);
}

#[cfg(unix)]
#[test]
fn simulator_discovers_and_runs_a_manifest_process_executor() {
    let directory = plugin_directory("manifest-process");
    let mut manifest = plugin_manifest();
    manifest.permissions.push(PluginPermission::ChildProcess);
    manifest.command = Some(vec![
        "sh".to_string(),
        "-c".to_string(),
        "cat >/dev/null; printf '[{\"pluginId\":\"simulator-plugin\",\"patch\":{\"kind\":\"cabinVisibility\",\"value\":0.4},\"expectedStateVersion\":0}]'".to_string(),
    ]);
    resign_manifest(&mut manifest);
    std::fs::write(
        directory.join("plugin.json"),
        serde_json::to_vec(&manifest).expect("manifest serializes"),
    )
    .expect("manifest writes");

    let mut handler = SimulatorHandler::new("session-1");
    assert!(
        handler
            .dispatch(request(SimulatorCommand::CreateSimulationRun {
                path: "scenarios/smoke-in-cockpit.yaml".to_string(),
            }))
            .ok
    );
    let policy = PluginPolicy {
        allowed_permissions: [
            PluginPermission::WorldRead,
            PluginPermission::WorldWrite,
            PluginPermission::ChildProcess,
        ]
        .into_iter()
        .collect(),
        require_signature: false,
        ..PluginPolicy::default()
    };
    assert!(
        handler
            .configure_plugins(&directory, policy, BTreeMap::new())
            .is_empty()
    );
    assert!(
        handler
            .dispatch(request(SimulatorCommand::StartSimulation))
            .ok
    );
    assert!(
        handler
            .dispatch(request(SimulatorCommand::StepSimulation))
            .ok
    );
    let snapshot = handler
        .dispatch(request(SimulatorCommand::GetSimulationSnapshot))
        .result
        .expect("snapshot");
    assert_eq!(
        snapshot
            .get("environment")
            .and_then(|environment| environment.get("visibility"))
            .and_then(Value::as_f64),
        Some(0.4)
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[cfg(unix)]
#[test]
fn simulator_records_process_plugin_deadline_evidence() {
    let directory = plugin_directory("manifest-timeout-evidence");
    let mut manifest = plugin_manifest();
    manifest.command = Some(vec![
        "sh".to_string(),
        "-c".to_string(),
        "while :; do :; done".to_string(),
    ]);
    resign_manifest(&mut manifest);
    std::fs::write(
        directory.join("plugin.json"),
        serde_json::to_vec(&manifest).expect("manifest serializes"),
    )
    .expect("manifest writes");
    let database = std::env::temp_dir().join(format!(
        "cockpit-simulator-plugin-timeout-{}.db",
        uuid::Uuid::new_v4()
    ));
    let mut handler = SimulatorHandler::new_persistent("session-1", &database.to_string_lossy())
        .expect("recording store");
    assert!(
        handler
            .dispatch(request(SimulatorCommand::CreateSimulationRun {
                path: "scenarios/smoke-in-cockpit.yaml".to_string(),
            }))
            .ok
    );
    let policy = PluginPolicy {
        allowed_permissions: [PluginPermission::WorldRead, PluginPermission::WorldWrite]
            .into_iter()
            .collect(),
        tick_budget_ms: Some(20),
        require_signature: false,
        ..PluginPolicy::default()
    };
    assert!(
        handler
            .configure_plugins(&directory, policy, BTreeMap::new())
            .is_empty()
    );
    assert!(
        handler
            .dispatch(request(SimulatorCommand::StartSimulation))
            .ok
    );
    assert!(
        handler
            .dispatch(request(SimulatorCommand::StepSimulation))
            .ok
    );
    let recording = RecordingStore::open(&database.to_string_lossy())
        .expect("open recording store")
        .load("run-smoke-in-cockpit")
        .expect("load recording");
    let execution = recording.ticks[0].plugin_failures[0]
        .execution
        .as_ref()
        .expect("process failure carries execution evidence");
    assert!(execution.timed_out);
    assert!(execution.terminated_process_group);
    assert!(execution.elapsed_ms >= 20);
    let _ = std::fs::remove_dir_all(directory);
    let _ = std::fs::remove_file(database);
}

#[test]
fn simulator_applies_pause_and_fail_plugin_policies() {
    for (policy, expected_status) in [
        (PluginFailurePolicy::PauseRun, "paused"),
        (PluginFailurePolicy::FailRun, "failed"),
    ] {
        let mut handler = SimulatorHandler::new("session-1");
        assert!(
            handler
                .dispatch(request(SimulatorCommand::CreateSimulationRun {
                    path: "scenarios/smoke-in-cockpit.yaml".to_string(),
                }))
                .ok
        );
        let directory = configure_simulator_plugin(
            &mut handler,
            expected_status,
            policy,
            Err("executor failed".to_string()),
        );
        assert!(
            handler
                .dispatch(request(SimulatorCommand::StartSimulation))
                .ok
        );
        let response = handler.dispatch(request(SimulatorCommand::StepSimulation));
        assert!(response.ok, "{response:?}");
        assert_eq!(
            response
                .result
                .as_ref()
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str),
            Some(expected_status)
        );
        let events = handler.dispatch(request(SimulatorCommand::GetSimulationEvents {
            cursor: Some(0),
        }));
        let events = events.result.expect("events");
        assert!(
            events
                .get("events")
                .and_then(Value::as_array)
                .expect("event list")
                .iter()
                .any(|event| event.get("type") == Some(&json!("SimulationPluginFailure")))
        );
        let _ = std::fs::remove_dir_all(directory);
    }
}

#[test]
fn simulator_disables_plugin_and_continues_after_plugin_failure() {
    let mut handler = SimulatorHandler::new("session-1");
    assert!(
        handler
            .dispatch(request(SimulatorCommand::CreateSimulationRun {
                path: "scenarios/smoke-in-cockpit.yaml".to_string(),
            }))
            .ok
    );
    let directory = configure_simulator_plugin(
        &mut handler,
        "disabled",
        PluginFailurePolicy::DisablePlugin,
        Err("executor failed".to_string()),
    );
    assert!(
        handler
            .dispatch(request(SimulatorCommand::StartSimulation))
            .ok
    );
    let first = handler.dispatch(request(SimulatorCommand::StepSimulation));
    assert!(first.ok, "{first:?}");
    let second = handler.dispatch(request(SimulatorCommand::StepSimulation));
    assert!(second.ok, "{second:?}");
    assert_eq!(
        second
            .result
            .as_ref()
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str),
        Some("running")
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn authenticated_ipc_controls_open_world_lifecycle() {
    let mut handler = SimulatorHandler::new("session-1");
    let created = handler.dispatch(request(SimulatorCommand::CreateSimulationRun {
        path: "scenarios/smoke-in-cockpit.yaml".to_string(),
    }));
    assert!(created.ok, "{created:?}");

    let mut guest = HumanState::new("ipc-guest-1");
    guest.goal = "find a safe seat".to_string();
    let spawned = handler.dispatch(request(SimulatorCommand::SpawnEntity {
        entity: DynamicEntity::Human(guest),
    }));
    assert!(spawned.ok, "{spawned:?}");

    let added = handler.dispatch(request(SimulatorCommand::AddAgentGoal {
        agent_id: "ipc-guest-1".to_string(),
        description: "observe the nearest safe exit".to_string(),
        priority: 7,
    }));
    assert!(added.ok, "{added:?}");
    let goal_id = added
        .result
        .as_ref()
        .and_then(|value| value.get("goalId"))
        .and_then(Value::as_str)
        .expect("goal id")
        .to_string();
    assert!(
        handler
            .dispatch(request(SimulatorCommand::SetAgentGoalStatus {
                agent_id: "ipc-guest-1".to_string(),
                goal_id,
                status: GoalStatus::Active,
            }))
            .ok
    );
    assert!(
        handler
            .dispatch(request(SimulatorCommand::WaitAgentUntil {
                agent_id: "ipc-guest-1".to_string(),
                wake_tick: 5,
            }))
            .ok
    );
    let runtime = handler.dispatch(request(SimulatorCommand::GetOpenWorldRuntime));
    assert_eq!(
        runtime
            .result
            .as_ref()
            .and_then(|value| value.pointer("/sessions/ipc-guest-1/wakeAtTick"))
            .and_then(Value::as_u64),
        Some(5)
    );
    let checkpoint = handler.dispatch(request(SimulatorCommand::CheckpointOpenWorld));
    assert!(checkpoint.ok, "{checkpoint:?}");
    assert_eq!(
        checkpoint
            .result
            .as_ref()
            .and_then(|value| value.get("agents"))
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        handler
            .dispatch(request(SimulatorCommand::RemoveEntity {
                entity_id: "ipc-guest-1".to_string(),
            }))
            .ok
    );
}
