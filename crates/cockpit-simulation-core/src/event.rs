use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPayload {
    pub message: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope {
    pub event_id: String,
    pub event_type: String,
    pub run_id: String,
    pub tick: u64,
    pub source: String,
    pub priority: i32,
    pub sequence: u64,
    pub correlation_id: String,
    pub payload: EventPayload,
}
