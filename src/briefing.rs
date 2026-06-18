//! Briefing output types.
//!
//! A [`Briefing`] is the final, synthesized output of `threshold brief`.
//! It holds four sections of prioritized [`BriefingItem`]s.
//!
//! ## JSON Schema
//!
//! See the crate-level documentation for the full schema.

use serde::{Deserialize, Serialize};

use crate::signal::SignalKind;

/// A single item in the synthesized briefing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BriefingItem {
    /// Semantic kind (matches the [`SignalKind`] that produced this item).
    pub kind: SignalKind,
    /// Short title line.
    pub title: String,
    /// Supporting detail.
    pub body: String,
    /// Priority 0–100.
    pub priority: u8,
    /// Name of the source that provided this signal.
    pub source: String,
    /// Age of the underlying data in seconds (if known).
    pub freshness_secs: Option<u64>,
}

/// The four sections of an arrival briefing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BriefingSections {
    /// In-flight tasks and PRDs actively being worked on.
    pub mid_flight: Vec<BriefingItem>,
    /// Blocked PRDs, open docket findings, pending reviews.
    pub owed_to_you: Vec<BriefingItem>,
    /// Dirty repos, unpushed commits, filesystem changes.
    pub changed_since_last: Vec<BriefingItem>,
    /// Already-completed or already-failed items (avoid re-doing).
    pub dont_redo: Vec<BriefingItem>,
}

/// The synthesized arrival briefing.
///
/// Serializes to JSON with `schema: "threshold.briefing.v1"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Briefing {
    /// Schema version identifier.
    pub schema: String,
    /// ISO 8601 timestamp of when this briefing was generated.
    pub generated_at: String,
    /// The four sections of items.
    pub sections: BriefingSections,
    /// Total number of items across all sections.
    pub total_items: usize,
    /// Names of sources that were queried to produce this briefing.
    pub sources_queried: Vec<String>,
}

impl Briefing {
    /// Render the briefing as human-readable text (≤4 KB target).
    #[must_use]
    pub fn render_text(&self) -> String {
        let mut out = String::with_capacity(2048);
        out.push_str("=== Arrival Briefing ===\n");
        out.push_str(&format!("Generated: {}\n\n", self.generated_at));

        render_section(&mut out, "Mid-flight", &self.sections.mid_flight);
        render_section(&mut out, "Owed to you", &self.sections.owed_to_you);
        render_section(
            &mut out,
            "Changed since last session",
            &self.sections.changed_since_last,
        );
        render_section(&mut out, "Don't redo", &self.sections.dont_redo);

        if self.total_items == 0 {
            out.push_str("(no signals — all clear)\n");
        }
        out
    }

    /// Check whether this briefing has any items at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.total_items == 0
    }
}

fn render_section(out: &mut String, title: &str, items: &[BriefingItem]) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("--- {title} ---\n"));
    for item in items {
        out.push_str(&format!("• [{}] {}\n", item.source, item.title));
        if !item.body.is_empty() {
            // Indent body lines
            for line in item.body.lines() {
                out.push_str(&format!("    {line}\n"));
            }
        }
    }
    out.push('\n');
}
