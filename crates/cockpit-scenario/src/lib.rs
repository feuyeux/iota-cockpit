use std::{collections::HashSet, fs, io::Read, path::Path};

use cockpit_world::{
    action::AgentGrant,
    capability::CapabilityCatalog,
    clock::ClockConfig,
    error::{SimulationError, SimulationResult},
    influence::{ConflictPolicy, InfluenceRule},
    simulation::{Fault, ScenarioEvent, SimulationScenario},
    world::{AlarmState, CabinEnvironment, DeviceState, HumanState, OuterEnvironmentState},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const MAX_SCENARIO_BYTES: usize = 1_048_576;
pub const MAX_SCENARIO_ENTITIES: usize = 1_000;
pub const MAX_SCENARIO_FAULTS: usize = 10_000;
pub const MAX_SCENARIO_AGENTS: usize = 32;
pub const MAX_SCENARIO_GOALS: usize = 32;
pub const MAX_SCENARIO_IDENTIFIER_BYTES: usize = 128;
pub const MAX_AGENT_CAPABILITIES: usize = 64;
pub const MAX_SCENARIO_INFLUENCES: usize = 10_000;
pub const MAX_SCENARIO_EVENTS: usize = 1_000;
pub const MAX_SCENARIO_TICKS: u64 = 10_000;
pub const MAX_SCENARIO_LANGUAGE_BYTES: usize = 32;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
    goals: Vec<String>,
    #[serde(default = "default_max_ticks")]
    max_ticks: u64,
    #[serde(default)]
    influences: Vec<InfluenceRule>,
    #[serde(default)]
    scenario_events: Vec<ScenarioEvent>,
    #[serde(default)]
    conflict_policy: Option<ConflictPolicy>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EntityDocument {
    id: String,
    #[serde(rename = "type")]
    entity_type: String,
    #[serde(default)]
    components: serde_yaml::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FaultDocument {
    at_tick: u64,
    target: String,
    #[serde(rename = "type")]
    fault_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentDocument {
    id: String,
    backend: String,
    observation_profile: String,
    capabilities: Vec<String>,
}

fn default_max_ticks() -> u64 {
    80
}

fn default_language() -> String {
    "en".to_string()
}

pub fn load_scenario(path: impl AsRef<Path>) -> SimulationResult<SimulationScenario> {
    let path = path.as_ref();
    let file = fs::File::open(path).map_err(|err| {
        SimulationError::InvalidScenario(format!("failed to open scenario: {err}"))
    })?;
    let size = file.metadata().map_err(|err| {
        SimulationError::InvalidScenario(format!("failed to inspect scenario: {err}"))
    })?.len();
    if size > MAX_SCENARIO_BYTES as u64 {
        return Err(SimulationError::InvalidScenario(format!(
            "scenario exceeds {MAX_SCENARIO_BYTES} byte limit"
        )));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(MAX_SCENARIO_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| {
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
    let catalog = CapabilityCatalog::load_default();
    validate_document(&document, &catalog)?;

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
                apply_human_components(&mut human, &entity.components, &catalog)?;
                humans.push(human);
            }
            "device" => {
                let mut device = DeviceState::new(entity.id.clone());
                apply_device_components(&mut device, &entity.components)?;
                devices.push(device);
            }
            other => {
                return Err(SimulationError::InvalidScenario(format!(
                    "unsupported entity type '{other}'"
                )));
            }
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
        physics: cockpit_world::digital_twin::DigitalTwinParameters::default(),
        faults: document
            .faults
            .into_iter()
            .map(|fault| Fault {
                at_tick: fault.at_tick,
                target: fault.target,
                fault_type: fault.fault_type,
            })
            .collect(),
        scenario_events: document.scenario_events,
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
        public_goals: document.goals,
        max_ticks: document.max_ticks,
        influences: document.influences,
        conflict_policy: document
            .conflict_policy
            .unwrap_or(ConflictPolicy::RejectConflicting),
    })
}

fn validate_document(
    document: &ScenarioDocument,
    catalog: &CapabilityCatalog,
) -> SimulationResult<()> {
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
    validate_limit("goals", document.goals.len(), MAX_SCENARIO_GOALS)?;
    validate_identifier("scenario id", &document.id)?;
    if document.language.trim().is_empty()
        || document.language.len() > MAX_SCENARIO_LANGUAGE_BYTES
    {
        return Err(SimulationError::InvalidScenario(format!(
            "language must contain 1..={MAX_SCENARIO_LANGUAGE_BYTES} bytes"
        )));
    }
    if !(1..=MAX_SCENARIO_TICKS).contains(&document.max_ticks) {
        return Err(SimulationError::InvalidScenario(format!(
            "maxTicks must be in range 1..={MAX_SCENARIO_TICKS}"
        )));
    }
    for goal in &document.goals {
        if goal.trim().is_empty() || goal.len() > 1_024 {
            return Err(SimulationError::InvalidScenario(
                "each public goal must contain 1..=1024 bytes".to_string(),
            ));
        }
    }

    let mut entity_ids = HashSet::with_capacity(document.entities.len());
    let mut human_ids = HashSet::new();
    let mut outer_environments = 0;
    for entity in &document.entities {
        validate_identifier("entity id", &entity.id)?;
        if !entity_ids.insert(entity.id.as_str()) {
            return Err(SimulationError::InvalidScenario(format!(
                "duplicate entity id '{}'",
                entity.id
            )));
        }
        if !entity.components.is_null() && !entity.components.is_mapping() {
            return Err(SimulationError::InvalidScenario(format!(
                "entity '{}' components must be a mapping",
                entity.id
            )));
        }
        match entity.entity_type.as_str() {
            "environment" if entity.id == "cabin" => {}
            "environment" => {
                return Err(SimulationError::InvalidScenario(
                    "environment entity must use id 'cabin'".to_string(),
                ));
            }
            "outerEnvironment" => outer_environments += 1,
            "human" => {
                human_ids.insert(entity.id.as_str());
            }
            "device" => {}
            other => {
                return Err(SimulationError::InvalidScenario(format!(
                    "unsupported entity type '{other}'"
                )));
            }
        }
    }
    if outer_environments > 1 {
        return Err(SimulationError::InvalidScenario(
            "at most one outerEnvironment entity is allowed".to_string(),
        ));
    }

    for fault in &document.faults {
        validate_identifier("fault target", &fault.target)?;
        validate_identifier("fault type", &fault.fault_type)?;
        if !entity_ids.contains(fault.target.as_str()) {
            return Err(SimulationError::InvalidScenario(format!(
                "fault target '{}' does not reference an entity",
                fault.target
            )));
        }
    }

    let mut agent_ids = HashSet::with_capacity(document.agents.len());
    for agent in &document.agents {
        validate_identifier("agent id", &agent.id)?;
        validate_identifier("agent backend", &agent.backend)?;
        validate_identifier("agent observation profile", &agent.observation_profile)?;
        if entity_ids.contains(agent.id.as_str()) || !agent_ids.insert(agent.id.as_str()) {
            return Err(SimulationError::InvalidScenario(format!(
                "duplicate entity or agent id '{}'",
                agent.id
            )));
        }
        validate_capabilities(
            &format!("agent '{}'", agent.id),
            &agent.capabilities,
            catalog,
        )?;
    }
    if !document
        .entities
        .iter()
        .any(|entity| entity.id == "cabin" && entity.entity_type == "environment")
    {
        return Err(SimulationError::InvalidScenario(
            "missing cabin environment entity".to_string(),
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
    validate_limit(
        "scenario events",
        document.scenario_events.len(),
        MAX_SCENARIO_EVENTS,
    )?;
    for event in &document.scenario_events {
        validate_identifier("scenario event source", &event.source)?;
        validate_identifier("scenario event type", &event.event_type)?;
        if let Some(target) = &event.target {
            validate_identifier("scenario event target", target)?;
            if !entity_ids.contains(target.as_str()) {
                return Err(SimulationError::InvalidScenario(format!(
                    "scenario event target '{target}' does not reference an entity"
                )));
            }
        }
        if event.message.trim().is_empty() || event.message.len() > 1_024 {
            return Err(SimulationError::InvalidScenario(
                "scenario event message must be 1..=1024 bytes".to_string(),
            ));
        }
    }
    for influence in &document.influences {
        validate_identifier("influence rule id", &influence.rule_id)?;
        if influence.rule_version != cockpit_world::CURRENT_INFLUENCE_RULE_VERSION {
            return Err(SimulationError::InvalidScenario(format!(
                "influence rule '{}' has unsupported ruleVersion {}",
                influence.rule_id, influence.rule_version
            )));
        }
        if let Some(human_id) = influence.patch.human_id() {
            validate_identifier("influence human id", human_id)?;
            if !human_ids.contains(human_id) {
                return Err(SimulationError::InvalidScenario(format!(
                    "influence rule '{}' references unknown human '{}'",
                    influence.rule_id, human_id
                )));
            }
        }
        if let cockpit_world::influence::InfluenceSchedule::Every { interval, .. } =
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

fn validate_limit(name: &str, actual: usize, limit: usize) -> SimulationResult<()> {
    if actual <= limit {
        Ok(())
    } else {
        Err(SimulationError::InvalidScenario(format!(
            "{name} exceeds {limit} item limit"
        )))
    }
}

fn validate_capabilities(
    owner: &str,
    capabilities: &[String],
    catalog: &CapabilityCatalog,
) -> SimulationResult<()> {
    validate_limit(
        &format!("{owner} capabilities"),
        capabilities.len(),
        MAX_AGENT_CAPABILITIES,
    )?;
    let mut seen = HashSet::with_capacity(capabilities.len());
    for capability in capabilities {
        validate_identifier("capability", capability)?;
        if !seen.insert(capability.as_str()) {
            return Err(SimulationError::InvalidScenario(format!(
                "{owner} declares duplicate capability '{capability}'"
            )));
        }
        if !catalog.contains(capability) {
            return Err(SimulationError::InvalidScenario(format!(
                "{owner} declares unknown capability '{capability}'"
            )));
        }
    }
    Ok(())
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
    if let Some(smoke) = lookup(components, "smoke", "density")? {
        environment.smoke_density = smoke;
    }
    if let Some(temperature) = lookup(components, "temperature", "celsius")? {
        environment.temperature_c = temperature;
    }
    if let Some(humidity) = lookup(components, "humidity", "relativePct")? {
        environment.humidity_pct = humidity;
    }
    if let Some(pressure) = lookup(components, "pressure", "pascal")? {
        environment.pressure_pa = pressure;
    }
    if let Some(co2) = lookup(components, "airQuality", "carbonDioxidePpm")? {
        environment.carbon_dioxide_ppm = co2;
    }
    if let Some(co) = lookup(components, "airQuality", "carbonMonoxidePpm")? {
        environment.carbon_monoxide_ppm = co;
    }
    ensure_range("smoke.density", environment.smoke_density, 0.0, 3.0)?;
    ensure_range("humidity.relativePct", environment.humidity_pct, 0.0, 100.0)?;
    ensure_range(
        "pressure.pascal",
        environment.pressure_pa,
        20_000.0,
        120_000.0,
    )?;
    ensure_range(
        "airQuality.carbonDioxidePpm",
        environment.carbon_dioxide_ppm,
        300.0,
        50_000.0,
    )?;
    ensure_range(
        "airQuality.carbonMonoxidePpm",
        environment.carbon_monoxide_ppm,
        0.0,
        100_000.0,
    )?;
    Ok(())
}

fn apply_outer_environment_components(
    outer: &mut OuterEnvironmentState,
    components: &serde_yaml::Value,
) -> SimulationResult<()> {
    if let Some(temperature) = lookup(components, "temperature", "celsius")? {
        outer.external_temperature_c = temperature;
    }
    if let Some(humidity) = lookup(components, "humidity", "relativePct")? {
        outer.relative_humidity_pct = humidity;
    }
    if let Some(solar) = lookup(components, "solar", "irradianceWm2")? {
        outer.solar_irradiance_w_m2 = solar;
    }
    if let Some(altitude) = lookup(components, "altitude", "meters")? {
        outer.altitude_m = altitude;
    }
    if let Some(wind) = lookup(components, "wind", "speedKmh")? {
        outer.wind_speed_kmh = wind;
    }
    if let Some(precipitation) = lookup(components, "weather", "precipitation")? {
        outer.precipitation = precipitation;
    }
    ensure_range(
        "humidity.relativePct",
        outer.relative_humidity_pct,
        0.0,
        100.0,
    )?;
    ensure_range(
        "solar.irradianceWm2",
        outer.solar_irradiance_w_m2,
        0.0,
        1_500.0,
    )?;
    ensure_range("altitude.meters", outer.altitude_m, -500.0, 11_000.0)?;
    Ok(())
}

fn apply_human_components(
    human: &mut HumanState,
    components: &serde_yaml::Value,
    catalog: &CapabilityCatalog,
) -> SimulationResult<()> {
    if let Some(attention) = lookup(components, "attention", "value")? {
        human.attention = attention;
    }
    if let Some(location) = scalar_string(components, "location")? {
        human.location = location;
    }
    if let Some(name) = scalar_string(components, "name")? {
        human.persona.name = name;
    }
    if let Some(role) = scalar_string(components, "role")? {
        human.persona.role = role;
    }
    if let Some(background) = scalar_string(components, "background")? {
        human.persona.background = background;
    }
    if let Some(capabilities) = sequence_strings(components, "actionCapabilities")? {
        validate_capabilities(
            &format!("human '{}'", human.id),
            &capabilities,
            catalog,
        )?;
        human.action_capabilities = capabilities;
    }
    if let Some(relationships) = sequence_strings(components, "relationships")? {
        validate_limit("human relationships", relationships.len(), MAX_AGENT_CAPABILITIES)?;
        if relationships
            .iter()
            .any(|value| value.trim().is_empty() || value.len() > 1_024)
        {
            return Err(SimulationError::InvalidScenario(
                "each human relationship must contain 1..=1024 bytes".to_string(),
            ));
        }
        human.persona.relationships = relationships;
    }
    let mut traits = human.persona.traits;
    if let Some(value) = lookup(components, "traits", "openness")? {
        traits.openness = value;
    }
    if let Some(value) = lookup(components, "traits", "conscientiousness")? {
        traits.conscientiousness = value;
    }
    if let Some(value) = lookup(components, "traits", "extraversion")? {
        traits.extraversion = value;
    }
    if let Some(value) = lookup(components, "traits", "agreeableness")? {
        traits.agreeableness = value;
    }
    if let Some(value) = lookup(components, "traits", "neuroticism")? {
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
    if let Some(capabilities) = sequence_strings(components, "capabilities")? {
        validate_limit(
            "device capabilities",
            capabilities.len(),
            MAX_AGENT_CAPABILITIES,
        )?;
        let mut seen = HashSet::with_capacity(capabilities.len());
        for capability in &capabilities {
            validate_identifier("device capability", capability)?;
            if !seen.insert(capability.as_str()) {
                return Err(SimulationError::InvalidScenario(format!(
                    "device '{}' declares duplicate capability '{capability}'",
                    device.id
                )));
            }
        }
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

fn lookup(
    components: &serde_yaml::Value,
    component: &str,
    field: &str,
) -> SimulationResult<Option<f64>> {
    let Some(component_value) = components.get(component) else {
        return Ok(None);
    };
    if !component_value.is_mapping() {
        return Err(SimulationError::InvalidScenario(format!(
            "component '{component}' must be a mapping"
        )));
    }
    let Some(value) = component_value.get(field) else {
        return Ok(None);
    };
    let number = value.as_f64().filter(|number| number.is_finite()).ok_or_else(|| {
        SimulationError::InvalidScenario(format!(
            "component '{component}.{field}' must be a finite number"
        ))
    })?;
    Ok(Some(number))
}

fn scalar_string(
    components: &serde_yaml::Value,
    field: &str,
) -> SimulationResult<Option<String>> {
    let Some(value) = components.get(field) else {
        return Ok(None);
    };
    let value = value.as_str().ok_or_else(|| {
        SimulationError::InvalidScenario(format!("component '{field}' must be a string"))
    })?;
    if value.trim().is_empty() || value.len() > 1_024 {
        return Err(SimulationError::InvalidScenario(format!(
            "component '{field}' must contain 1..=1024 bytes"
        )));
    }
    Ok(Some(value.to_string()))
}

fn sequence_strings(
    components: &serde_yaml::Value,
    field: &str,
) -> SimulationResult<Option<Vec<String>>> {
    let Some(value) = components.get(field) else {
        return Ok(None);
    };
    let sequence = value.as_sequence().ok_or_else(|| {
        SimulationError::InvalidScenario(format!("component '{field}' must be a sequence"))
    })?;
    let values = sequence
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value.as_str().map(ToString::to_string).ok_or_else(|| {
                SimulationError::InvalidScenario(format!(
                    "component '{field}' item {index} must be a string"
                ))
            })
        })
        .collect::<SimulationResult<Vec<_>>>()?;
    Ok(Some(values))
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

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_SCENARIO: &str = include_str!("../../../scenarios/smoke-in-cockpit.yaml");

    #[test]
    fn parses_built_in_scenario() {
        let scenario = parse_scenario_bytes(VALID_SCENARIO.as_bytes()).expect("valid scenario");
        assert_eq!(scenario.id, "smoke-in-cockpit");
        assert_eq!(scenario.max_ticks, 34);
    }

    #[test]
    fn rejects_duplicate_entity_and_agent_ids() {
        let duplicate_entity = VALID_SCENARIO.replacen("id: pilot-1", "id: cabin", 1);
        assert!(parse_scenario_bytes(duplicate_entity.as_bytes()).is_err());

        let duplicate_agent = VALID_SCENARIO.replacen("id: cockpit-agent", "id: cabin", 1);
        assert!(parse_scenario_bytes(duplicate_agent.as_bytes()).is_err());
    }

    #[test]
    fn rejects_unknown_entity_and_document_fields() {
        let unknown_type = VALID_SCENARIO.replacen("type: human", "type: typoHuman", 1);
        assert!(parse_scenario_bytes(unknown_type.as_bytes()).is_err());

        let unknown_field = VALID_SCENARIO.replacen(
            "backend: scripted",
            "backend: scripted\n    unexpectedField: true",
            1,
        );
        assert!(parse_scenario_bytes(unknown_field.as_bytes()).is_err());
    }

    #[test]
    fn rejects_non_string_capabilities_and_excessive_ticks() {
        let malformed = VALID_SCENARIO.replacen("- engine.shutdown", "- 7", 1);
        assert!(parse_scenario_bytes(malformed.as_bytes()).is_err());

        let excessive = VALID_SCENARIO.replacen("maxTicks: 34", "maxTicks: 10001", 1);
        assert!(parse_scenario_bytes(excessive.as_bytes()).is_err());
    }

    #[test]
    fn rejects_oversized_scenario_bytes() {
        let oversized = vec![b' '; MAX_SCENARIO_BYTES + 1];
        assert!(parse_scenario_bytes(&oversized).is_err());
    }
}
