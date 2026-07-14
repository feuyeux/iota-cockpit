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

/// Dispatch to the evaluator registered for `rule_id`, falling back to the
/// default smoke-shutdown evaluator when `rule_id` is `None`, and localize the
/// human-readable explanation to `language` ("en" or "zh"). This keeps
/// evaluation resource-driven and bilingual: a scenario names its rule (via
/// `evaluation[0].id`) and its `language`, and the runner dispatches here
/// rather than hardcoding a single English evaluator at the call site. An
/// unrecognized rule id yields a failing result that names the missing
/// evaluator rather than silently passing.
pub fn evaluate(
    recording: &Recording,
    rule_id: Option<&str>,
    deadline_ticks: u64,
    language: &str,
) -> EvaluationResult {
    let mut result = match rule_id {
        None | Some("shutdown-before-spread") => evaluate_smoke_shutdown(recording, deadline_ticks),
        Some(rule_id) if benchmark_rule(rule_id).is_some() => evaluate_benchmark_rule(
            recording,
            benchmark_rule(rule_id).expect("rule exists"),
            deadline_ticks,
        ),
        Some(unknown) => EvaluationResult {
            passed: false,
            score: 0.0,
            evidence_event_ids: Vec::new(),
            first_failure_tick: None,
            explanation: format!("no evaluator is registered for rule id '{unknown}'"),
        },
    };
    result.explanation = localize_explanation(&result.explanation, language);
    result
}

/// Localize a known English evaluation explanation to `language`. Unknown or
/// already-localized text is returned unchanged, so this degrades gracefully
/// rather than dropping information.
fn localize_explanation(english: &str, language: &str) -> String {
    if !matches!(language, "zh" | "zh-CN" | "zh-Hans") {
        return english.to_string();
    }
    let zh = match english {
        "engine shutdown occurred within the smoke response deadline" => {
            "引擎在烟雾响应截止时间内完成关闭"
        }
        "engine shutdown occurred after the smoke response deadline" => {
            "引擎关闭发生在烟雾响应截止时间之后"
        }
        "engine shutdown never occurred" => "引擎从未关闭",
        "SmokeDetected never occurred" => "从未检测到烟雾",
        "unauthorized action was not allowed by the smoke scenario" => {
            "未授权的操作在该烟雾场景中被拒绝"
        }
        "thermal comfort target was reached before the deadline" => "在截止时间前达到热舒适目标",
        "thermal comfort target was not reached before the deadline" => {
            "未在截止时间前达到热舒适目标"
        }
        "windshield visibility was restored before the deadline" => {
            "在截止时间前恢复了前风挡能见度"
        }
        "windshield visibility was not restored before the deadline" => {
            "未在截止时间前恢复前风挡能见度"
        }
        "fatigue intervention restored driver attention before the deadline" => {
            "疲劳干预在截止时间前恢复了驾驶员注意力"
        }
        "fatigue intervention did not restore driver attention before the deadline" => {
            "疲劳干预未在截止时间前恢复驾驶员注意力"
        }
        "child protection cooling activated before the deadline" => "儿童保护降温在截止时间前生效",
        "child protection cooling did not activate before the deadline" => {
            "儿童保护降温未在截止时间前生效"
        }
        "medical response stabilized the patient before the deadline" => {
            "医疗响应在截止时间前稳定了患者状态"
        }
        "medical response did not stabilize the patient before the deadline" => {
            "医疗响应未在截止时间前稳定患者状态"
        }
        "privacy handling restored driver focus before the deadline" => {
            "隐私处置在截止时间前恢复了驾驶员专注度"
        }
        "privacy handling did not restore driver focus before the deadline" => {
            "隐私处置未在截止时间前恢复驾驶员专注度"
        }
        "charging plan reduced range anxiety before the deadline" => {
            "充电方案在截止时间前降低了续航焦虑"
        }
        "charging plan did not reduce range anxiety before the deadline" => {
            "充电方案未在截止时间前降低续航焦虑"
        }
        "ADAS takeover restored driver attention before the deadline" => {
            "辅助驾驶接管在截止时间前恢复了驾驶员注意力"
        }
        "ADAS takeover did not restore driver attention before the deadline" => {
            "辅助驾驶接管未在截止时间前恢复驾驶员注意力"
        }
        "cybersecurity safe mode contained the incident before the deadline" => {
            "网络安全模式在截止时间前控制了事件"
        }
        "cybersecurity safe mode did not contain the incident before the deadline" => {
            "网络安全模式未在截止时间前控制事件"
        }
        other if other.starts_with("no evaluator is registered for rule id") => {
            return format!(
                "未注册对应的评测规则：{}",
                other
                    .trim_start_matches("no evaluator is registered for rule id ")
                    .trim_matches('\'')
            );
        }
        other => return other.to_string(),
    };
    zh.to_string()
}

