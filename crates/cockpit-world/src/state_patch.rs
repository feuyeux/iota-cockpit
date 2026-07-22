//! The closed writable-state vocabulary shared by external diffs and
//! scheduled influences.

use serde::{Deserialize, Serialize};

use crate::world::WorldSnapshot;

/// A version-checked external write request. This remains separate from the
/// patch value so producers cannot omit provenance or optimistic concurrency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateDiff {
    pub source_id: String,
    pub patch: StatePatch,
    pub expected_state_version: u64,
}

impl StateDiff {
    pub(crate) fn is_valid(&self) -> bool {
        self.patch.is_valid()
    }
}

/// Tagged external patch vocabulary. A closed enum prevents producers from
/// introducing arbitrary component paths outside the world write boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StatePatch {
    CabinSmokeDensity {
        value: f64,
    },
    CabinVisibility {
        value: f64,
    },
    CabinTemperature {
        value: f64,
    },
    EngineHealth {
        value: f64,
    },
    AlarmActive {
        value: f64,
    },
    HumanStress {
        #[serde(rename = "humanId")]
        human_id: String,
        value: f64,
    },
    HumanAttention {
        #[serde(rename = "humanId")]
        human_id: String,
        value: f64,
    },
}

impl StatePatch {
    pub fn value(&self) -> f64 {
        match self {
            Self::CabinSmokeDensity { value }
            | Self::CabinVisibility { value }
            | Self::CabinTemperature { value }
            | Self::EngineHealth { value }
            | Self::AlarmActive { value }
            | Self::HumanStress { value, .. }
            | Self::HumanAttention { value, .. } => *value,
        }
    }

    pub fn target_key(&self) -> (&str, &str) {
        match self {
            Self::CabinSmokeDensity { .. } => ("cabin", "environment.smokeDensity"),
            Self::CabinVisibility { .. } => ("cabin", "environment.visibility"),
            Self::CabinTemperature { .. } => ("cabin", "environment.temperatureC"),
            Self::EngineHealth { .. } => ("engine-1", "engine.health"),
            Self::AlarmActive { .. } => ("alarm-1", "alarm.active"),
            Self::HumanStress { human_id, .. } => (human_id, "pilot.stress"),
            Self::HumanAttention { human_id, .. } => (human_id, "pilot.attention"),
        }
    }

    fn target(&self) -> StatePatchTarget<'_> {
        let (entity_id, component_path) = self.target_key();
        StatePatchTarget::parse(entity_id, component_path).expect("StatePatch variants are valid")
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.target().value_is_valid(self.value())
    }

    pub(crate) fn apply(&self, snapshot: &mut WorldSnapshot) {
        self.target().write(snapshot, self.value());
    }
}

/// The single writable-component registry used by external state diffs and
/// scheduled influences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatePatchTarget<'a> {
    CabinSmokeDensity,
    CabinVisibility,
    CabinTemperature,
    EngineHealth,
    AlarmActive,
    HumanStress(&'a str),
    HumanAttention(&'a str),
}

impl<'a> StatePatchTarget<'a> {
    pub fn parse(entity_id: &'a str, component_path: &str) -> Option<Self> {
        match (entity_id, component_path) {
            ("cabin", "environment.smokeDensity") => Some(Self::CabinSmokeDensity),
            ("cabin", "environment.visibility") => Some(Self::CabinVisibility),
            ("cabin", "environment.temperatureC") => Some(Self::CabinTemperature),
            ("engine-1", "engine.health") => Some(Self::EngineHealth),
            ("alarm-1", "alarm.active") => Some(Self::AlarmActive),
            (human_id, "pilot.stress") => Some(Self::HumanStress(human_id)),
            (human_id, "pilot.attention") => Some(Self::HumanAttention(human_id)),
            _ => None,
        }
    }

    pub fn value_is_valid(self, value: f64) -> bool {
        match self {
            Self::CabinSmokeDensity => (0.0..=3.0).contains(&value),
            Self::CabinVisibility
            | Self::EngineHealth
            | Self::AlarmActive
            | Self::HumanStress(_)
            | Self::HumanAttention(_) => (0.0..=1.0).contains(&value),
            Self::CabinTemperature => (-80.0..=100.0).contains(&value),
        }
    }

    pub(crate) fn read(self, snapshot: &WorldSnapshot) -> Option<f64> {
        match self {
            Self::CabinSmokeDensity => Some(snapshot.environment.smoke_density),
            Self::CabinVisibility => Some(snapshot.environment.visibility),
            Self::CabinTemperature => Some(snapshot.environment.temperature_c),
            Self::EngineHealth => snapshot.device("engine-1").map(|engine| engine.health),
            Self::AlarmActive => Some(if snapshot.alarm.active { 1.0 } else { 0.0 }),
            Self::HumanStress(human_id) => snapshot.human(human_id).map(|human| human.stress),
            Self::HumanAttention(human_id) => snapshot.human(human_id).map(|human| human.attention),
        }
    }

    pub(crate) fn write(self, snapshot: &mut WorldSnapshot, value: f64) {
        match self {
            Self::CabinSmokeDensity => snapshot.environment.smoke_density = value,
            Self::CabinVisibility => snapshot.environment.visibility = value,
            Self::CabinTemperature => snapshot.environment.temperature_c = value,
            Self::EngineHealth => {
                if let Some(engine) = snapshot.device_mut("engine-1") {
                    engine.health = value;
                }
            }
            Self::AlarmActive => snapshot.alarm.active = value > 0.5,
            Self::HumanStress(human_id) => {
                if let Some(human) = snapshot.human_mut(human_id) {
                    human.stress = value;
                }
            }
            Self::HumanAttention(human_id) => {
                if let Some(human) = snapshot.human_mut(human_id) {
                    human.attention = value;
                }
            }
        }
    }
}

pub(crate) fn read_component_value(
    snapshot: &WorldSnapshot,
    entity_id: &str,
    component_path: &str,
) -> Option<f64> {
    StatePatchTarget::parse(entity_id, component_path)?.read(snapshot)
}
