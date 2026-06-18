//! Property-based invariant tests for threshold's synthesizer.
//!
//! Read-only after scaffold. The edit-agent must NOT modify proptests.

use proptest::prelude::*;
use threshold::{
    signal::{Signal, SignalKind},
    synthesize,
};

fn arb_signal_kind() -> impl Strategy<Value = SignalKind> {
    prop_oneof![
        Just(SignalKind::InFlight),
        Just(SignalKind::Owed),
        Just(SignalKind::Changed),
        Just(SignalKind::DontRedo),
    ]
}

fn arb_signal() -> impl Strategy<Value = Signal> {
    (arb_signal_kind(), "[a-z]{1,20}", 0u8..=100u8).prop_map(|(kind, title, priority)| {
        Signal::new(kind, title, "body", priority, "proptest")
    })
}

proptest! {
    #[test]
    fn total_items_matches_section_sum(signals in prop::collection::vec(arb_signal(), 0..50)) {
        let briefing = synthesize(signals, 0);
        let sum = briefing.sections.mid_flight.len()
            + briefing.sections.owed_to_you.len()
            + briefing.sections.changed_since_last.len()
            + briefing.sections.dont_redo.len();
        prop_assert_eq!(briefing.total_items, sum);
    }

    #[test]
    fn priority_ordering_always_descending(signals in prop::collection::vec(arb_signal(), 1..30)) {
        let briefing = synthesize(signals, 0);
        for section in [
            &briefing.sections.mid_flight,
            &briefing.sections.owed_to_you,
            &briefing.sections.changed_since_last,
            &briefing.sections.dont_redo,
        ] {
            for window in section.windows(2) {
                prop_assert!(
                    window[0].priority >= window[1].priority,
                    "section items must be sorted descending by priority"
                );
            }
        }
    }

    #[test]
    fn max_items_cap_is_respected(
        signals in prop::collection::vec(arb_signal(), 0..50),
        max in 1usize..10
    ) {
        let briefing = synthesize(signals, max);
        prop_assert!(briefing.sections.mid_flight.len() <= max);
        prop_assert!(briefing.sections.owed_to_you.len() <= max);
        prop_assert!(briefing.sections.changed_since_last.len() <= max);
        prop_assert!(briefing.sections.dont_redo.len() <= max);
    }

    #[test]
    fn dedup_never_increases_item_count(
        signals in prop::collection::vec(arb_signal(), 0..50)
    ) {
        let input_count = signals.len();
        let briefing = synthesize(signals, 0);
        prop_assert!(briefing.total_items <= input_count,
            "dedup must never increase total items");
    }
}
