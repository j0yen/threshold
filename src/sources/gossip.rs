//! [`GossipSource`] — reads from the wintermute gossip file.
//!
//! Backing data: last 500 bytes of
//! `<source_root>/wintermute/autobuilder/notes/gossip.md`
//! (or `~/wintermute/autobuilder/notes/gossip.md` when no root is set).
//!
//! Degrades to empty on any error.

use std::path::{Path, PathBuf};

use crate::signal::{Signal, SignalKind, SignalSource};

/// Reads the gossip.md file and emits `InFlight` signals for recent notes.
pub struct GossipSource {
    gossip_path: PathBuf,
}

impl GossipSource {
    /// Create a new `GossipSource`.
    ///
    /// `source_root` overrides the filesystem root (testing seam).
    #[must_use]
    pub fn new(source_root: Option<&Path>) -> Self {
        let gossip_path = source_root.map_or_else(
            || {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_owned());
                PathBuf::from(home).join("wintermute/autobuilder/notes/gossip.md")
            },
            |root| root.join("wintermute/autobuilder/notes/gossip.md"),
        );
        Self { gossip_path }
    }
}

impl SignalSource for GossipSource {
    fn collect(&self) -> Result<Vec<Signal>, anyhow::Error> {
        let Ok(content) = std::fs::read_to_string(&self.gossip_path) else {
            return Ok(vec![]);
        };

        // Take last 500 bytes worth of content (whole lines only)
        let tail: &str = if content.len() > 500 {
            let bytes = content.as_bytes();
            let start = content.len() - 500;
            // Back up to the next newline boundary
            let adjusted_start = bytes
                .get(start..)
                .and_then(|slice| slice.iter().position(|&b| b == b'\n'))
                .map_or(start, |p| start + p + 1);
            content.get(adjusted_start..).unwrap_or(&content)
        } else {
            &content
        };

        let signals: Vec<Signal> = tail
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .take(3)
            .map(|line| {
                Signal::new(
                    SignalKind::InFlight,
                    truncate(line, 80),
                    String::new(),
                    30,
                    "gossip",
                )
            })
            .collect();

        Ok(signals)
    }

    fn name(&self) -> &str {
        "gossip"
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
