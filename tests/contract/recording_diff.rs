use cockpit_recording::{diff_recordings, replay_recording, run_scripted_recording};
use cockpit_scenario::load_scenario;

#[test]
fn recording_diff_ignores_run_specific_identifiers_for_equivalent_replay() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario");
    let source = run_scripted_recording("source-run", scenario.clone(), 20).expect("source");
    let replay = replay_recording("replay-run", scenario, &source).expect("replay");
    let report = diff_recordings(&source, &replay);
    assert!(report.equivalent, "{report:?}");
    assert!(report.tick_differences.is_empty());
}

#[test]
fn recording_diff_reports_first_changed_tick_and_evidence_category() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario");
    let source = run_scripted_recording("source-run", scenario, 20).expect("source");
    let mut candidate = source.clone();
    candidate.ticks[5].events.clear();
    let report = diff_recordings(&source, &candidate);
    assert!(!report.equivalent);
    assert_eq!(
        report.first_divergence.as_ref().map(|diff| diff.tick),
        Some(5)
    );
    assert_eq!(
        report
            .first_divergence
            .as_ref()
            .map(|diff| diff.events_match),
        Some(false)
    );
}
