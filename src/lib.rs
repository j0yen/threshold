//! threshold — session arrival briefing synthesizer library.
//!
//! # Overview
//!
//! Provides the core types and functions for synthesizing scattered session
//! signals (recall reflective entries, gossip notes, build manifest status,
//! git dirty state, open docket findings, self-review due flag) into a single
//! prioritized [`Briefing`].
//!
//! ## Architecture
//!
//! ```text
//! [SignalSource impls]
//!        |
//!        v (Vec<Signal>)
//! synthesize()   -- pure function, no I/O
//!        |
//!        v
//!   Briefing     -- sectioned, priority-ordered, size-capped
//!        |
//!        v
//! render_text() / serde_json::to_string_pretty()
//! ```
//!
//! ## JSON Schema
//!
//! The [`Briefing`] struct serializes to:
//!
//! ```json
//! {
//!   "schema": "threshold.briefing.v1",
//!   "generated_at": "<ISO 8601>",
//!   "sections": {
//!     "mid_flight": [ <BriefingItem>, ... ],
//!     "owed_to_you": [ <BriefingItem>, ... ],
//!     "changed_since_last": [ <BriefingItem>, ... ],
//!     "dont_redo": [ <BriefingItem>, ... ]
//!   },
//!   "total_items": <integer>,
//!   "sources_queried": [ "<source_name>", ... ]
//! }
//! ```
//!
//! Each [`BriefingItem`] serializes to:
//!
//! ```json
//! {
//!   "kind": "<SignalKind>",
//!   "title": "<string>",
//!   "body": "<string>",
//!   "priority": <0-100 integer>,
//!   "source": "<source_name>",
//!   "freshness_secs": <integer or null>
//! }
//! ```

pub mod briefing;
pub mod ledger;
pub mod session_id;
pub mod signal;
pub mod sources;
pub mod synthesizer;

pub use briefing::{Briefing, BriefingItem};
pub use ledger::{OpenQuestion, OpenQuestionsOutput};
pub use signal::{Signal, SignalKind, SignalSource};
pub use sources::SourceSet;
pub use synthesizer::synthesize;
