//! [`RecallSource`] — reads from the recall reflective log.
//!
//! Backing data: last 5 lines of
//! `<source_root>/recall/reflective.log`
//! (or `~/.local/share/recall/reflective.log` when no root is set).
//!
//! Degrades to empty on any error (missing file, parse failure, etc.).

use std::path::{Path, PathBuf};

use crate::signal::{Signal, SignalKind, SignalSource};

/// Reads the recall reflective log and emits `Owed` signals for recent entries.
pub struct RecallSource {
    log_path: PathBuf,
}

impl RecallSource {
    /// Create a new `RecallSource`.
    ///
    /// `source_root` overrides the filesystem root (testing seam; see module docs).
    #[must_use]
    pub fn new(source_root: Option<&Path>) -> Self {
        let log_path = if let Some(root) = source_root {
            root.join("recall/reflective.log")
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_owned());
            PathBuf::from(home).join(".local/share/recall/reflective.log")
        };
        Self { log_path }
    }
}

impl SignalSource for RecallSource {
    fn collect(&self) -> Result<Vec<Signal>, anyhow::Error> {
        let content = match std::fs::read_to_string(&self.log_path) {
            Ok(c) => c,
            Err(_) => return Ok(vec![]),
        };

        let signals: Vec<Signal> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .rev()
            .take(5)
            .map(|line| {
                Signal::new(
                    SignalKind::Owed,
                    truncate(line, 80),
                    String::new(),
                    40,
                    "recall",
                )
            })
            .collect();

        Ok(signals)
    }

    fn name(&self) -> &str {
        "recall"
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
