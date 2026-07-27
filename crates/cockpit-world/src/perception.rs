//! Deterministic perception delay simulation.
//!
//! Perception is modeled as its own simulation step, separate from the
//! backend-driven human decision layer: an event happening in the world does
//! not arrive in a human's [`PerceivedEvent`] queue instantly. Instead it is
//! enqueued with a deterministic `available_at_tick` computed from the
//! perceiving human's location (distance from the event's source) and current
//! attention. This keeps delivery reproducible across replay without any
//! randomness.
//!
//! Two kinds of perception feed the same per-human `short_term_memory` queue:
//! - physical perception: changes in the cabin/outer environment or device
//!   state (e.g. smoke, alarms).
//! - social perception: another human's utterance or visible action from the
//!   prior tick, modeling the "single-round" interaction model where a
//!   response is only perceived on a later tick, never within the same tick.

use crate::world::{HumanState, PerceivedEvent, WorldSnapshot};

/// Locations are compared as opaque strings; humans in the same location have
/// zero physical distance, otherwise distance is a fixed step. This is
/// intentionally simple and deterministic; scenarios needing finer-grained
/// topology can extend this with a location graph later without changing the
/// public delay contract.
fn location_distance(from: &str, to: &str) -> u64 {
    if from == to { 0 } else { 1 }
}

/// Compute the deterministic tick delay before `human` perceives an event
/// originating at `source_location`. Delay grows with distance and shrinks
/// with attention: a fully attentive human (`attention == 1.0`) perceives a
/// same-location event immediately (delay 0) and a different-location event
/// after 1 tick; a fully inattentive human (`attention == 0.0`) takes up to
/// 3 ticks longer.
pub fn perception_delay_ticks(human: &HumanState, source_location: &str) -> u64 {
    let distance = location_distance(&human.location, source_location);
    let inattention_penalty = ((1.0 - human.attention.clamp(0.0, 1.0)) * 3.0).round() as u64;
    distance + inattention_penalty
}

/// Enqueue a physical event (environment/device change) into every human's
/// perception queue with a per-human deterministic delay based on their
/// location and attention relative to `source_location`.
pub fn enqueue_physical_event(
    snapshot: &mut WorldSnapshot,
    origin_tick: u64,
    source_location: &str,
    source: &str,
    kind: &str,
    summary: &str,
) {
    for human in &mut snapshot.humans {
        let delay = perception_delay_ticks(human, source_location);
        human.short_term_memory.push(PerceivedEvent {
            origin_tick,
            available_at_tick: origin_tick + delay,
            source: source.to_string(),
            kind: kind.to_string(),
            summary: summary.to_string(),
        });
    }
}

/// Enqueue a social perception (an utterance or visible action from
/// `speaker_id`) into every *other* human's perception queue. The event is
/// always delayed by at least 1 tick even for a same-location listener, so a
/// reply can never be perceived within the same tick it was spoken (the
/// single-round interaction model): a listener in the same location still
/// waits 1 tick, and distance/inattention add on top of that floor.
pub fn enqueue_social_event(
    snapshot: &mut WorldSnapshot,
    origin_tick: u64,
    speaker_id: &str,
    speaker_location: &str,
    utterance: &str,
) {
    let listeners: Vec<usize> = snapshot
        .humans
        .iter()
        .enumerate()
        .filter(|(_, human)| human.id != speaker_id)
        .map(|(index, _)| index)
        .collect();
    for index in listeners {
        let human = &mut snapshot.humans[index];
        let delay = perception_delay_ticks(human, speaker_location).max(1);
        human.short_term_memory.push(PerceivedEvent {
            origin_tick,
            available_at_tick: origin_tick + delay,
            source: speaker_id.to_string(),
            kind: "utterance".to_string(),
            summary: utterance.to_string(),
        });
    }
}

/// Split `human`'s short-term memory into (delivered, still-pending) as of
/// `current_tick`. Delivered events are the ones a backend prompt for this
/// human should include this tick; pending events remain queued for a later
/// tick. Ordering within each group preserves insertion order, which is
/// itself deterministic (ticks are processed in order and enqueue calls
/// within a tick follow a fixed, scenario-defined iteration order).
pub fn delivered_and_pending(
    human: &HumanState,
    current_tick: u64,
) -> (Vec<PerceivedEvent>, Vec<PerceivedEvent>) {
    let mut delivered = Vec::new();
    let mut pending = Vec::new();
    for event in &human.short_term_memory {
        if event.available_at_tick <= current_tick {
            delivered.push(event.clone());
        } else {
            pending.push(event.clone());
        }
    }
    (delivered, pending)
}

/// Compact a human's short-term memory: events already delivered as of
/// `current_tick` and older than `retain_recent` are summarized into a single
/// long-term memory entry and removed from the short-term queue, keeping the
/// queue bounded. This is a deterministic, non-backend operation.
pub fn compact_memory(human: &mut HumanState, current_tick: u64, retain_recent: usize) {
    let (mut delivered, pending): (Vec<PerceivedEvent>, Vec<PerceivedEvent>) =
        delivered_and_pending(human, current_tick);
    if delivered.len() <= retain_recent {
        return;
    }
    let to_compact: Vec<PerceivedEvent> =
        delivered.drain(..delivered.len() - retain_recent).collect();
    if !to_compact.is_empty() {
        let summary = to_compact
            .iter()
            .map(|event| format!("[{}] {}: {}", event.origin_tick, event.kind, event.summary))
            .collect::<Vec<_>>()
            .join("; ");
        human.long_term_memory.push(summary);
    }
    let mut retained = delivered;
    retained.extend(pending);
    human.short_term_memory = retained;
}
