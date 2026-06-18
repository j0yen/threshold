//! Core signal types for threshold.
//!
//! A [`Signal`] is a single piece of information from one source. The
//! [`SignalSource`] trait is implemented by every data source that can
//! contribute to the briefing.

use serde::{Deserialize, Serialize};

/// The semantic category of a signal, used for section assignment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
    /// An in-flight task or PRD that is actively being worked on.
    InFlight,
    /// Something owed: a blocked PRD, open docket finding, or pending review.
    Owed,
    /// A change since the last session: dirty git repo, unpushed commits.
    Changed,
    /// A "don't redo" reminder: already-completed or already-failed items.
    DontRedo,
}

/// A single signal emitted by one source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    /// Semantic category (determines section placement in the briefing).
    pub kind: SignalKind,
    /// Short title line (≤80 chars recommended).
    pub title: String,
    /// Supporting detail (may be multi-line; keep concise).
    pub body: String,
    /// Priority 0–100 (100 = most urgent). Used for ordering within a section.
    pub priority: u8,
    /// Identifier of the source that produced this signal (e.g. "recall", "git").
    pub source: String,
    /// Age of the underlying data in seconds, if known.
    pub freshness_secs: Option<u64>,
}

impl Signal {
    /// Create a new signal.
    #[must_use]
    pub fn new(
        kind: SignalKind,
        title: impl Into<String>,
        body: impl Into<String>,
        priority: u8,
        source: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            title: title.into(),
            body: body.into(),
            priority,
            source: source.into(),
            freshness_secs: None,
        }
    }

    /// Set freshness in seconds.
    #[must_use]
    pub fn with_freshness(mut self, secs: u64) -> Self {
        self.freshness_secs = Some(secs);
        self
    }
}

/// A source of signals. Every concrete source implements this trait.
///
/// Implementations **must not panic** when backing data is missing or
/// malformed — they should return `Ok(vec![])` instead.
pub trait SignalSource: Send + Sync {
    /// Collect signals from this source. Returns `Ok(vec![])` on any error
    /// (missing file, parse failure, etc.) so a broken source never sinks
    /// the whole briefing.
    ///
    /// # Errors
    ///
    /// This method never returns `Err` in the public contract — sources are
    /// expected to degrade gracefully. The return type is `Result` to allow
    /// future sources to surface structured diagnostics without a breaking
    /// change.
    fn collect(&self) -> Result<Vec<Signal>, anyhow::Error>;

    /// Human-readable name of this source (used in diagnostic output).
    fn name(&self) -> &str;
}
