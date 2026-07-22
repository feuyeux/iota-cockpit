use cockpit_evaluation::evaluate_smoke_shutdown;
use std::collections::BTreeMap;

use cockpit_agent::{LocalMcpServer, RuleAgent, RulePolicy, RulePolicyAction};
use cockpit_recording::run_rule_agent_recording;
use cockpit_scenario::load_scenario;
use cockpit_world::Simulation;

#[test]
fn rule_agent_uses_mcp_boundary_to_shutdown_engine() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let deadline = scenario.max_ticks;
    let recording =
        run_rule_agent_recording("rule-agent-run", scenario, 80).expect("run completes");
    let evaluation = evaluate_smoke_shutdown(&recording, deadline);

    assert!(evaluation.passed, "{evaluation:?}");
    assert!(recording.ticks.iter().any(|tick| {
        tick.tool_calls
            .iter()
            .any(|call| call.tool_name == "simulation.request_action")
    }));
    assert!(recording.ticks.iter().all(|tick| {
        tick.tool_calls.iter().all(|call| {
            let serialized = call.result.to_string();
            !serialized.contains("smokeDensity") && !serialized.contains("fireActive")
        })
    }));
    assert_eq!(
        recording.provenance.rule_policy_hash,
        Some(RulePolicy::default().hash())
    );
}

#[test]
fn rule_policy_rejects_unknown_capability_before_a_tick_is_committed() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let mut simulation = Simulation::new("invalid-rule-policy", scenario);
    simulation.start().expect("run starts");
    let mut agent = RuleAgent::new(RulePolicy {
        version: 1,
        responses: BTreeMap::from([(
            "SmokeDetected".to_string(),
            RulePolicyAction {
                target: "engine-1".to_string(),
                command: "notARealCapability".to_string(),
            },
        )]),
    });
    let error = agent
        .step(&mut simulation, &mut LocalMcpServer::default())
        .expect_err("invalid policy must fail before tool dispatch");
    assert!(error.to_string().contains("not in the capability catalog"));
    assert_eq!(simulation.snapshot.tick, 0);
}

#[test]
fn rule_policy_fixture_loads_with_a_stable_content_hash() {
    let path =
        std::env::temp_dir().join(format!("cockpit-rule-policy-{}.json", uuid::Uuid::new_v4()));
    let policy = RulePolicy::default();
    std::fs::write(
        &path,
        serde_json::to_vec(&policy).expect("policy serializes"),
    )
    .expect("fixture writes");
    let loaded = RulePolicy::from_file(&path).expect("fixture loads");
    assert_eq!(loaded, policy);
    assert_eq!(loaded.hash(), policy.hash());
    let missing =
        RulePolicy::from_file(path.with_extension("missing")).expect_err("missing fixture fails");
    assert!(missing.contains("failed to read RulePolicy"));
    let _ = std::fs::remove_file(path);
}
