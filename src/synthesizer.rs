//! Pure synthesizer: `Vec<Signal>` → `Briefing`.
//!
//! This function has **no I/O** in its signature. It is fully unit-testable
//! with in-memory inputs.
//!
//! ## Algorithm
//!
//! 1. **Dedup**: signals with identical `(kind, title)` pairs are collapsed
//!    to the highest-priority copy.
//! 2. **Section assignment**: each signal's `kind` maps it to exactly one
//!    section of the [`Briefing`].
//! 3. **Priority ordering**: within each section, items are sorted by
//!    `priority` descending (100 = most urgent first).
//! 4. **Cap**: if `max_items > 0`, each section is truncated to `max_items`
//!    entries (lowest-priority items dropped).

use std::collections::HashMap;

use chrono::Utc;

use crate::briefing::{Briefing, BriefingItem, BriefingSections};
use crate::signal::{Signal, SignalKind};

/// Synthesize a collection of signals into a prioritized briefing.
///
/// - `signals`: raw signals from all sources (may contain duplicates).
/// - `max_items`: cap per section (0 = unlimited).
///
/// This function is **pure**: no I/O, no side effects.
#[must_use]
pub fn synthesize(signals: Vec<Signal>, max_items: usize) -> Briefing {
    // --- Dedup: keep highest-priority signal per (kind, title) pair ---
    let mut deduped: HashMap<(String, String), Signal> = HashMap::new();
    let mut source_names: Vec<String> = Vec::new();

    for sig in signals {
        if !source_names.contains(&sig.source) {
            source_names.push(sig.source.clone());
        }
        let key = (format!("{:?}", sig.kind), sig.title.clone());
        deduped
            .entry(key)
            .and_modify(|existing| {
                if sig.priority > existing.priority {
                    *existing = sig.clone();
                }
            })
            .or_insert(sig);
    }

    // --- Section assignment ---
    let mut mid_flight: Vec<BriefingItem> = Vec::new();
    let mut owed_to_you: Vec<BriefingItem> = Vec::new();
    let mut changed_since_last: Vec<BriefingItem> = Vec::new();
    let mut dont_redo: Vec<BriefingItem> = Vec::new();

    for sig in deduped.into_values() {
        let item = BriefingItem {
            kind: sig.kind.clone(),
            title: sig.title,
            body: sig.body,
            priority: sig.priority,
            source: sig.source,
            freshness_secs: sig.freshness_secs,
        };
        match sig.kind {
            SignalKind::InFlight => mid_flight.push(item),
            SignalKind::Owed => owed_to_you.push(item),
            SignalKind::Changed => changed_since_last.push(item),
            SignalKind::DontRedo => dont_redo.push(item),
        }
    }

    // --- Priority ordering (descending) ---
    mid_flight.sort_by(|a, b| b.priority.cmp(&a.priority));
    owed_to_you.sort_by(|a, b| b.priority.cmp(&a.priority));
    changed_since_last.sort_by(|a, b| b.priority.cmp(&a.priority));
    dont_redo.sort_by(|a, b| b.priority.cmp(&a.priority));

    // --- Cap per section ---
    if max_items > 0 {
        mid_flight.truncate(max_items);
        owed_to_you.truncate(max_items);
        changed_since_last.truncate(max_items);
        dont_redo.truncate(max_items);
    }

    let total_items = mid_flight.len()
        + owed_to_you.len()
        + changed_since_last.len()
        + dont_redo.len();

    source_names.sort();

    Briefing {
        schema: "threshold.briefing.v1".to_owned(),
        generated_at: Utc::now().to_rfc3339(),
        sections: BriefingSections {
            mid_flight,
            owed_to_you,
            changed_since_last,
            dont_redo,
        },
        total_items,
        sources_queried: source_names,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::Signal;

    fn make_signal(kind: SignalKind, title: &str, priority: u8, source: &str) -> Signal {
        Signal::new(kind, title, "body", priority, source)
    }

    #[test]
    fn dedup_identical_signals_keeps_highest_priority() {
        let signals = vec![
            make_signal(SignalKind::InFlight, "task-A", 50, "s1"),
            make_signal(SignalKind::InFlight, "task-A", 90, "s2"), // higher priority
            make_signal(SignalKind::InFlight, "task-A", 30, "s3"),
        ];
        let briefing = synthesize(signals, 0);
        assert_eq!(briefing.sections.mid_flight.len(), 1);
        assert_eq!(briefing.sections.mid_flight[0].priority, 90);
    }

    #[test]
    fn priority_ordering_descending() {
        let signals = vec![
            make_signal(SignalKind::Owed, "low", 10, "src"),
            make_signal(SignalKind::Owed, "high", 80, "src"),
            make_signal(SignalKind::Owed, "mid", 50, "src"),
        ];
        let briefing = synthesize(signals, 0);
        let items = &briefing.sections.owed_to_you;
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].priority, 80);
        assert_eq!(items[1].priority, 50);
        assert_eq!(items[2].priority, 10);
    }

    #[test]
    fn section_assignment_routes_correctly() {
        let signals = vec![
            make_signal(SignalKind::InFlight, "t1", 50, "s"),
            make_signal(SignalKind::Owed, "t2", 50, "s"),
            make_signal(SignalKind::Changed, "t3", 50, "s"),
            make_signal(SignalKind::DontRedo, "t4", 50, "s"),
        ];
        let briefing = synthesize(signals, 0);
        assert_eq!(briefing.sections.mid_flight.len(), 1);
        assert_eq!(briefing.sections.owed_to_you.len(), 1);
        assert_eq!(briefing.sections.changed_since_last.len(), 1);
        assert_eq!(briefing.sections.dont_redo.len(), 1);
        assert_eq!(briefing.total_items, 4);
    }

    #[test]
    fn max_items_cap_truncates_lowest_priority() {
        let signals = vec![
            make_signal(SignalKind::InFlight, "a", 100, "s"),
            make_signal(SignalKind::InFlight, "b", 60, "s"),
            make_signal(SignalKind::InFlight, "c", 20, "s"),
        ];
        let briefing = synthesize(signals, 2);
        let items = &briefing.sections.mid_flight;
        assert_eq!(items.len(), 2);
        // The two highest-priority items survive
        assert_eq!(items[0].priority, 100);
        assert_eq!(items[1].priority, 60);
    }

    #[test]
    fn empty_input_produces_empty_briefing() {
        let briefing = synthesize(vec![], 0);
        assert!(briefing.is_empty());
        assert_eq!(briefing.total_items, 0);
    }

    #[test]
    fn max_items_zero_means_unlimited() {
        let signals: Vec<Signal> = (0u8..50)
            .map(|i| make_signal(SignalKind::InFlight, &format!("t{i}"), i, "s"))
            .collect();
        let briefing = synthesize(signals, 0);
        assert_eq!(briefing.sections.mid_flight.len(), 50);
    }
}
