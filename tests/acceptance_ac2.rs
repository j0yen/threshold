//! AC2: threshold brief --format json against a FakeSource fixture set
//! produces a Briefing with the documented sections and a stable, documented
//! JSON schema validated by a test.

use threshold::{
    briefing::BriefingItem,
    signal::{Signal, SignalKind},
    sources::FakeSource,
    synthesize,
};

fn make_fixture_signals() -> Vec<Signal> {
    vec![
        Signal::new(SignalKind::InFlight, "PRD in progress: foo", "working on it", 70, "fake"),
        Signal::new(SignalKind::Owed, "Review pending", "needs attention", 60, "fake"),
        Signal::new(SignalKind::Changed, "repo-a: 3 uncommitted changes", "", 55, "fake"),
        Signal::new(SignalKind::DontRedo, "Already shipped: bar", "", 40, "fake"),
    ]
}

#[test]
fn fake_source_produces_briefing_with_all_sections() {
    let signals = make_fixture_signals();
    let briefing = synthesize(signals, 0);

    // Schema field
    assert_eq!(briefing.schema, "threshold.briefing.v1");

    // All four sections exist (as struct fields, always present)
    assert!(!briefing.sections.mid_flight.is_empty(), "mid_flight should have items");
    assert!(!briefing.sections.owed_to_you.is_empty(), "owed_to_you should have items");
    assert!(!briefing.sections.changed_since_last.is_empty(), "changed_since_last should have items");
    assert!(!briefing.sections.dont_redo.is_empty(), "dont_redo should have items");

    assert_eq!(briefing.total_items, 4);
}

#[test]
fn json_schema_has_required_fields() {
    let signals = make_fixture_signals();
    let briefing = synthesize(signals, 0);

    let json_str = serde_json::to_string_pretty(&briefing)
        .expect("briefing must serialize to JSON");
    let json: serde_json::Value = serde_json::from_str(&json_str)
        .expect("serialized briefing must deserialize");

    // Top-level required fields
    assert!(json.get("schema").is_some(), "schema field missing");
    assert!(json.get("generated_at").is_some(), "generated_at field missing");
    assert!(json.get("sections").is_some(), "sections field missing");
    assert!(json.get("total_items").is_some(), "total_items field missing");
    assert!(json.get("sources_queried").is_some(), "sources_queried field missing");

    // Sections object has all four keys
    let sections = json.get("sections").and_then(|v| v.as_object())
        .expect("sections must be an object");
    assert!(sections.contains_key("mid_flight"), "sections.mid_flight missing");
    assert!(sections.contains_key("owed_to_you"), "sections.owed_to_you missing");
    assert!(sections.contains_key("changed_since_last"), "sections.changed_since_last missing");
    assert!(sections.contains_key("dont_redo"), "sections.dont_redo missing");

    // Each section is an array of items with the required fields
    for (section_name, section_val) in sections {
        let items = section_val.as_array()
            .unwrap_or_else(|| panic!("{section_name} must be an array"));
        for item in items {
            assert!(item.get("kind").is_some(), "{section_name} item missing 'kind'");
            assert!(item.get("title").is_some(), "{section_name} item missing 'title'");
            assert!(item.get("body").is_some(), "{section_name} item missing 'body'");
            assert!(item.get("priority").is_some(), "{section_name} item missing 'priority'");
            assert!(item.get("source").is_some(), "{section_name} item missing 'source'");
        }
    }
}

#[test]
fn fake_source_implements_signal_source_trait() {
    // Verify FakeSource works as a SignalSource through the public trait API
    use threshold::signal::SignalSource;

    let signals = vec![
        Signal::new(SignalKind::InFlight, "task-x", "detail", 80, "fake-src"),
    ];
    let src = FakeSource::new("fake-src", signals);
    let result = src.collect().expect("FakeSource must not fail");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].title, "task-x");
    assert_eq!(src.name(), "fake-src");
}

#[test]
fn briefing_item_round_trips_json() {
    let item = BriefingItem {
        kind: SignalKind::InFlight,
        title: "test title".to_owned(),
        body: "test body".to_owned(),
        priority: 75,
        source: "test-source".to_owned(),
        freshness_secs: Some(3600),
    };

    let json = serde_json::to_string(&item).expect("must serialize");
    let decoded: BriefingItem = serde_json::from_str(&json).expect("must deserialize");
    assert_eq!(decoded, item);
}
