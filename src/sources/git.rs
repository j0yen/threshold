//! [`GitSource`] — scans `~/wintermute/*/` for dirty or unpushed repos.
//!
//! Backing data:
//! - `git status --short` for dirty working trees
//! - `git log @{u}..HEAD --oneline` for unpushed commits
//!
//! With `--source-root`, scans `<source_root>/wintermute/` instead of
//! `~/wintermute/`.
//!
//! Emits `Changed` signals. Degrades to empty on any error.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::signal::{Signal, SignalKind, SignalSource};

/// Scans wintermute repos and emits `Changed` signals for dirty/unpushed state.
pub struct GitSource {
    wintermute_dir: PathBuf,
}

impl GitSource {
    /// Create a new `GitSource`.
    ///
    /// `source_root` overrides the filesystem root (testing seam).
    #[must_use]
    pub fn new(source_root: Option<&Path>) -> Self {
        let wintermute_dir = if let Some(root) = source_root {
            root.join("wintermute")
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_owned());
            PathBuf::from(home).join("wintermute")
        };
        Self { wintermute_dir }
    }
}

impl SignalSource for GitSource {
    fn collect(&self) -> Result<Vec<Signal>, anyhow::Error> {
        let entries = match std::fs::read_dir(&self.wintermute_dir) {
            Ok(e) => e,
            Err(_) => return Ok(vec![]),
        };

        let mut signals = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if !path.join(".git").exists() {
                continue;
            }

            let repo_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_owned();

            // Check dirty state
            if let Some(dirty_count) = git_dirty_count(&path) {
                if dirty_count > 0 {
                    signals.push(
                        Signal::new(
                            SignalKind::Changed,
                            format!("{repo_name}: {dirty_count} uncommitted change(s)"),
                            String::new(),
                            55,
                            "git",
                        )
                        .with_freshness(0),
                    );
                }
            }

            // Check unpushed commits
            if let Some(unpushed) = git_unpushed_count(&path) {
                if unpushed > 0 {
                    signals.push(
                        Signal::new(
                            SignalKind::Changed,
                            format!("{repo_name}: {unpushed} unpushed commit(s)"),
                            String::new(),
                            50,
                            "git",
                        )
                        .with_freshness(0),
                    );
                }
            }
        }

        Ok(signals)
    }

    fn name(&self) -> &str {
        "git"
    }
}

fn git_dirty_count(repo: &Path) -> Option<usize> {
    let out = Command::new("git")
        .args(["status", "--short"])
        .current_dir(repo)
        .output()
        .ok()?;
    if !out.status.success() && out.stdout.is_empty() {
        return None;
    }
    let stdout = String::from_utf8(out.stdout).ok()?;
    Some(stdout.lines().filter(|l| !l.trim().is_empty()).count())
}

fn git_unpushed_count(repo: &Path) -> Option<usize> {
    let out = Command::new("git")
        .args(["log", "@{u}..HEAD", "--oneline"])
        .current_dir(repo)
        .output()
        .ok()?;
    // If there's no upstream, git returns exit code 128 — treat as 0 unpushed
    if !out.status.success() {
        return Some(0);
    }
    let stdout = String::from_utf8(out.stdout).ok()?;
    Some(stdout.lines().filter(|l| !l.trim().is_empty()).count())
}
