use cockpit_evaluation::evaluate;
use cockpit_recording::run_rule_agent_recording;
use cockpit_scenario::load_scenario;

const BENCHMARKS: &[(&str, &str)] = &[
    ("scenarios/heatwave-thermal-comfort.yaml", "hvac-1"),
    ("scenarios/winter-defog-visibility.yaml", "defogger-1"),
    ("scenarios/driver-fatigue-guardian.yaml", "dms-1"),
    ("scenarios/child-left-behind.yaml", "occupant-radar-1"),
    ("scenarios/medical-emergency.yaml", "emergency-call-1"),
    ("scenarios/voice-privacy-conflict.yaml", "voice-array-1"),
    ("scenarios/ev-range-anxiety.yaml", "navigation-1"),
    (
        "scenarios/adas-takeover-construction.yaml",
        "adas-controller-1",
    ),
    (
        "scenarios/cybersecurity-anomalous-control.yaml",
        "security-monitor-1",
    ),
];

#[test]
fn every_non_smoke_benchmark_has_passing_traceable_evidence() {
    for (path, evidence_source) in BENCHMARKS {
        let scenario = load_scenario(path).unwrap_or_else(|error| panic!("{path}: {error}"));
        let deadline = scenario.shutdown_deadline_ticks;
        let rule_id = scenario
            .evaluation_rule_id
            .clone()
            .unwrap_or_else(|| panic!("{path}: missing evaluation rule"));
        assert_ne!(
            rule_id, "shutdown-before-spread",
            "{path} uses smoke evaluator"
        );

        let recording = run_rule_agent_recording(
            format!("benchmark-{}", scenario.id),
            scenario.clone(),
            deadline + 1,
        )
        .unwrap_or_else(|error| panic!("{path}: {error}"));
        let result = evaluate(&recording, Some(&rule_id), deadline, &scenario.language);

        assert!(result.passed, "{path}: {}", result.explanation);
        assert_eq!(result.score, 1.0, "{path}");
        assert!(!result.evidence_event_ids.is_empty(), "{path}: no evidence");
        assert!(
            recording
                .ticks
                .iter()
                .flat_map(|tick| &tick.events)
                .any(|event| {
                    event.source == *evidence_source
                        && result.evidence_event_ids.contains(&event.event_id)
                }),
            "{path}: evaluation evidence does not point to {evidence_source}"
        );
    }
}

#[test]
fn benchmark_evaluation_fails_when_required_evidence_is_removed() {
    let path = "scenarios/driver-fatigue-guardian.yaml";
    let scenario = load_scenario(path).expect("scenario loads");
    let deadline = scenario.shutdown_deadline_ticks;
    let rule_id = scenario.evaluation_rule_id.clone().expect("rule id");
    let mut recording = run_rule_agent_recording("missing-evidence", scenario, deadline + 1)
        .expect("recording runs");

    for tick in &mut recording.ticks {
        tick.events.retain(|event| event.source != "dms-1");
    }

    let result = evaluate(&recording, Some(&rule_id), deadline, "en");
    assert!(!result.passed);
    assert_eq!(result.score, 0.2);
    assert!(result.evidence_event_ids.is_empty());
    assert_eq!(result.first_failure_tick, Some(deadline));
}

#[test]
fn benchmark_explanations_follow_scenario_language() {
    let path = "scenarios/child-left-behind.yaml";
    let scenario = load_scenario(path).expect("scenario loads");
    let deadline = scenario.shutdown_deadline_ticks;
    let rule_id = scenario.evaluation_rule_id.clone().expect("rule id");
    let recording =
        run_rule_agent_recording("localized-evaluation", scenario.clone(), deadline + 1)
            .expect("recording runs");

    let result = evaluate(&recording, Some(&rule_id), deadline, &scenario.language);
    assert!(result.passed);
    assert_eq!(result.explanation, "儿童保护降温在截止时间前生效");
}
