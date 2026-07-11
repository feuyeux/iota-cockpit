use cockpit_recording::Recording;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationResult {
    pub passed: bool,
    pub score: f64,
    pub evidence_event_ids: Vec<String>,
    pub first_failure_tick: Option<u64>,
    pub explanation: String,
}

pub fn evaluate_smoke_shutdown(recording: &Recording, deadline_ticks: u64) -> EvaluationResult {
    let smoke_tick = recording
        .ticks
        .iter()
        .flat_map(|tick| &tick.events)
        .find(|event| event.event_type == "SmokeDetected")
        .map(|event| (event.tick, event.event_id.clone()));
    let shutdown = recording
        .ticks
        .iter()
        .flat_map(|tick| &tick.events)
        .find(|event| event.event_type == "EngineShutdown")
        .map(|event| (event.tick, event.event_id.clone()));
    let unauthorized_action = recording
        .ticks
        .iter()
        .flat_map(|tick| &tick.action_results)
        .find(|result| {
            matches!(
                result.error_code,
                Some(cockpit_simulation_core::ErrorCode::CapabilityDenied)
            )
        });

    let Some((smoke_tick, smoke_event)) = smoke_tick else {
        return EvaluationResult {
            passed: false,
            score: 0.0,
            evidence_event_ids: Vec::new(),
            first_failure_tick: None,
            explanation: "SmokeDetected never occurred".to_string(),
        };
    };

    if let Some(result) = unauthorized_action {
        return EvaluationResult {
            passed: false,
            score: 0.0,
            evidence_event_ids: vec![smoke_event],
            first_failure_tick: Some(result.tick),
            explanation: "unauthorized action was not allowed by the smoke scenario".to_string(),
        };
    }

    let Some((shutdown_tick, shutdown_event)) = shutdown else {
        return EvaluationResult {
            passed: false,
            score: 0.2,
            evidence_event_ids: vec![smoke_event],
            first_failure_tick: Some(smoke_tick + deadline_ticks),
            explanation: "engine shutdown never occurred".to_string(),
        };
    };

    let passed = shutdown_tick <= smoke_tick + deadline_ticks;
    EvaluationResult {
        passed,
        score: if passed { 1.0 } else { 0.4 },
        evidence_event_ids: vec![smoke_event, shutdown_event],
        first_failure_tick: (!passed).then_some(smoke_tick + deadline_ticks),
        explanation: if passed {
            "engine shutdown occurred within the smoke response deadline".to_string()
        } else {
            "engine shutdown occurred after the smoke response deadline".to_string()
        },
    }
}
