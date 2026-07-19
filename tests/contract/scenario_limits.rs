use cockpit_scenario::{MAX_SCENARIO_BYTES, MAX_SCENARIO_ENTITIES, parse_scenario_bytes};

const BASE: &str = r#"
schemaVersion: 1
id: bounded-scenario
seed: 1
clock: { mode: stepped, tickMs: 100 }
entities:
  - { id: cabin, type: environment }
  - { id: engine-1, type: device, components: { capabilities: [shutdown] } }
  - { id: operator-1, type: human, components: { location: cockpit } }
agents:
  - { id: agent, backend: scripted, observationProfile: default, capabilities: [engine.shutdown] }
"#;

#[test]
fn scenario_parser_rejects_oversized_documents_before_deserialization() {
    let mut bytes = BASE.as_bytes().to_vec();
    bytes.resize(MAX_SCENARIO_BYTES + 1, b' ');
    let error = parse_scenario_bytes(&bytes).expect_err("oversized scenario must fail");
    assert!(error.to_string().contains("byte limit"));
}

#[test]
fn scenario_parser_rejects_excessive_entities() {
    let entities = (0..=MAX_SCENARIO_ENTITIES)
        .map(|index| format!("  - {{ id: extra-{index}, type: environment }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = BASE.replace("agents:", &format!("{entities}\nagents:"));
    let error = parse_scenario_bytes(source.as_bytes()).expect_err("too many entities must fail");
    assert!(error.to_string().contains("entities exceeds"));
}

#[test]
fn scenario_parser_rejects_private_evaluation_contracts() {
    let source =
        format!("{BASE}\nevaluation: [{{ id: shutdown-before-spread, deadlineTick: 10 }}]\n");
    let error = parse_scenario_bytes(source.as_bytes())
        .expect_err("private evaluation contracts must not be accepted in scenario YAML");
    assert!(error.to_string().contains("unknown field `evaluation`"));
}
