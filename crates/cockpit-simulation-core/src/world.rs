use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{SimulationError, SimulationResult};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentState {
    pub temperature_c: f64,
    pub humidity_pct: f64,
    pub visibility: f64,
    pub smoke_density: f64,
    pub lighting_lux: f64,
    pub noise_db: f64,
    pub fire_active: bool,
}

impl Default for EnvironmentState {
    fn default() -> Self {
        Self {
            temperature_c: 22.0,
            humidity_pct: 45.0,
            visibility: 1.0,
            smoke_density: 0.0,
            lighting_lux: 400.0,
            noise_db: 42.0,
            fire_active: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HumanState {
    pub stress: f64,
    pub fatigue: f64,
    pub health: f64,
    pub attention: f64,
    pub knowledge: Vec<String>,
    pub memory: Vec<String>,
    pub goal: String,
    pub location: String,
}

impl Default for HumanState {
    fn default() -> Self {
        Self {
            stress: 0.1,
            fatigue: 0.0,
            health: 1.0,
            attention: 0.9,
            knowledge: vec!["engine-panel".to_string()],
            memory: Vec::new(),
            goal: "maintain safe cockpit state".to_string(),
            location: "cockpit".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceLifecycle {
    Normal,
    Warning,
    Failed,
    Recovering,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceState {
    pub health: f64,
    pub power_state: String,
    pub lifecycle: DeviceLifecycle,
    pub faults: Vec<String>,
    pub capabilities: Vec<String>,
    pub shutdown: bool,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            health: 1.0,
            power_state: "powered".to_string(),
            lifecycle: DeviceLifecycle::Normal,
            faults: Vec::new(),
            capabilities: vec!["shutdown".to_string()],
            shutdown: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmState {
    pub active: bool,
    pub volume_db: f64,
}

impl Default for AlarmState {
    fn default() -> Self {
        Self {
            active: false,
            volume_db: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldSnapshot {
    pub run_id: String,
    pub tick: u64,
    pub sim_time_ms: u64,
    pub version: u64,
    pub environment: EnvironmentState,
    pub pilot: HumanState,
    pub engine: DeviceState,
    pub alarm: AlarmState,
}

impl WorldSnapshot {
    pub fn content_hash(&self) -> SimulationResult<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct HashableSnapshot<'a> {
            tick: u64,
            sim_time_ms: u64,
            version: u64,
            environment: &'a EnvironmentState,
            pilot: &'a HumanState,
            engine: &'a DeviceState,
            alarm: &'a AlarmState,
        }

        let hashable = HashableSnapshot {
            tick: self.tick,
            sim_time_ms: self.sim_time_ms,
            version: self.version,
            environment: &self.environment,
            pilot: &self.pilot,
            engine: &self.engine,
            alarm: &self.alarm,
        };
        let bytes = serde_json::to_vec(&hashable)
            .map_err(|err| SimulationError::Serialization(err.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Ok(format!("{:x}", hasher.finalize()))
    }
}
