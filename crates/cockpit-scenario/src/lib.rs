use std::{fs, path::Path};

use cockpit_simulation_core::{
    action::AgentGrant,
    clock::ClockConfig,
    error::{SimulationError, SimulationResult},
    influence::{ConflictPolicy, InfluenceRule},
    simulation::{
        EvaluationPolicy, EvaluationSpec, Fault, SimulationScenario, is_registered_evaluation_rule,
    },
    world::{AlarmState, CabinEnvironment, DeviceState, HumanState, OuterEnvironmentState},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const MAX_SCENARIO_BYTES: usize = 1_048_576;
pub const MAX_SCENARIO_ENTITIES: usize = 1_000;
pub const MAX_SCENARIO_FAULTS: usize = 10_000;
pub const MAX_SCENARIO_AGENTS: usize = 32;
pub const MAX_SCENARIO_EVALUATIONS: usize = 100;
pub const MAX_SCENARIO_IDENTIFIER_BYTES: usize = 128;
pub const MAX_AGENT_CAPABILITIES: usize = 64;
pub const MAX_SCENARIO_INFLUENCES: usize = 10_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioDocument {
    schema_version: u32,
    id: String,
    seed: u64,
    clock: ClockConfig,
    #[serde(default = "default_language")]
    language: String,
    entities: Vec<EntityDocument>,
    #[serde(default)]
    faults: Vec<FaultDocument>,
    agents: Vec<AgentDocument>,
    #[serde(default)]
    evaluation: Vec<EvaluationDocument>,
    #[serde(default)]
    influences: Vec<InfluenceRule>,
    #[serde(default)]
    conflict_policy: Option<ConflictPolicy>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityDocument {
    id: String,
    #[serde(rename = "type")]
    entity_type: String,
    #[serde(default)]
    components: serde_yaml::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FaultDocument {
    at_tick: u64,
    target: String,
    #[serde(rename = "type")]
    fault_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentDocument {
    id: String,
    #[allow(dead_code)]
    backend: String,
    #[allow(dead_code)]
    observation_profile: String,
    capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvaluationDocument {
    id: String,
    #[serde(default = "default_deadline")]
    deadline_tick: u64,
    #[allow(dead_code)]
    rule: String,
    #[serde(default)]
    policy: EvaluationPolicy,
}

fn default_deadline() -> u64 {
    30
}

fn default_language() -> String {
    "en".to_string()
}

pub fn load_scenario(path: impl AsRef<Path>) -> SimulationResult<SimulationScenario> {
    let bytes = fs::read(path.as_ref()).map_err(|err| {
        SimulationError::InvalidScenario(format!("failed to read scenario: {err}"))
    })?;
    parse_scenario_bytes(&bytes)
}

pub fn parse_scenario_bytes(bytes: &[u8]) -> SimulationResult<SimulationScenario> {
    if bytes.len() > MAX_SCENARIO_BYTES {
        return Err(SimulationError::InvalidScenario(format!(
            "scenario exceeds {MAX_SCENARIO_BYTES} byte limit"
        )));
    }
    let document: ScenarioDocument = serde_yaml::from_slice(bytes)
        .map_err(|err| SimulationError::InvalidScenario(format!("invalid YAML: {err}")))?;
    validate_document(&document)?;

    let mut outer_environment = OuterEnvironmentState::default();
    let mut environment = CabinEnvironment::default();
    let mut humans: Vec<HumanState> = Vec::new();
    let mut devices: Vec<DeviceState> = Vec::new();
    let alarm = AlarmState::default();

    for entity in &document.entities {
        match entity.entity_type.as_str() {
            "environment" if entity.id == "cabin" => {
                apply_environment_components(&mut environment, &entity.components)?
            }
            "outerEnvironment" => {
                apply_outer_environment_components(&mut outer_environment, &entity.components)?
            }
            "human" => {
                let mut human = HumanState::new(entity.id.clone());
                apply_human_components(&mut human, &entity.components)?;
                humans.push(human);
            }
            "device" => {
                let mut device = DeviceState::new(entity.id.clone());
                apply_device_components(&mut device, &entity.components)?;
                devices.push(device);
            }
            _ => {}
        }
    }

    let agent = document
        .agents
        .first()
        .ok_or_else(|| SimulationError::InvalidScenario("missing agent".to_string()))?;
    // Live runs are driven by one decision turn per human. Scenarios that do
    // not explicitly delegate action capabilities therefore grant the primary
    // human the primary cockpit-agent's scoped capabilities. Any explicit
    // human-level grant remains authoritative, preserving least privilege.
    if humans
        .iter()
        .all(|human| human.action_capabilities.is_empty())
    {
        let primary_human = humans.first_mut().ok_or_else(|| {
            SimulationError::InvalidScenario("missing at least one human entity".to_string())
        })?;
        primary_human.action_capabilities = agent.capabilities.clone();
    }
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let scenario_hash = format!("{:x}", hasher.finalize());
    let shutdown_deadline_ticks = document
        .evaluation
        .first()
        .map(|evaluation| evaluation.deadline_tick)
        .unwrap_or(30);
    let evaluation_rule_id = document
        .evaluation
        .first()
        .map(|evaluation| evaluation.id.clone());
    let evaluation_policy = document
        .evaluation
        .first()
        .map(|evaluation| evaluation.policy.clone())
        .unwrap_or_default();
    let evaluation_rules = document
        .evaluation
        .iter()
        .map(|evaluation| EvaluationSpec {
            id: evaluation.id.clone(),
            deadline_tick: evaluation.deadline_tick,
            policy: evaluation.policy.clone(),
        })
        .collect();

    Ok(SimulationScenario {
        id: document.id,
        schema_version: document.schema_version,
        scenario_hash,
        seed: document.seed,
        clock: document.clock,
        language: document.language,
        outer_environment,
        environment,
        humans,
        devices,
        alarm,
        faults: document
            .faults
            .into_iter()
            .map(|fault| Fault {
                at_tick: fault.at_tick,
                target: fault.target,
                fault_type: fault.fault_type,
            })
            .collect(),
        agent: AgentGrant {
            agent_id: agent.id.clone(),
            capabilities: agent.capabilities.clone(),
        },
        agents: document
            .agents
            .into_iter()
            .map(|agent| AgentGrant {
                agent_id: agent.id,
                capabilities: agent.capabilities,
            })
            .collect(),
        shutdown_deadline_ticks,
        evaluation_rule_id,
        evaluation_policy,
        evaluation_rules,
        influences: document.influences,
        conflict_policy: document
            .conflict_policy
            .unwrap_or(ConflictPolicy::RejectConflicting),
    })
}

fn validate_document(document: &ScenarioDocument) -> SimulationResult<()> {
    if document.schema_version != 1 {
        return Err(SimulationError::InvalidScenario(format!(
            "unsupported schemaVersion {}",
            document.schema_version
        )));
    }
    if document.clock.tick_ms == 0 {
        return Err(SimulationError::InvalidScenario(
            "clock.tickMs must be greater than zero".to_string(),
        ));
    }
    validate_limit("entities", document.entities.len(), MAX_SCENARIO_ENTITIES)?;
    validate_limit("faults", document.faults.len(), MAX_SCENARIO_FAULTS)?;
    validate_limit("agents", document.agents.len(), MAX_SCENARIO_AGENTS)?;
    validate_limit(
        "evaluation rules",
        document.evaluation.len(),
        MAX_SCENARIO_EVALUATIONS,
    )?;
    validate_identifier("scenario id", &document.id)?;
    let mut evaluation_ids = std::collections::BTreeSet::new();
    for evaluation in &document.evaluation {
        validate_identifier("evaluation rule id", &evaluation.id)?;
        if !is_registered_evaluation_rule(&evaluation.id) {
            return Err(SimulationError::InvalidScenario(format!(
                "evaluation rule '{}' is not registered",
                evaluation.id
            )));
        }
        if !evaluation_ids.insert(&evaluation.id) {
            return Err(SimulationError::InvalidScenario(format!(
                "evaluation rule '{}' is duplicated",
                evaluation.id
            )));
        }
        if evaluation.deadline_tick == 0 {
            return Err(SimulationError::InvalidScenario(format!(
                "evaluation rule '{}' has a zero deadlineTick",
                evaluation.id
            )));
        }
        for code in &evaluation.policy.safety_rejection_codes {
            if !is_known_safety_code(code) {
                return Err(SimulationError::InvalidScenario(format!(
                    "evaluation rule '{}' has unknown safety rejection code '{code}'",
                    evaluation.id
                )));
            }
        }
    }
    for entity in &document.entities {
        validate_identifier("entity id", &entity.id)?;
    }
    for fault in &document.faults {
        validate_identifier("fault target", &fault.target)?;
        validate_identifier("fault type", &fault.fault_type)?;
    }
    for agent in &document.agents {
        validate_identifier("agent id", &agent.id)?;
        validate_limit(
            "agent capabilities",
            agent.capabilities.len(),
            MAX_AGENT_CAPABILITIES,
        )?;
        for capability in &agent.capabilities {
            validate_identifier("agent capability", capability)?;
        }
    }
    if !document.entities.iter().any(|entity| entity.id == "cabin") {
        return Err(SimulationError::InvalidScenario(
            "missing cabin entity".to_string(),
        ));
    }
    if !document
        .entities
        .iter()
        .any(|entity| entity.entity_type == "human")
    {
        return Err(SimulationError::InvalidScenario(
            "missing at least one human entity".to_string(),
        ));
    }
    if !document
        .entities
        .iter()
        .any(|entity| entity.entity_type == "device" && entity.id == "engine-1")
    {
        return Err(SimulationError::InvalidScenario(
            "missing engine-1 entity".to_string(),
        ));
    }
    if document.agents.is_empty() {
        return Err(SimulationError::InvalidScenario(
            "missing agents".to_string(),
        ));
    }
    validate_limit(
        "influences",
        document.influences.len(),
        MAX_SCENARIO_INFLUENCES,
    )?;
    for influence in &document.influences {
        validate_identifier("influence rule id", &influence.rule_id)?;
        validate_identifier("influence entity id", &influence.entity_id)?;
        if !is_writable_component(&influence.entity_id, &influence.component_path) {
            return Err(SimulationError::InvalidScenario(format!(
                "influence rule '{}' targets unknown component {}::{}",
                influence.rule_id, influence.entity_id, influence.component_path
            )));
        }
        if let cockpit_simulation_core::influence::InfluenceSchedule::Every { interval, .. } =
            influence.schedule
            && interval == 0
        {
            return Err(SimulationError::InvalidScenario(format!(
                "influence rule '{}' has a zero interval",
                influence.rule_id
            )));
        }
    }
    Ok(())
}

fn is_known_safety_code(code: &str) -> bool {
    matches!(
        code,
        "CAPABILITY_DENIED"
            | "DEVICE_UNPOWERED"
            | "PRECONDITION_FAILED"
            | "STATE_VERSION_CONFLICT"
            | "ACTION_EXPIRED"
            | "ACTION_CONFLICT"
            | "UNKNOWN_TARGET"
            | "APPROVAL_DENIED"
            | "ACTION_CANCELLED"
            | "TOOL_CALL_DENIED"
    )
}

/// Component paths that scheduled influences may target, mirroring the writable
/// StateDiff surface in the simulation core. Human component paths are accepted
/// for any human id since the entity set is scenario-defined.
fn is_writable_component(entity_id: &str, component_path: &str) -> bool {
    matches!(component_path, "pilot.stress" | "pilot.attention")
        || matches!(
            (entity_id, component_path),
            ("cabin", "environment.smokeDensity")
                | ("cabin", "environment.visibility")
                | ("cabin", "environment.temperatureC")
                | ("engine-1", "engine.health")
                | ("alarm-1", "alarm.active")
        )
}

fn validate_limit(name: &str, actual: usize, limit: usize) -> SimulationResult<()> {
    if actual <= limit {
        Ok(())
    } else {
        Err(SimulationError::InvalidScenario(format!(
            "{name} exceeds {limit} item limit"
        )))
    }
}

fn validate_identifier(name: &str, value: &str) -> SimulationResult<()> {
    if value.is_empty() || value.len() > MAX_SCENARIO_IDENTIFIER_BYTES {
        return Err(SimulationError::InvalidScenario(format!(
            "{name} must be 1..={MAX_SCENARIO_IDENTIFIER_BYTES} bytes"
        )));
    }
    Ok(())
}

fn apply_environment_components(
    environment: &mut CabinEnvironment,
    components: &serde_yaml::Value,
) -> SimulationResult<()> {
    if let Some(smoke) = lookup(components, "smoke", "density") {
        environment.smoke_density = smoke;
    }
    if let Some(temperature) = lookup(components, "temperature", "celsius") {
        environment.temperature_c = temperature;
    }
    ensure_range("smoke.density", environment.smoke_density, 0.0, 3.0)?;
    Ok(())
}

fn apply_outer_environment_components(
    outer: &mut OuterEnvironmentState,
    components: &serde_yaml::Value,
) -> SimulationResult<()> {
    if let Some(temperature) = lookup(components, "temperature", "celsius") {
        outer.external_temperature_c = temperature;
    }
    if let Some(altitude) = lookup(components, "altitude", "meters") {
        outer.altitude_m = altitude;
    }
    if let Some(wind) = lookup(components, "wind", "speedKmh") {
        outer.wind_speed_kmh = wind;
    }
    if let Some(precipitation) = lookup(components, "weather", "precipitation") {
        outer.precipitation = precipitation;
    }
    Ok(())
}

fn apply_human_components(
    human: &mut HumanState,
    components: &serde_yaml::Value,
) -> SimulationResult<()> {
    if let Some(attention) = lookup(components, "attention", "value") {
        human.attention = attention;
    }
    if let Some(location) = scalar_string(components, "location") {
        human.location = location;
    }
    if let Some(name) = scalar_string(components, "name") {
        human.persona.name = name;
    }
    if let Some(role) = scalar_string(components, "role") {
        human.persona.role = role;
    }
    if let Some(background) = scalar_string(components, "background") {
        human.persona.background = background;
    }
    if let Some(capabilities) = sequence_strings(components, "actionCapabilities") {
        human.action_capabilities = capabilities;
    }
    if let Some(relationships) = sequence_strings(components, "relationships") {
        human.persona.relationships = relationships;
    }
    let mut traits = human.persona.traits;
    if let Some(value) = lookup(components, "traits", "openness") {
        traits.openness = value;
    }
    if let Some(value) = lookup(components, "traits", "conscientiousness") {
        traits.conscientiousness = value;
    }
    if let Some(value) = lookup(components, "traits", "extraversion") {
        traits.extraversion = value;
    }
    if let Some(value) = lookup(components, "traits", "agreeableness") {
        traits.agreeableness = value;
    }
    if let Some(value) = lookup(components, "traits", "neuroticism") {
        traits.neuroticism = value;
    }
    human.persona.traits = traits;

    ensure_range("attention.value", human.attention, 0.0, 1.0)?;
    for (name, value) in [
        ("traits.openness", traits.openness),
        ("traits.conscientiousness", traits.conscientiousness),
        ("traits.extraversion", traits.extraversion),
        ("traits.agreeableness", traits.agreeableness),
        ("traits.neuroticism", traits.neuroticism),
    ] {
        ensure_range(name, value, 0.0, 1.0)?;
    }
    Ok(())
}

fn apply_device_components(
    device: &mut DeviceState,
    components: &serde_yaml::Value,
) -> SimulationResult<()> {
    if let Some(capabilities) = sequence_strings(components, "capabilities") {
        device.capabilities = capabilities;
    }
    if device.id == "engine-1"
        && !device
            .capabilities
            .iter()
            .any(|capability| capability == "shutdown")
    {
        return Err(SimulationError::InvalidScenario(
            "engine-1 must define shutdown capability".to_string(),
        ));
    }
    Ok(())
}

fn lookup(components: &serde_yaml::Value, component: &str, field: &str) -> Option<f64> {
    components.get(component)?.get(field)?.as_f64()
}

fn scalar_string(components: &serde_yaml::Value, field: &str) -> Option<String> {
    components.get(field)?.as_str().map(ToString::to_string)
}

fn sequence_strings(components: &serde_yaml::Value, field: &str) -> Option<Vec<String>> {
    let sequence = components.get(field)?.as_sequence()?;
    Some(
        sequence
            .iter()
            .filter_map(|value| value.as_str().map(ToString::to_string))
            .collect(),
    )
}

fn ensure_range(name: &str, value: f64, min: f64, max: f64) -> SimulationResult<()> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(SimulationError::InvalidScenario(format!(
            "{name} must be in range {min}..={max}"
        )))
    }
}
