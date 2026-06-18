//! [`BuildManifestSource`] — reads the /build manifest for in-flight PRDs.
//!
//! Backing data: `<source_root>/.claude/skills/build/state/manifest.json`
//! (or `~/.claude/skills/build/state/manifest.json` when no root is set).
//!
//! Emits:
//! - `InFlight` signals for PRDs with `status: "in_progress"`
//! - `Owed` signals for PRDs with `status: "blocked"`
//!
//! Degrades to empty on any error.

use std::path::{Path, PathBuf};

use crate::signal::{Signal, SignalKind, SignalSource};

/// Reads the build manifest and emits signals for in-flight and blocked PRDs.
pub struct BuildManifestSource {
    manifest_path: PathBuf,
}

impl BuildManifestSource {
    /// Create a new `BuildManifestSource`.
    ///
    /// `source_root` overrides the filesystem root (testing seam).
    #[must_use]
    pub fn new(source_root: Option<&Path>) -> Self {
        let manifest_path = source_root.map_or_else(
            || {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_owned());
                PathBuf::from(home).join(".claude/skills/build/state/manifest.json")
            },
            |root| root.join(".claude/skills/build/state/manifest.json"),
        );
        Self { manifest_path }
    }
}

impl SignalSource for BuildManifestSource {
    fn collect(&self) -> Result<Vec<Signal>, anyhow::Error> {
        let Ok(content) = std::fs::read_to_string(&self.manifest_path) else {
            return Ok(vec![]);
        };

        let manifest: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return Ok(vec![]),
        };

        let Some(prds) = manifest.get("prds").and_then(|v| v.as_array()) else {
            return Ok(vec![]);
        };

        let mut signals = Vec::new();
        for prd in prds {
            let slug = prd
                .get("slug")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let status = prd
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            match status {
                "in_progress" => {
                    signals.push(Signal::new(
                        SignalKind::InFlight,
                        format!("PRD in progress: {slug}"),
                        String::new(),
                        70,
                        "build-manifest",
                    ));
                }
                "blocked" => {
                    signals.push(Signal::new(
                        SignalKind::Owed,
                        format!("PRD blocked: {slug}"),
                        String::new(),
                        60,
                        "build-manifest",
                    ));
                }
                _ => {}
            }
        }

        Ok(signals)
    }

    fn name(&self) -> &str {
        "build-manifest"
    }
}
