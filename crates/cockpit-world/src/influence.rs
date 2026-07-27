//! Scheduled influence rules, deterministic conflict arbitration, and event
//! subscriptions beyond the smoke scenario's direct systems.
//!
//! An [`InfluenceRule`] declares a versioned, scheduled mutation of one world
//! component. On each tick the [`schedule_due`] selector picks the rules due at
//! that tick, and [`arbitrate`] resolves conflicts — multiple rules targeting
//! the same `(entity, component)` in the same tick — deterministically under an
//! explicit [`ConflictPolicy`]. [`Subscription::deliver`] filters events for a
//! subscriber by event type in stable order.
//!
//! All ordering is total and content-derived so replays are bit-stable. When a
//! scenario declares no influences, none of this changes tick behavior.

use serde::{Deserialize, Serialize};

use crate::state_patch::StatePatch;

pub const CURRENT_INFLUENCE_RULE_VERSION: u32 = 2;

/// When an influence rule fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum InfluenceSchedule {
    /// Fires once at exactly this tick.
    AtTick { tick: u64 },
    /// Fires at `start` and then every `interval` ticks (interval >= 1).
    Every { start: u64, interval: u64 },
}

impl InfluenceSchedule {
    /// Whether the rule is due at `tick`.
    pub fn is_due(&self, tick: u64) -> bool {
        match *self {
            InfluenceSchedule::AtTick { tick: at } => tick == at,
            InfluenceSchedule::Every { start, interval } => {
                let interval = interval.max(1);
                tick >= start && (tick - start).is_multiple_of(interval)
            }
        }
    }
}

/// How an influence rule changes its target component's numeric value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "op", content = "value")]
pub enum InfluenceOp {
    /// Replace the component value.
    Set(f64),
    /// Add a signed delta to the current component value.
    Delta(f64),
}

impl InfluenceOp {
    /// Resolve the target value given the current value.
    pub fn resolve(&self, current: f64) -> f64 {
        match *self {
            InfluenceOp::Set(value) => value,
            InfluenceOp::Delta(delta) => current + delta,
        }
    }
}

/// A typed influence target and operation. Unlike [`StatePatch`], this stores
/// an operation that resolves against the target's value at a future tick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InfluencePatch {
    CabinSmokeDensity {
        op: InfluenceOp,
    },
    CabinVisibility {
        op: InfluenceOp,
    },
    CabinTemperature {
        op: InfluenceOp,
    },
    EngineHealth {
        op: InfluenceOp,
    },
    AlarmActive {
        op: InfluenceOp,
    },
    HumanStress {
        #[serde(rename = "humanId")]
        human_id: String,
        op: InfluenceOp,
    },
    HumanAttention {
        #[serde(rename = "humanId")]
        human_id: String,
        op: InfluenceOp,
    },
}

impl InfluencePatch {
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

    pub fn human_id(&self) -> Option<&str> {
        match self {
            Self::HumanStress { human_id, .. } | Self::HumanAttention { human_id, .. } => {
                Some(human_id)
            }
            _ => None,
        }
    }

    pub fn resolve(&self, current: f64) -> StatePatch {
        match self {
            Self::CabinSmokeDensity { op } => StatePatch::CabinSmokeDensity {
                value: op.resolve(current),
            },
            Self::CabinVisibility { op } => StatePatch::CabinVisibility {
                value: op.resolve(current),
            },
            Self::CabinTemperature { op } => StatePatch::CabinTemperature {
                value: op.resolve(current),
            },
            Self::EngineHealth { op } => StatePatch::EngineHealth {
                value: op.resolve(current),
            },
            Self::AlarmActive { op } => StatePatch::AlarmActive {
                value: op.resolve(current),
            },
            Self::HumanStress { human_id, op } => StatePatch::HumanStress {
                human_id: human_id.clone(),
                value: op.resolve(current),
            },
            Self::HumanAttention { human_id, op } => StatePatch::HumanAttention {
                human_id: human_id.clone(),
                value: op.resolve(current),
            },
        }
    }
}

/// A versioned, scheduled influence over a single world component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfluenceRule {
    pub rule_id: String,
    /// Rule schema version, recorded so replays can reject incompatible rules.
    pub rule_version: u32,
    pub schedule: InfluenceSchedule,
    pub patch: InfluencePatch,
    /// Higher priority wins under `HighestPriorityWins`.
    #[serde(default)]
    pub priority: i32,
}

impl InfluenceRule {
    /// Stable identity of the writable target used for ordering and conflict
    /// arbitration.
    pub fn target_key(&self) -> (&str, &str) {
        self.patch.target_key()
    }
}

/// How to resolve multiple rules targeting the same component in one tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    /// Reject all conflicting rules for the contested component this tick.
    RejectConflicting,
    /// Apply the highest-priority rule; ties broken by lowest `rule_id`.
    HighestPriorityWins,
    /// Apply the rule with the lexicographically lowest `rule_id`.
    LowestRuleIdWins,
}

