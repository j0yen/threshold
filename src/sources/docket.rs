//! [`DocketSource`] — reads open findings from the docket file.
//!
//! Backing data: `<source_root>/wintermute/autobuilder/notes/docket.md`
//! (or `~/wintermute/autobuilder/notes/docket.md` when no root is set).
//!
//! Lines that begin with `- [ ]` are open findings; each becomes an `Owed`
//! signal. Degrades to empty on any error.

use std::path::{Path, PathBuf};

use crate::signal::{Signal, SignalKind, SignalSource};

/// Reads open findings from docket.md and emits `Owed` signals.
pub struct DocketSource {
    docket_path: PathBuf,
}

impl DocketSource {
    /// Create a new `DocketSource`.
    ///
    /// `source_root` overrides the filesystem root (testing seam).
    #[must_use]
    pub fn new(source_root: Option<&Path>) -> Self {
        let docket_path = source_root.map_or_else(
            || {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_owned());
                PathBuf::from(home).join("wintermute/autobuilder/notes/docket.md")
            },
            |root| root.join("wintermute/autobuilder/notes/docket.md"),
        );
        Self { docket_path }
    }
}

impl SignalSource for DocketSource {
    fn collect(&self) -> Result<Vec<Signal>, anyhow::Error> {
        let Ok(content) = std::fs::read_to_string(&self.docket_path) else {
            return Ok(vec![]);
        };

        let signals: Vec<Signal> = content
            .lines()
            .filter(|l| l.trim_start().starts_with("- [ ]"))
            .take(10)
            .map(|line| {
                let title = line.trim_start_matches("- [ ]").trim();
                Signal::new(
                    SignalKind::Owed,
                    truncate(title, 80),
                    String::new(),
                    45,
                    "docket",
                )
            })
            .collect();

        Ok(signals)
    }

    fn name(&self) -> &str {
        "docket"
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
