use cockpit_evaluation::evaluate_smoke_shutdown;
use cockpit_recording::{replay_recording, run_scripted_recording};
use cockpit_scenario::load_scenario;

#[test]
fn smoke_scenario_records_replays_and_evaluates_deterministically() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let deadline = scenario.shutdown_deadline_ticks;

    let first = run_scripted_recording("smoke-run-1", scenario.clone(), 80).expect("first run");
    let second = run_scripted_recording("smoke-run-2", scenario.clone(), 80).expect("second run");
    let replay = replay_recording("smoke-replay-1", scenario, &first).expect("replay");

    assert_eq!(first.final_snapshot_hash(), second.final_snapshot_hash());
    assert_eq!(first.final_snapshot_hash(), replay.final_snapshot_hash());

    let evaluation = evaluate_smoke_shutdown(&first, deadline);
    assert!(evaluation.passed, "{evaluation:?}");

    let leaked_ground_truth = first.ticks.iter().any(|tick| {
        serde_json::to_value(&tick.observation)
            .unwrap()
            .get("smokeDensity")
            .is_some()
    });
    assert!(
        !leaked_ground_truth,
        "observation leaked ground truth smoke density"
    );

    let smoke_tick = first
        .ticks
        .iter()
        .flat_map(|tick| &tick.events)
        .find(|event| event.event_type == "SmokeDetected")
        .expect("smoke detected")
        .tick;
    let shutdown_tick = first
        .ticks
        .iter()
        .flat_map(|tick| &tick.events)
        .find(|event| event.event_type == "EngineShutdown")
        .expect("engine shutdown")
        .tick;
    assert!(shutdown_tick <= smoke_tick + deadline);
}
