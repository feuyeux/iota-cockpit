use serde::{Deserialize, Serialize};

use crate::world::WorldSnapshot;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorQuality {
    pub visibility_quality: f64,
    pub audio_quality: f64,
    pub confidence: f64,
    pub degraded: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub observation_id: String,
    pub run_id: String,
    pub agent_id: String,
    pub sensor_id: String,
    pub observed_tick: u64,
    pub delivered_tick: u64,
    pub visible_entities: Vec<String>,
    pub alerts: Vec<String>,
    pub action_results: Vec<String>,
    pub confidence: f64,
    pub quality: SensorQuality,
}

impl Observation {
    pub fn from_snapshot(run_id: &str, agent_id: &str, snapshot: &WorldSnapshot) -> Self {
        let visibility_quality = snapshot.environment.visibility.clamp(0.0, 1.0);
        let audio_quality =
            (1.0 - ((snapshot.environment.noise_db - 45.0).max(0.0) / 55.0)).clamp(0.0, 1.0);
        let confidence = ((visibility_quality + audio_quality) / 2.0).clamp(0.0, 1.0);
        let degraded = confidence < 0.72;
        let mut alerts = Vec::new();
        if degraded && snapshot.environment.fire_active {
            alerts.push("SmokeDetected".to_string());
        }
        if snapshot.alarm.active {
            alerts.push("AlarmActive".to_string());
        }

        Self {
            observation_id: format!("{run_id}-obs-{}", snapshot.tick),
            run_id: run_id.to_string(),
            agent_id: agent_id.to_string(),
            sensor_id: "pilot-default".to_string(),
            observed_tick: snapshot.tick,
            delivered_tick: snapshot.tick,
            visible_entities: vec![
                "cabin".to_string(),
                "pilot-1".to_string(),
                "engine-1".to_string(),
                "alarm-1".to_string(),
            ],
            alerts,
            action_results: Vec::new(),
            confidence,
            quality: SensorQuality {
                visibility_quality,
                audio_quality,
                confidence,
                degraded,
            },
        }
    }
}
