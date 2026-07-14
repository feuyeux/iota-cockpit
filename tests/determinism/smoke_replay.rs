use cockpit_evaluation::evaluate_smoke_shutdown;
use cockpit_recording::{
    Recording, replay_recording, run_rule_agent_recording, run_scripted_recording,
};
use cockpit_scenario::load_scenario;
use cockpit_simulation_core::{Simulation, StateDiff};
use serde_json::json;

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

#[test]
fn committed_state_diffs_are_audited_and_replayed_deterministically() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let mut simulation = Simulation::new("state-diff-run", scenario.clone());
    simulation.start().expect("run starts");
    let diff = StateDiff {
        source_id: "smoke-plugin".to_string(),
        entity_id: "cabin".to_string(),
        component_path: "environment.visibility".to_string(),
        value: json!(0.4),
        expected_state_version: 0,
    };
    let step = simulation
        .step_with_state_diffs(vec![diff])
        .expect("state diff commits");
    assert_eq!(simulation.snapshot.environment.visibility, 0.4);
    assert!(
        step.events
            .iter()
            .any(|event| event.event_type == "StateDiffApplied")
    );

    let mut recording = Recording::new("state-diff-run", &scenario);
    recording.push(step);
    let replay = replay_recording("state-diff-replay", scenario, &recording).expect("replay");
    assert_eq!(
        recording.final_snapshot_hash(),
        replay.final_snapshot_hash()
    );
}

#[test]
fn cockpit_system_state_is_included_in_deterministic_replay() {
    let scenario =
        load_scenario("scenarios/heatwave-thermal-comfort.yaml").expect("scenario loads");
    let recording =
        run_rule_agent_recording("domain-state-run", scenario.clone(), 30).expect("rule run");
    let replay = replay_recording("domain-state-replay", scenario, &recording).expect("replay");

    assert_eq!(
        recording.final_snapshot_hash(),
        replay.final_snapshot_hash()
    );
    assert!(
        recording
            .ticks
            .iter()
            .flat_map(|tick| &tick.events)
            .any(|event| event.event_type == "ThermalComfortRestored")
    );
    assert!(
        replay
            .ticks
            .iter()
            .flat_map(|tick| &tick.events)
            .any(|event| event.event_type == "ThermalComfortRestored")
    );
}