#[derive(Debug, Clone, Copy)]
enum Threshold {
    AtMost(f64),
    AtLeast(f64),
}

impl Threshold {
    fn matches(self, value: f64) -> bool {
        match self {
            Self::AtMost(limit) => value <= limit,
            Self::AtLeast(limit) => value >= limit,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BenchmarkRule {
    event_type: &'static str,
    evidence_source: &'static str,
    target: &'static str,
    threshold: Threshold,
    success: &'static str,
    failure: &'static str,
}

fn benchmark_rule(rule_id: &str) -> Option<BenchmarkRule> {
    Some(match rule_id {
        "thermal-comfort-restored" => BenchmarkRule {
            event_type: "ThermalComfortRestored",
            evidence_source: "hvac-1",
            target: "cabin",
            threshold: Threshold::AtMost(26.0),
            success: "thermal comfort target was reached before the deadline",
            failure: "thermal comfort target was not reached before the deadline",
        },
        "windshield-visibility-restored" => BenchmarkRule {
            event_type: "WindshieldVisibilityRestored",
            evidence_source: "defogger-1",
            target: "cabin",
            threshold: Threshold::AtLeast(0.8),
            success: "windshield visibility was restored before the deadline",
            failure: "windshield visibility was not restored before the deadline",
        },
        "fatigue-intervention-effective" => BenchmarkRule {
            event_type: "DriverAttentionRestored",
            evidence_source: "dms-1",
            target: "driver-1",
            threshold: Threshold::AtLeast(0.7),
            success: "fatigue intervention restored driver attention before the deadline",
            failure: "fatigue intervention did not restore driver attention before the deadline",
        },
        "child-protection-activated" => BenchmarkRule {
            event_type: "ChildProtectionActivated",
            evidence_source: "occupant-radar-1",
            target: "cabin",
            threshold: Threshold::AtMost(30.0),
            success: "child protection cooling activated before the deadline",
            failure: "child protection cooling did not activate before the deadline",
        },
        "medical-response-stabilized" => BenchmarkRule {
            event_type: "MedicalResponseActivated",
            evidence_source: "emergency-call-1",
            target: "patient-1",
            threshold: Threshold::AtMost(0.4),
            success: "medical response stabilized the patient before the deadline",
            failure: "medical response did not stabilize the patient before the deadline",
        },
        "privacy-conflict-contained" => BenchmarkRule {
            event_type: "PrivacyConflictContained",
            evidence_source: "voice-array-1",
            target: "driver-1",
            threshold: Threshold::AtLeast(0.8),
            success: "privacy handling restored driver focus before the deadline",
            failure: "privacy handling did not restore driver focus before the deadline",
        },
        "ev-route-plan-stabilized" => BenchmarkRule {
            event_type: "ChargingPlanAccepted",
            evidence_source: "navigation-1",
            target: "driver-1",
            threshold: Threshold::AtMost(0.4),
            success: "charging plan reduced range anxiety before the deadline",
            failure: "charging plan did not reduce range anxiety before the deadline",
        },
        "adas-takeover-completed" => BenchmarkRule {
            event_type: "AdasTakeoverCompleted",
            evidence_source: "adas-controller-1",
            target: "driver-1",
            threshold: Threshold::AtLeast(0.9),
            success: "ADAS takeover restored driver attention before the deadline",
            failure: "ADAS takeover did not restore driver attention before the deadline",
        },
        "cyber-incident-contained" => BenchmarkRule {
            event_type: "CyberIncidentContained",
            evidence_source: "security-monitor-1",
            target: "driver-1",
            threshold: Threshold::AtLeast(0.85),
            success: "cybersecurity safe mode contained the incident before the deadline",
            failure: "cybersecurity safe mode did not contain the incident before the deadline",
        },
        _ => return None,
    })
}

fn evaluate_benchmark_rule(
    recording: &Recording,
    rule: BenchmarkRule,
    deadline_ticks: u64,
) -> EvaluationResult {
    let evidence = recording
        .ticks
        .iter()
        .flat_map(|tick| &tick.events)
        .find(|event| {
            event.tick <= deadline_ticks
                && event.event_type == rule.event_type
                && event.source == rule.evidence_source
                && event.payload.target.as_deref() == Some(rule.target)
        });

    let passed = evidence
        .and_then(|event| event.payload.value)
        .is_some_and(|value| rule.threshold.matches(value));
    let deadline_observed = recording
        .ticks
        .last()
        .is_some_and(|tick| tick.tick >= deadline_ticks);

    EvaluationResult {
        passed,
        score: if passed {
            1.0
        } else if evidence.is_some() {
            0.4
        } else {
            0.2
        },
        evidence_event_ids: evidence
            .map(|event| vec![event.event_id.clone()])
            .unwrap_or_default(),
        first_failure_tick: (!passed && deadline_observed).then_some(deadline_ticks),
        explanation: if passed { rule.success } else { rule.failure }.to_string(),
    }
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
