use cockpit_evaluation::{evaluate_scenario, evaluate_with_policy, mark_execution_failed};
use cockpit_recording::run_rule_agent_recording;
use cockpit_scenario::{load_scenario, parse_scenario_bytes};
use cockpit_simulation_core::{
    action::{ActionStatus, ErrorCode},
    event::ToolCallTrace,
    simulation::{EvaluationPolicy, EvaluationSpec},
};
use serde_json::json;

#[test]
fn safety_rejection_gates_an_otherwise_successful_task() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario");
    let deadline = scenario.shutdown_deadline_ticks;
    let mut recording = run_rule_agent_recording("unsafe-success", scenario.clone(), deadline + 1)
        .expect("recording");
    let action = recording
        .ticks
        .iter_mut()
        .flat_map(|tick| &mut tick.action_results)
        .next()
        .expect("rule agent submitted an action");
    action.status = ActionStatus::Rejected;
    action.error_code = Some(ErrorCode::CapabilityDenied);

    let result = evaluate_with_policy(
        &recording,
        scenario.evaluation_rule_id.as_deref(),
        deadline,
        &scenario.language,
        &scenario.evaluation_policy,
    );
    assert!(result.task_passed, "world state still reached the goal");
    assert!(!result.passed, "safety gate overrides task success");
    assert!(!result.safety_passed);
    assert_eq!(result.score, 0.0);
    assert_eq!(result.safety_violations[0].code, "CAPABILITY_DENIED");
}

#[test]
fn trajectory_budget_is_an_independent_gate() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario");
    let deadline = scenario.shutdown_deadline_ticks;
    let recording = run_rule_agent_recording("budget-success", scenario.clone(), deadline + 1)
        .expect("recording");
    let policy = EvaluationPolicy {
        max_action_requests: Some(0),
        ..scenario.evaluation_policy.clone()
    };
    let result = evaluate_with_policy(
        &recording,
        scenario.evaluation_rule_id.as_deref(),
        deadline,
        &scenario.language,
        &policy,
    );
    assert!(result.task_passed);
    assert!(!result.trajectory_passed);
    assert!(!result.passed);
    assert_eq!(result.trajectory.action_requests, 1);
}

#[test]
fn denied_side_effect_tool_call_is_a_safety_failure() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario");
    let deadline = scenario.shutdown_deadline_ticks;
    let mut recording =
        run_rule_agent_recording("denied-tool", scenario.clone(), deadline + 1).expect("recording");
    recording.ticks[0].tool_calls.push(ToolCallTrace {
        call_id: "denied-action".to_string(),
        tool_name: "simulation.request_action".to_string(),
        run_id: recording.run_id.clone(),
        agent_id: "unauthorized-agent".to_string(),
        tick: 0,
        correlation_id: "denied-tool".to_string(),
        arguments: json!({}),
        result: json!({}),
        side_effect: true,
        allowed: false,
    });
    let result = evaluate_with_policy(
        &recording,
        scenario.evaluation_rule_id.as_deref(),
        deadline,
        &scenario.language,
        &scenario.evaluation_policy,
    );
    assert!(!result.safety_passed);
    assert!(
        result
            .safety_violations
            .iter()
            .any(|violation| violation.code == "TOOL_CALL_DENIED")
    );
}

#[test]
fn multi_rule_scenario_reports_every_rule_and_requires_all_to_pass() {
    let mut scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario");
    scenario.evaluation_rules.push(EvaluationSpec {
        id: "thermal-comfort-restored".to_string(),
        deadline_tick: scenario.shutdown_deadline_ticks,
        policy: scenario.evaluation_policy.clone(),
    });
    let recording = run_rule_agent_recording(
        "multiple-rules",
        scenario.clone(),
        scenario.shutdown_deadline_ticks + 1,
    )
    .expect("recording");
    let result = evaluate_scenario(&recording, &scenario);
    assert_eq!(result.rule_results.len(), 2);
    assert!(!result.passed, "the unmet second rule gates the aggregate");
    assert_eq!(result.trajectory.action_requests, 1);
    assert_eq!(result.rule_results[0].rule_id, "shutdown-before-spread");
    assert_eq!(result.rule_results[1].rule_id, "thermal-comfort-restored");
}

#[test]
fn multi_rule_summary_uses_the_scenario_language() {
    let mut scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario");
    scenario.language = "zh-CN".to_string();
    scenario.evaluation_rules.push(EvaluationSpec {
        id: "thermal-comfort-restored".to_string(),
        deadline_tick: scenario.shutdown_deadline_ticks,
        policy: scenario.evaluation_policy.clone(),
    });
    let recording = run_rule_agent_recording(
        "localized-multiple-rules",
        scenario.clone(),
        scenario.shutdown_deadline_ticks + 1,
    )
    .expect("recording");
    assert_eq!(
        evaluate_scenario(&recording, &scenario).explanation,
        "2 条评测规则中通过 1 条"
    );
}

#[test]
fn execution_failure_gates_a_completed_task() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario");
    let recording = run_rule_agent_recording(
        "execution-failure",
        scenario.clone(),
        scenario.shutdown_deadline_ticks + 1,
    )
    .expect("recording");
    let result = mark_execution_failed(evaluate_scenario(&recording, &scenario), "backend timeout");
    assert!(result.task_passed);
    assert!(!result.execution_passed);
    assert!(!result.passed);
    assert_eq!(result.execution_error.as_deref(), Some("backend timeout"));
}

#[test]
fn scenario_parser_preserves_all_declared_evaluation_rules() {
    let source =
        std::fs::read_to_string("scenarios/smoke-in-cockpit.yaml").expect("scenario source");
    let scenario = parse_scenario_bytes(
        format!(
            "{source}\n  - id: thermal-comfort-restored\n    deadlineTick: 30\n    rule: second objective\n"
        )
        .as_bytes(),
    )
    .expect("multi-rule scenario parses");
    assert_eq!(scenario.evaluation_rules.len(), 2);
    assert_eq!(scenario.evaluation_rules[1].id, "thermal-comfort-restored");
}
