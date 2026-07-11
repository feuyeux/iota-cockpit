use std::path::PathBuf;

use cockpit_runner::{
    RunnerHandler,
    ipc::proto::{IPC_VERSION, RunnerCommand, RunnerRequest},
};
use serde_json::Value;

fn request(command: RunnerCommand) -> RunnerRequest {
    RunnerRequest {
        version: IPC_VERSION,
        session_token: "restart-token".to_string(),
        correlation_id: "restart-correlation".to_string(),
        command,
    }
}

#[test]
fn persistent_handler_restores_snapshot_and_event_cursor_after_restart() {
    let database =
        std::env::temp_dir().join(format!("cockpit-restart-{}.sqlite", uuid::Uuid::new_v4()));
    let database_path = database.to_string_lossy().to_string();
    let mut first =
        RunnerHandler::new_persistent("restart-token", &database_path).expect("first handler");
    assert!(
        first
            .dispatch(request(RunnerCommand::CreateSimulationRun {
                path: "scenarios/smoke-in-cockpit.yaml".to_string(),
            }))
            .ok
    );
    assert!(first.dispatch(request(RunnerCommand::StartSimulation)).ok);
    for _ in 0..10 {
        assert!(first.dispatch(request(RunnerCommand::StepSimulation)).ok);
    }
    let snapshot_before = first.dispatch(request(RunnerCommand::GetSimulationSnapshot));
    let tick_before = snapshot_before
        .result
        .as_ref()
        .and_then(|value| value.get("tick"))
        .and_then(Value::as_u64)
        .expect("snapshot tick");
    drop(first);

    let mut second =
        RunnerHandler::new_persistent("restart-token", &database_path).expect("second handler");
    let resumed = second.dispatch(request(RunnerCommand::ResumeSimulation {
        scenario_path: "scenarios/smoke-in-cockpit.yaml".to_string(),
        run_id: "run-smoke-in-cockpit".to_string(),
    }));
    assert!(resumed.ok, "{resumed:?}");
    assert_eq!(
        resumed
            .result
            .as_ref()
            .and_then(|value| value.get("tick"))
            .and_then(Value::as_u64),
        Some(tick_before)
    );

    let events = second.dispatch(request(RunnerCommand::GetSimulationEvents {
        cursor: Some(0),
    }));
    assert!(events.ok, "{events:?}");
    let count = events
        .result
        .as_ref()
        .and_then(|value| value.get("events"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    assert!(count > 0);

    let _ = std::fs::remove_file(&database);
    let payloads = PathBuf::from(format!("{database_path}.payloads"));
    let _ = std::fs::remove_dir_all(payloads);
}
