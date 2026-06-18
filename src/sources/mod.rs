//! Concrete [`SignalSource`] implementations.
//!
//! Each source is individually fallible: if its backing data is absent or
//! malformed, it returns `Ok(vec![])` rather than propagating an error.
//! This ensures a broken source never prevents the briefing from rendering.
//!
//! ## Sources
//!
//! | Source | Backing data | Signal kinds emitted |
//! |--------|-------------|----------------------|
//! | [`RecallSource`] | `recall list --kind reflective` (last 5 lines of `~/.local/share/recall/reflective.log`) | `Owed`, `DontRedo` |
//! | [`GossipSource`] | `~/wintermute/autobuilder/notes/gossip.md` (last 500 bytes) | `InFlight`, `Owed` |
//! | [`BuildManifestSource`] | `~/.claude/skills/build/state/manifest.json` | `InFlight`, `Owed` |
//! | [`GitSource`] | `git status --short` + `git log @{u}..HEAD` under `~/wintermute/*` | `Changed` |
//! | [`DocketSource`] | `~/wintermute/autobuilder/notes/docket.md` (open findings) | `Owed` |
//! | [`ReviewDueSource`] | `~/.claude/skills/build/state/review-due` (flag file) | `Owed` |
//! | [`LedgerSource`] | `$XDG_STATE_HOME/threshold/ledger.jsonl` (open questions) | `Owed` |
//!
//! ## `--source-root` Testing Seam
//!
//! All sources accept an optional `source_root: Option<&Path>`. When set,
//! each source resolves its backing data relative to that root instead of
//! the real filesystem paths. This lets acceptance tests point at a fixture
//! directory without touching real data.
//!
//! For example, with `--source-root /tmp/fixtures`:
//! - `RecallSource` reads `/tmp/fixtures/recall/reflective.log`
//! - `GossipSource` reads `/tmp/fixtures/wintermute/autobuilder/notes/gossip.md`
//! - `BuildManifestSource` reads `/tmp/fixtures/.claude/skills/build/state/manifest.json`
//! - `GitSource` scans `/tmp/fixtures/wintermute/` for git repos
//! - `DocketSource` reads `/tmp/fixtures/wintermute/autobuilder/notes/docket.md`
//! - `ReviewDueSource` checks `/tmp/fixtures/.claude/skills/build/state/review-due`
//! - `LedgerSource` reads `/tmp/fixtures/threshold/ledger.jsonl`

pub mod build_manifest;
pub mod docket;
pub mod fake;
pub mod git;
pub mod gossip;
pub mod ledger_source;
pub mod recall;
pub mod review_due;

pub use build_manifest::BuildManifestSource;
pub use docket::DocketSource;
pub use fake::{FakeSource, SlowSource};
pub use git::GitSource;
pub use gossip::GossipSource;
pub use ledger_source::LedgerSource;
pub use recall::RecallSource;
pub use review_due::ReviewDueSource;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::signal::{Signal, SignalSource};

/// A set of all real sources configured for one `threshold brief` run.
pub struct SourceSet {
    sources: Vec<Box<dyn SignalSource>>,
}

impl SourceSet {
    /// Build a `SourceSet` with all real sources.
    ///
    /// `source_root` overrides the filesystem root for all sources (testing seam).
    #[must_use]
    pub fn real(source_root: Option<&Path>) -> Self {
        let root: Option<PathBuf> = source_root.map(Path::to_owned);
        let r = root.as_deref();
        Self {
            sources: vec![
                Box::new(RecallSource::new(r)),
                Box::new(GossipSource::new(r)),
                Box::new(BuildManifestSource::new(r)),
                Box::new(GitSource::new(r)),
                Box::new(DocketSource::new(r)),
                Box::new(ReviewDueSource::new(r)),
                Box::new(LedgerSource::new(r)),
            ],
        }
    }

    /// Collect signals from all sources, degrading gracefully on failure.
    #[must_use]
    pub fn collect_all(&self) -> Vec<Signal> {
        let mut signals = Vec::new();
        for src in &self.sources {
            if let Ok(s) = src.collect() {
                signals.extend(s);
            }
            // On Err: source failed — degraded to empty contribution
        }
        signals
    }

    /// Collect signals from all sources with an overall wall-clock deadline.
    ///
    /// Each source is queried in sequence; if the deadline is exceeded after
    /// any source completes, remaining sources are skipped and a partial
    /// briefing is returned. The deadline is enforced between sources, not
    /// within a single source call (sources are synchronous).
    ///
    /// This guarantees that a session-start hook never hangs indefinitely even
    /// if one or more sources are pathologically slow.
    #[must_use]
    pub fn collect_with_deadline(&self, deadline: Duration) -> Vec<Signal> {
        let start = Instant::now();
        let mut signals = Vec::new();
        for src in &self.sources {
            if start.elapsed() >= deadline {
                // Out of time — return partial results
                break;
            }
            if let Ok(s) = src.collect() {
                signals.extend(s);
            }
            // On Err: source failed — degraded to empty contribution
        }
        signals
    }
}
