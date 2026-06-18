//! [`ReviewDueSource`] — checks if a self-review is due.
//!
//! Backing data: presence of flag file
//! `<source_root>/.claude/skills/build/state/review-due`
//! (or `~/.claude/skills/build/state/review-due` when no root is set).
//!
//! If the flag file exists, emits one `Owed` signal. Degrades to empty on
//! any error.

use std::path::{Path, PathBuf};

use crate::signal::{Signal, SignalKind, SignalSource};

/// Checks the review-due flag file and emits an `Owed` signal if set.
pub struct ReviewDueSource {
    flag_path: PathBuf,
}

impl ReviewDueSource {
    /// Create a new `ReviewDueSource`.
    ///
    /// `source_root` overrides the filesystem root (testing seam).
    #[must_use]
    pub fn new(source_root: Option<&Path>) -> Self {
        let flag_path = if let Some(root) = source_root {
            root.join(".claude/skills/build/state/review-due")
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_owned());
            PathBuf::from(home).join(".claude/skills/build/state/review-due")
        };
        Self { flag_path }
    }
}

impl SignalSource for ReviewDueSource {
    fn collect(&self) -> Result<Vec<Signal>, anyhow::Error> {
        if !self.flag_path.exists() {
            return Ok(vec![]);
        }
        Ok(vec![Signal::new(
            SignalKind::Owed,
            "Self-review is due".to_owned(),
            "Run /self-review to complete the daily self-optimization pass.".to_owned(),
            75,
            "review-due",
        )])
    }

    fn name(&self) -> &str {
        "review-due"
    }
}
