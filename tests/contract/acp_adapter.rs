use cockpit_agent_runtime::{
    HumanTurnContext, acp_adapter::IotaCoreAcpAdapter, iota_core_adapter::IotaCoreAdapter,
};
use cockpit_scenario::load_scenario;
use cockpit_simulation_core::Simulation;

fn context_for_primary_human(simulation: &Simulation) -> HumanTurnContext {
    let human = simulation.snapshot.primary_human().clone();
    HumanTurnContext {
        human_id: human.id.clone(),
        observation: simulation.observation(),
        persona: human.persona.clone(),
        needs: human.needs,
        goal: human.goal.clone(),
        delivered_perception: human.short_term_memory.clone(),
        long_term_memory: human.long_term_memory.clone(),
        action_capabilities: human.action_capabilities.clone(),
        language: simulation.scenario.language.clone(),
    }
}

#[test]
fn acp_prompt_contains_persona_and_only_perceived_observation_data() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("acp-contract-run", scenario);
    let skill = IotaCoreAdapter::new(env!("CARGO_MANIFEST_DIR"))
        .load_cockpit_skill()
        .expect("skill loads");
    let context = context_for_primary_human(&simulation);
    let prompt = IotaCoreAcpAdapter::build_prompt(&context, &skill);

    // Persona and skill instructions are present (persona-aware, resource-driven).
    assert!(prompt.contains("Never request or infer Ground Truth"));
    assert!(prompt.contains(&context.persona.name));
    assert!(prompt.contains("Personality (Big Five"));
    assert!(prompt.contains("visibleEntities"));
    assert!(prompt.contains("confidence"));

    // Ground Truth must never leak into the prompt.
    assert!(!prompt.contains("smokeDensity"));
    assert!(!prompt.contains("fireActive"));
}

#[test]
fn acp_prompt_exposes_authorized_tools_without_leaking_the_benchmark_action() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("acp-required-action", scenario);
    let skill = IotaCoreAdapter::new(env!("CARGO_MANIFEST_DIR"))
        .load_cockpit_skill()
        .expect("skill loads");
    let mut context = context_for_primary_human(&simulation);
    context.observation.alerts = vec!["SmokeDetected".to_string()];

    let prompt = IotaCoreAcpAdapter::build_prompt(&context, &skill);

    assert!(prompt.contains("- engineShutdown -> engine-1"));
    assert!(!prompt.contains("SmokeDetected: engineShutdown -> engine-1"));
    assert!(!prompt.contains("include every listed action in the actions array this turn"));
}
