//! AC3: The synthesizer is a pure function (no I/O in its signature) and is
//! covered by unit tests for: dedup of identical signals, priority ordering,
//! section assignment, and the --max-items cap.
//!
//! Note: the synthesizer's own #[cfg(test)] module also covers these cases.
//! These acceptance tests provide an additional layer that exercises the public API.

use threshold::{
    signal::{Signal, SignalKind},
    synthesize,
};

fn sig(kind: SignalKind, title: &str, priority: u8) -> Signal {
    Signal::new(kind, title, "body", priority, "test-source")
}

#[test]
fn acceptance_dedup_keeps_highest_priority() {
    let signals = vec![
        sig(SignalKind::Owed, "open-finding-A", 30),
        sig(SignalKind::Owed, "open-finding-A", 80), // duplicate, higher priority
        sig(SignalKind::Owed, "open-finding-A", 10),
    ];
    let briefing = synthesize(signals, 0);
    // Should collapse to exactly one item with priority 80
    assert_eq!(briefing.sections.owed_to_you.len(), 1);
    assert_eq!(briefing.sections.owed_to_you[0].priority, 80);
}

#[test]
fn acceptance_priority_ordering() {
    let signals = vec![
        sig(SignalKind::Changed, "repo-z", 20),
        sig(SignalKind::Changed, "repo-a", 95),
        sig(SignalKind::Changed, "repo-m", 50),
    ];
    let briefing = synthesize(signals, 0);
    let items = &briefing.sections.changed_since_last;
    assert_eq!(items.len(), 3);
    assert!(
        items[0].priority >= items[1].priority && items[1].priority >= items[2].priority,
        "items must be sorted descending by priority"
    );
    assert_eq!(items[0].title, "repo-a"); // highest priority
}

#[test]
fn acceptance_section_assignment() {
    let signals = vec![
        sig(SignalKind::InFlight, "in-flight-item", 70),
        sig(SignalKind::Owed, "owed-item", 60),
        sig(SignalKind::Changed, "changed-item", 55),
        sig(SignalKind::DontRedo, "dont-redo-item", 40),
    ];
    let briefing = synthesize(signals, 0);

    // Each goes to the correct section
    assert_eq!(briefing.sections.mid_flight.len(), 1);
    assert_eq!(briefing.sections.mid_flight[0].title, "in-flight-item");

    assert_eq!(briefing.sections.owed_to_you.len(), 1);
    assert_eq!(briefing.sections.owed_to_you[0].title, "owed-item");

    assert_eq!(briefing.sections.changed_since_last.len(), 1);
    assert_eq!(briefing.sections.changed_since_last[0].title, "changed-item");

    assert_eq!(briefing.sections.dont_redo.len(), 1);
    assert_eq!(briefing.sections.dont_redo[0].title, "dont-redo-item");

    assert_eq!(briefing.total_items, 4);
}

#[test]
fn acceptance_max_items_cap() {
    let signals: Vec<Signal> = (0u8..10)
        .map(|i| sig(SignalKind::InFlight, &format!("task-{i:02}"), i))
        .collect();

    let briefing = synthesize(signals, 3);
    // Should keep the 3 highest-priority items (priority 9, 8, 7)
    let items = &briefing.sections.mid_flight;
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].priority, 9);
    assert_eq!(items[1].priority, 8);
    assert_eq!(items[2].priority, 7);
}

#[test]
fn synthesize_is_deterministic_on_same_input() {
    // Pure function: same input always produces same output
    let signals = || vec![
        sig(SignalKind::InFlight, "task-X", 80),
        sig(SignalKind::Owed, "finding-Y", 60),
        sig(SignalKind::Changed, "repo-Z", 40),
    ];
    let b1 = synthesize(signals(), 5);
    let b2 = synthesize(signals(), 5);

    // Section contents should be identical (total_items, priorities)
    assert_eq!(b1.total_items, b2.total_items);
    assert_eq!(b1.sections.mid_flight.len(), b2.sections.mid_flight.len());
    assert_eq!(b1.sections.owed_to_you.len(), b2.sections.owed_to_you.len());
    assert_eq!(b1.sections.changed_since_last.len(), b2.sections.changed_since_last.len());
}
