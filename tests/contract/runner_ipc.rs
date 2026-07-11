use cockpit_recording::run_scripted_recording;
use cockpit_runner::ipc::{
    RunnerHandler,
    proto::{IPC_VERSION, RunnerCommand, RunnerRequest},
};
use cockpit_scenario::load_scenario;
use serde_json::Value;

fn request(command: RunnerCommand) -> RunnerRequest {
    RunnerRequest {
        version: IPC_VERSION,
        session_token: "session-1".to_string(),
        correlation_id: "contract-correlation".to_string(),
        command,
    }
}

#[test]
fn runner_requires_version_and_session_token() {
    let mut handler = RunnerHandler::new("session-1");
    let mut invalid = request(RunnerCommand::GetSimulationSnapshot);
    invalid.version = IPC_VERSION + 1;
    let response = handler.dispatch(invalid);
    assert_eq!(
        response.error.as_ref().map(|error| error.code.as_str()),
        Some("IPC_VERSION_UNSUPPORTED")
    );

    let mut unauthorized = request(RunnerCommand::GetSimulationSnapshot);
    unauthorized.session_token = "wrong".to_string();
    let response = handler.dispatch(unauthorized);
    assert_eq!(
        response.error.as_ref().map(|error| error.code.as_str()),
        Some("SESSION_UNAUTHORIZED")
    );
}

#[test]
fn runner_step_emits_snapshot_trace_evaluation_and_cursored_events() {
    let mut handler = RunnerHandler::new("session-1");
    let response = handler.dispatch(request(RunnerCommand::CreateSimulationRun {
        path: "scenarios/smoke-in-cockpit.yaml".to_string(),
    }));
    assert!(response.ok, "{response:?}");
    let response = handler.dispatch(request(RunnerCommand::StartSimulation));
    assert!(response.ok, "{response:?}");

    for _ in 0..10 {
        let response = handler.dispatch(request(RunnerCommand::StepSimulation));
        assert!(response.ok, "{response:?}");
    }

    let response = handler.dispatch(request(RunnerCommand::GetSimulationEvents {
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
        == Some(&Value::String("SimulationEvaluationUpdated".to_string()))));

    let cursor = events
        .last()
        .and_then(|event| event.get("cursor"))
        .and_then(Value::as_u64)
        .expect("cursor");
    let response = handler.dispatch(request(RunnerCommand::GetSimulationEvents {
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
fn runner_replay_emits_real_snapshots_and_terminal_state() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario");
    let recording = run_scripted_recording("source-run", scenario, 10).expect("recording");
    let path = std::env::temp_dir().join(format!("cockpit-replay-{}.json", uuid::Uuid::new_v4()));
    std::fs::write(
        &path,
        serde_json::to_vec(&recording).expect("recording json"),
    )
    .expect("recording file");

    let mut handler = RunnerHandler::new("session-1");
    let response = handler.dispatch(request(RunnerCommand::StartReplay {
        scenario_path: "scenarios/smoke-in-cockpit.yaml".to_string(),
        recording_path: path.to_string_lossy().to_string(),
    }));
    assert!(response.ok, "{response:?}");
    let events = handler.dispatch(request(RunnerCommand::GetSimulationEvents {
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
                .get("snapshot")
                .and_then(|snapshot| snapshot.get("tick"))
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
fn runner_exposes_recording_diff_report() {
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

    let mut handler = RunnerHandler::new("session-1");
    let response = handler.dispatch(request(RunnerCommand::DiffRecordings {
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
