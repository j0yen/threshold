//! `threshold verify` — cross-check claims in the latest reflective letter against live ground truth.
//!
//! # Architecture
//!
//! ```text
//! letter text
//!      |
//!      v
//! extract_claims()   -- heuristic bullet/line parser
//!      |
//!      v  Vec<Claim>
//! verify_claims()    -- dispatch to per-kind verifier
//!      |
//!      v  Vec<ClaimVerdict>
//! render_text() / serde_json::to_string_pretty()
//! ```
//!
//! ## JSON Schema
//!
//! `threshold verify --format json` emits:
//!
//! ```json
//! [
//!   {
//!     "claim": "<text extracted from letter>",
//!     "kind": "<ClaimKind>",
//!     "status": "<confirmed|stale|contradicted|unverifiable>",
//!     "evidence": "<human-readable citation of what was checked>"
//!   },
//!   ...
//! ]
//! ```

pub mod claim;
pub mod render;
pub mod verdict;
pub mod verifier;

pub use claim::{Claim, ClaimKind, extract_claims};
pub use render::render_text;
pub use verdict::{ClaimVerdict, VerdictStatus};
pub use verifier::{VerifyOptions, verify_claims};