/// The disposition of one influence rule after arbitration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfluenceDecision {
    pub rule_id: String,
    pub entity_id: String,
    pub component_path: String,
    pub applied: bool,
    /// Present when `applied` is false: why the rule lost arbitration.
    pub rejected_reason: Option<String>,
}

/// Result of arbitrating the due rules for a tick.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArbitrationOutcome {
    /// Winning rules in deterministic `(entity, component)` order.
    pub winners: Vec<InfluenceRule>,
    /// Per-rule decisions (winners and losers), for recording evidence.
    pub decisions: Vec<InfluenceDecision>,
}

/// Select the rules due at `tick`, in deterministic order.
pub fn schedule_due(rules: &[InfluenceRule], tick: u64) -> Vec<InfluenceRule> {
    let mut due: Vec<InfluenceRule> = rules
        .iter()
        .filter(|rule| rule.schedule.is_due(tick))
        .cloned()
        .collect();
    due.sort_by(compare_rules);
    due
}

/// Total, content-derived ordering over rules: by target, then priority
/// (descending), then rule id.
fn compare_rules(left: &InfluenceRule, right: &InfluenceRule) -> std::cmp::Ordering {
    left.target_key()
        .cmp(&right.target_key())
        .then(right.priority.cmp(&left.priority))
        .then(left.rule_id.cmp(&right.rule_id))
}

/// Arbitrate the due rules under `policy`, returning winners and per-rule
/// decisions. Input need not be pre-sorted; output ordering is deterministic.
pub fn arbitrate(due: &[InfluenceRule], policy: ConflictPolicy) -> ArbitrationOutcome {
    // Group by target component while preserving deterministic order.
    let mut sorted = due.to_vec();
    sorted.sort_by(compare_rules);

    let mut outcome = ArbitrationOutcome::default();
    let mut index = 0;
    while index < sorted.len() {
        let target = sorted[index].target_key();
        let mut group_end = index + 1;
        while group_end < sorted.len() && sorted[group_end].target_key() == target {
            group_end += 1;
        }
        let group = &sorted[index..group_end];
        resolve_group(group, policy, &mut outcome);
        index = group_end;
    }
    outcome
}

fn resolve_group(
    group: &[InfluenceRule],
    policy: ConflictPolicy,
    outcome: &mut ArbitrationOutcome,
) {
    if group.len() == 1 {
        outcome.winners.push(group[0].clone());
        outcome.decisions.push(decision(&group[0], true, None));
        return;
    }
    match policy {
        ConflictPolicy::RejectConflicting => {
            for rule in group {
                outcome.decisions.push(decision(
                    rule,
                    false,
                    Some("conflicting rules rejected for contested component".to_string()),
                ));
            }
        }
        ConflictPolicy::HighestPriorityWins | ConflictPolicy::LowestRuleIdWins => {
            // `group` is already sorted by priority-desc then rule_id, so for
            // HighestPriorityWins the winner is index 0. For LowestRuleIdWins
            // pick the lexicographically smallest rule_id.
            let winner_index = match policy {
                ConflictPolicy::HighestPriorityWins => 0,
                ConflictPolicy::LowestRuleIdWins => group
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| a.rule_id.cmp(&b.rule_id))
                    .map(|(i, _)| i)
                    .unwrap_or(0),
                ConflictPolicy::RejectConflicting => unreachable!(),
            };
            for (i, rule) in group.iter().enumerate() {
                if i == winner_index {
                    outcome.winners.push(rule.clone());
                    outcome.decisions.push(decision(rule, true, None));
                } else {
                    outcome.decisions.push(decision(
                        rule,
                        false,
                        Some("lost deterministic arbitration".to_string()),
                    ));
                }
            }
        }
    }
}

fn decision(rule: &InfluenceRule, applied: bool, reason: Option<String>) -> InfluenceDecision {
    let (entity_id, component_path) = rule.target_key();
    InfluenceDecision {
        rule_id: rule.rule_id.clone(),
        entity_id: entity_id.to_string(),
        component_path: component_path.to_string(),
        applied,
        rejected_reason: reason,
    }
}

/// A subscription that selects events by type for a named subscriber.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub subscriber_id: String,
    /// Event types this subscriber receives. Empty means all types.
    #[serde(default)]
    pub event_types: Vec<String>,
}

impl Subscription {
    /// Whether an event of `event_type` matches this subscription.
    pub fn matches(&self, event_type: &str) -> bool {
        self.event_types.is_empty() || self.event_types.iter().any(|value| value == event_type)
    }

    /// Deliver the matching event indices from `event_types_in_order`,
    /// preserving input order (which is already deterministic per tick).
    pub fn deliver<'a, I>(&self, events: I) -> Vec<usize>
    where
        I: IntoIterator<Item = &'a str>,
    {
        events
            .into_iter()
            .enumerate()
            .filter(|(_, event_type)| self.matches(event_type))
            .map(|(index, _)| index)
            .collect()
    }
}
