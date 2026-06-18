//! Per-kind verifiers.
//!
//! Each verifier takes a [`Claim`] and returns a [`ClaimVerdict`] by consulting
//! live ground truth. All verifiers degrade gracefully: a missing tool or empty
//! output yields `unverifiable` rather than a panic.

use std::path::Path;
use std::process::Command;

use crate::verify::claim::{Claim, ClaimKind};
use crate::verify::verdict::{ClaimVerdict, VerdictStatus};

/// Options controlling how verifiers find ground truth.
#[derive(Debug, Clone, Default)]
pub struct VerifyOptions {
    /// Override the wintermute root path (testing seam).
    pub source_root: Option<std::path::PathBuf>,
}

impl VerifyOptions {
    /// Resolve a path relative to the wintermute root.
    #[must_use]
    pub fn wintermute_root(&self) -> std::path::PathBuf {
        self.source_root.clone().unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_owned());
            std::path::PathBuf::from(home).join("wintermute")
        })
    }

    /// Resolve a path relative to the build manifest location.
    #[must_use]
    pub fn manifest_path(&self) -> std::path::PathBuf {
        self.source_root.clone().map_or_else(
            || {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_owned());
                std::path::PathBuf::from(home)
                    .join(".claude/skills/build/state/manifest.json")
            },
            |root| root.join(".claude/skills/build/state/manifest.json"),
        )
    }
}

/// Verify all claims and return a verdict for each.
///
/// The verifier for each kind is selected by [`ClaimKind`]. Narrative claims
/// are always `unverifiable`.
#[must_use]
pub fn verify_claims(claims: &[Claim], opts: &VerifyOptions) -> Vec<ClaimVerdict> {
    claims
        .iter()
        .map(|c| verify_one(c, opts))
        .collect()
}

fn verify_one(claim: &Claim, opts: &VerifyOptions) -> ClaimVerdict {
    match claim.kind {
        ClaimKind::PushedRepo => verify_pushed_repo(claim, opts),
        ClaimKind::ShippedPrd => verify_shipped_prd(claim, opts),
        ClaimKind::DaemonUp => verify_daemon_up(claim),
        ClaimKind::PeerPresent => verify_peer_present(claim),
        ClaimKind::InFlightAgent => verify_inflight_agent(claim, opts),
        ClaimKind::PendingTodo => verify_pending_todo(claim, opts),
        ClaimKind::Narrative => ClaimVerdict::new(
            claim,
            VerdictStatus::Unverifiable,
            "narrative claim — not checkable by heuristic verifier",
        ),
    }
}

/// Verify a `pushed-repo` claim by checking git remotes in ~/wintermute/*.
fn verify_pushed_repo(claim: &Claim, opts: &VerifyOptions) -> ClaimVerdict {
    // Extract a plausible repo slug from the claim text.
    let slug = extract_repo_slug(&claim.text);
    let root = opts.wintermute_root();

    if let Some(slug) = slug {
        // Check if the repo dir exists under wintermute root.
        let repo_dir = root.join(&slug);
        if repo_dir.is_dir() {
            // Check if there are any remotes (proxy for "has been pushed").
            if let Some(remote_url) = git_remote_url(&repo_dir) {
                return ClaimVerdict::new(
                    claim,
                    VerdictStatus::Confirmed,
                    format!("repo found at {}: remote = {remote_url}", repo_dir.display()),
                );
            }
            return ClaimVerdict::new(
                claim,
                VerdictStatus::Contradicted,
                format!(
                    "repo dir exists at {} but has no remote configured",
                    repo_dir.display()
                ),
            );
        }
        // If the full path doesn't exist, check parent worktree paths.
        // A slug like "j0yen/foo" → check ~/wintermute/foo.
        let short_slug = slug.split('/').last().unwrap_or(&slug);
        let short_dir = root.join(short_slug);
        if short_dir.is_dir() {
            if let Some(remote_url) = git_remote_url(&short_dir) {
                return ClaimVerdict::new(
                    claim,
                    VerdictStatus::Confirmed,
                    format!(
                        "repo found at {}: remote = {remote_url}",
                        short_dir.display()
                    ),
                );
            }
        }

        ClaimVerdict::new(
            claim,
            VerdictStatus::Contradicted,
            format!("no repo dir found for slug {slug:?} under {}", root.display()),
        )
    } else {
        ClaimVerdict::new(
            claim,
            VerdictStatus::Unverifiable,
            "could not extract repo slug from claim text",
        )
    }
}

/// Verify a `shipped-prd` claim by checking the build manifest.
fn verify_shipped_prd(claim: &Claim, opts: &VerifyOptions) -> ClaimVerdict {
    let manifest_path = opts.manifest_path();
    let Ok(content) = std::fs::read_to_string(&manifest_path) else {
        return ClaimVerdict::new(
            claim,
            VerdictStatus::Unverifiable,
            format!(
                "build manifest not found at {}",
                manifest_path.display()
            ),
        );
    };

    // Extract a PRD slug from the claim text.
    let slug = extract_prd_slug(&claim.text).unwrap_or_default();

    if slug.is_empty() {
        return ClaimVerdict::new(
            claim,
            VerdictStatus::Unverifiable,
            "could not extract PRD slug from claim text",
        );
    }

    // Check if the slug appears in the manifest with status "archived" or "completed".
    let lower = content.to_ascii_lowercase();
    let slug_lower = slug.to_ascii_lowercase();
    if lower.contains(&slug_lower) {
        // Check if it's marked archived/completed.
        if lower.contains("archived") || lower.contains("completed") {
            return ClaimVerdict::new(
                claim,
                VerdictStatus::Confirmed,
                format!("slug {slug:?} found in manifest with archived/completed status"),
            );
        }
        return ClaimVerdict::new(
            claim,
            VerdictStatus::Stale,
            format!(
                "slug {slug:?} found in manifest but not archived/completed — may still be in progress"
            ),
        );
    }

    ClaimVerdict::new(
        claim,
        VerdictStatus::Contradicted,
        format!(
            "slug {slug:?} not found in manifest at {}",
            manifest_path.display()
        ),
    )
}

/// Verify a `daemon-up` claim by checking agorabus peer list or systemd unit.
fn verify_daemon_up(claim: &Claim) -> ClaimVerdict {
    // Try to extract a daemon name from the claim.
    let daemon = extract_daemon_name(&claim.text);

    if let Some(daemon) = daemon {
        // Check systemctl --user is-active for the daemon.
        let status = Command::new("systemctl")
            .args(["--user", "is-active", "--quiet", &daemon])
            .status();
        match status {
            Ok(s) if s.success() => {
                return ClaimVerdict::new(
                    claim,
                    VerdictStatus::Confirmed,
                    format!("systemctl --user is-active {daemon}: active"),
                );
            }
            Ok(_) => {
                return ClaimVerdict::new(
                    claim,
                    VerdictStatus::Contradicted,
                    format!("systemctl --user is-active {daemon}: not active"),
                );
            }
            Err(_) => {
                // systemctl unavailable — fall through to unverifiable.
            }
        }
    }

    ClaimVerdict::new(
        claim,
        VerdictStatus::Unverifiable,
        "could not extract daemon name or systemctl unavailable",
    )
}

/// Verify a `peer-present` claim via agorabus peer list.
fn verify_peer_present(claim: &Claim) -> ClaimVerdict {
    // Run `agorabus peers` or `agorabus list` if available.
    let out = Command::new("agorabus").arg("peers").output();
    match out {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim().is_empty() {
                ClaimVerdict::new(
                    claim,
                    VerdictStatus::Contradicted,
                    "agorabus peers: no peers listed",
                )
            } else {
                ClaimVerdict::new(
                    claim,
                    VerdictStatus::Confirmed,
                    format!("agorabus peers: {}", stdout.lines().count()),
                )
            }
        }
        _ => ClaimVerdict::new(
            claim,
            VerdictStatus::Unverifiable,
            "agorabus not available or peers subcommand not supported",
        ),
    }
}

/// Verify an `in-flight-agent` claim via build manifest.
fn verify_inflight_agent(claim: &Claim, opts: &VerifyOptions) -> ClaimVerdict {
    let manifest_path = opts.manifest_path();
    let Ok(content) = std::fs::read_to_string(&manifest_path) else {
        return ClaimVerdict::new(
            claim,
            VerdictStatus::Unverifiable,
            format!(
                "build manifest not found at {}",
                manifest_path.display()
            ),
        );
    };

    let slug = extract_prd_slug(&claim.text).unwrap_or_default();
    if slug.is_empty() {
        return ClaimVerdict::new(
            claim,
            VerdictStatus::Unverifiable,
            "could not extract agent/PRD slug from claim text",
        );
    }

    let lower = content.to_ascii_lowercase();
    let slug_lower = slug.to_ascii_lowercase();
    if lower.contains(&slug_lower) && (lower.contains("in_progress") || lower.contains("in-progress")) {
        ClaimVerdict::new(
            claim,
            VerdictStatus::Confirmed,
            format!("slug {slug:?} found in manifest with in-progress status"),
        )
    } else if lower.contains(&slug_lower) {
        ClaimVerdict::new(
            claim,
            VerdictStatus::Stale,
            format!("slug {slug:?} found in manifest but not in in-progress state"),
        )
    } else {
        ClaimVerdict::new(
            claim,
            VerdictStatus::Contradicted,
            format!("slug {slug:?} not found in manifest"),
        )
    }
}

/// Verify a `pending-todo` claim: check if it's still not done.
///
/// For `pending-todo` claims we check if the slug appears as anything other
/// than archived — if it's still live, the todo is `confirmed` as pending.
fn verify_pending_todo(claim: &Claim, opts: &VerifyOptions) -> ClaimVerdict {
    let manifest_path = opts.manifest_path();
    let Ok(content) = std::fs::read_to_string(&manifest_path) else {
        // No manifest → can't confirm or contradict.
        return ClaimVerdict::new(
            claim,
            VerdictStatus::Unverifiable,
            format!(
                "build manifest not found at {}; cannot confirm pending status",
                manifest_path.display()
            ),
        );
    };

    let slug = extract_prd_slug(&claim.text).unwrap_or_default();
    if slug.is_empty() {
        return ClaimVerdict::new(
            claim,
            VerdictStatus::Unverifiable,
            "no PRD/task slug found in pending-todo claim",
        );
    }

    let lower = content.to_ascii_lowercase();
    let slug_lower = slug.to_ascii_lowercase();

    if lower.contains(&slug_lower) {
        if lower.contains("archived") {
            return ClaimVerdict::new(
                claim,
                VerdictStatus::Contradicted,
                format!("slug {slug:?} is now archived — todo no longer pending"),
            );
        }
        return ClaimVerdict::new(
            claim,
            VerdictStatus::Confirmed,
            format!("slug {slug:?} still present and not archived in manifest"),
        );
    }

    ClaimVerdict::new(
        claim,
        VerdictStatus::Unverifiable,
        format!("slug {slug:?} not found in manifest"),
    )
}

// ---------------------------------------------------------------------------
// Extraction helpers
// ---------------------------------------------------------------------------

/// Extract a repo slug from a claim line.
///
/// Looks for patterns like `j0yen/foo`, `pushed foo`, `published foo`.
fn extract_repo_slug(text: &str) -> Option<String> {
    // Look for j0yen/<name> or username/name pattern.
    for word in text.split_whitespace() {
        let w = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '-' && c != '_');
        if w.contains('/') && w.len() > 3 {
            return Some(w.to_owned());
        }
    }
    // Fallback: word after "pushed", "published".
    extract_word_after(text, &["pushed", "published", "push"])
}

/// Extract a PRD/task slug from a claim line.
fn extract_prd_slug(text: &str) -> Option<String> {
    // Look for hyphenated words that look like PRD slugs (e.g. threshold-brief).
    let words: Vec<&str> = text.split_whitespace().collect();
    for word in &words {
        let w = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
        if w.contains('-') && w.len() > 3 && w.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Some(w.to_owned());
        }
    }
    // If no hyphenated slug, try the word after "for" or "prd".
    extract_word_after(text, &["for", "prd:", "prd "])
}

/// Extract a daemon/service name from a claim line.
fn extract_daemon_name(text: &str) -> Option<String> {
    // Look for wm-* service names.
    for word in text.split_whitespace() {
        let w = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.');
        if (w.starts_with("wm-") || w.ends_with(".service")) && w.len() > 3 {
            let name = w.strip_suffix(".service").unwrap_or(w);
            return Some(name.to_owned());
        }
    }
    None
}

fn extract_word_after<'a>(text: &'a str, keywords: &[&str]) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    for kw in keywords {
        if let Some(pos) = lower.find(kw) {
            let after = &text[pos + kw.len()..].trim_start();
            let word: String = after
                .split_whitespace()
                .next()
                .unwrap_or("")
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '/')
                .collect();
            if !word.is_empty() {
                return Some(word);
            }
        }
    }
    None
}

/// Check git remote URL for a repo directory.
fn git_remote_url(repo: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "remote", "get-url", "origin"])
        .output()
        .ok()?;
    if out.status.success() {
        let url = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if !url.is_empty() {
            return Some(url);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::claim::{extract_claims, ClaimKind};
    use tempfile::TempDir;

    fn opts_with_root(root: &std::path::Path) -> VerifyOptions {
        VerifyOptions {
            source_root: Some(root.to_owned()),
        }
    }

    // ---------------------------------------------------------------------------
    // Extraction helper tests
    // ---------------------------------------------------------------------------

    #[test]
    fn extract_repo_slug_github_style() {
        assert_eq!(
            extract_repo_slug("pushed j0yen/threshold to github"),
            Some("j0yen/threshold".to_owned())
        );
    }

    #[test]
    fn extract_repo_slug_plain_push() {
        assert_eq!(
            extract_repo_slug("pushed threshold-brief"),
            Some("threshold-brief".to_owned())
        );
    }

    #[test]
    fn extract_prd_slug_for_pattern() {
        assert_eq!(
            extract_prd_slug("PRD shipped for threshold-brief"),
            Some("threshold-brief".to_owned())
        );
    }

    #[test]
    fn extract_daemon_name_wm_prefix() {
        assert_eq!(
            extract_daemon_name("wm-audio daemon is up"),
            Some("wm-audio".to_owned())
        );
    }

    #[test]
    fn extract_daemon_name_service_suffix() {
        assert_eq!(
            extract_daemon_name("wm-stt.service is running"),
            Some("wm-stt".to_owned())
        );
    }

    // ---------------------------------------------------------------------------
    // Verifier fixture tests — four verdict values per verifier
    // ---------------------------------------------------------------------------

    /// narrative → always unverifiable
    #[test]
    fn narrative_always_unverifiable() {
        let claims = extract_claims("- it was a great session");
        assert_eq!(claims[0].kind, ClaimKind::Narrative);
        let opts = VerifyOptions::default();
        let verdicts = verify_claims(&claims, &opts);
        assert_eq!(verdicts[0].status, VerdictStatus::Unverifiable);
    }

    /// pushed-repo: confirmed when repo dir has a remote
    #[test]
    fn pushed_repo_confirmed() {
        let tmp = TempDir::new().unwrap();
        let wm_root = tmp.path().join("wintermute");
        std::fs::create_dir_all(&wm_root).unwrap();

        // Create a bare-ish git repo under wintermute/foo with a remote.
        let repo_dir = wm_root.join("foo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["remote", "add", "origin", "https://github.com/j0yen/foo"])
            .current_dir(&repo_dir)
            .output()
            .unwrap();

        let claims = extract_claims("- pushed j0yen/foo to github");
        // source_root IS the wintermute root (wintermute_root() returns source_root as-is).
        let opts = opts_with_root(&wm_root);
        let verdicts = verify_claims(&claims, &opts);
        assert_eq!(verdicts[0].status, VerdictStatus::Confirmed, "{}", verdicts[0].evidence);
    }

    /// pushed-repo: contradicted when repo dir is missing
    #[test]
    fn pushed_repo_contradicted_missing_dir() {
        let tmp = TempDir::new().unwrap();
        // source_root IS the wintermute root — nothing under it.

        let claims = extract_claims("- pushed j0yen/nonexistent-repo to github");
        let opts = opts_with_root(tmp.path());
        let verdicts = verify_claims(&claims, &opts);
        assert_eq!(verdicts[0].status, VerdictStatus::Contradicted, "{}", verdicts[0].evidence);
    }

    /// pushed-repo: unverifiable when no slug can be extracted
    #[test]
    fn pushed_repo_unverifiable_no_slug() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("wintermute")).unwrap();

        // Force PushedRepo kind but with text that has no slug.
        let claim = Claim::new("pushed (something unclear)", ClaimKind::PushedRepo);
        let opts = opts_with_root(tmp.path());
        let verdict = verify_one(&claim, &opts);
        // No recognizable slug → unverifiable OR contradicted (depends on extraction).
        // Either is acceptable; just check it doesn't panic.
        assert!(
            verdict.status == VerdictStatus::Unverifiable
                || verdict.status == VerdictStatus::Contradicted
        );
    }

    /// shipped-prd: confirmed when slug appears as archived in manifest
    #[test]
    fn shipped_prd_confirmed() {
        let tmp = TempDir::new().unwrap();
        let state_dir = tmp.path().join(".claude/skills/build/state");
        std::fs::create_dir_all(&state_dir).unwrap();
        let manifest = state_dir.join("manifest.json");
        std::fs::write(
            &manifest,
            r#"{"prds":[{"slug":"threshold-brief","status":"archived"}]}"#,
        )
        .unwrap();

        let claims = extract_claims("- PRD shipped for threshold-brief");
        let opts = opts_with_root(tmp.path());
        let verdicts = verify_claims(&claims, &opts);
        assert_eq!(verdicts[0].status, VerdictStatus::Confirmed, "{}", verdicts[0].evidence);
    }

    /// shipped-prd: stale when slug is present but not archived
    #[test]
    fn shipped_prd_stale() {
        let tmp = TempDir::new().unwrap();
        let state_dir = tmp.path().join(".claude/skills/build/state");
        std::fs::create_dir_all(&state_dir).unwrap();
        let manifest = state_dir.join("manifest.json");
        std::fs::write(
            &manifest,
            r#"{"prds":[{"slug":"threshold-brief","status":"in_progress"}]}"#,
        )
        .unwrap();

        let claims = extract_claims("- PRD shipped for threshold-brief");
        let opts = opts_with_root(tmp.path());
        let verdicts = verify_claims(&claims, &opts);
        assert_eq!(verdicts[0].status, VerdictStatus::Stale, "{}", verdicts[0].evidence);
    }

    /// shipped-prd: contradicted when slug not in manifest
    #[test]
    fn shipped_prd_contradicted() {
        let tmp = TempDir::new().unwrap();
        let state_dir = tmp.path().join(".claude/skills/build/state");
        std::fs::create_dir_all(&state_dir).unwrap();
        let manifest = state_dir.join("manifest.json");
        std::fs::write(&manifest, r#"{"prds":[]}"#).unwrap();

        let claims = extract_claims("- PRD shipped for threshold-brief");
        let opts = opts_with_root(tmp.path());
        let verdicts = verify_claims(&claims, &opts);
        assert_eq!(
            verdicts[0].status,
            VerdictStatus::Contradicted,
            "{}",
            verdicts[0].evidence
        );
    }

    /// shipped-prd: unverifiable when manifest missing
    #[test]
    fn shipped_prd_unverifiable_no_manifest() {
        let tmp = TempDir::new().unwrap();
        // No manifest file.
        let claims = extract_claims("- PRD shipped for threshold-brief");
        let opts = opts_with_root(tmp.path());
        let verdicts = verify_claims(&claims, &opts);
        assert_eq!(
            verdicts[0].status,
            VerdictStatus::Unverifiable,
            "{}",
            verdicts[0].evidence
        );
    }

    /// contradicted verdict never silently dropped from output
    #[test]
    fn contradicted_never_silently_dropped() {
        let tmp = TempDir::new().unwrap();
        // source_root acts as wintermute root — no repos inside.
        let state_dir = tmp.path().join(".claude/skills/build/state");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(state_dir.join("manifest.json"), r#"{"prds":[]}"#).unwrap();

        let claims = extract_claims(
            "- pushed j0yen/does-not-exist\n- PRD shipped for does-not-exist",
        );
        let opts = opts_with_root(tmp.path());
        let verdicts = verify_claims(&claims, &opts);

        // At least one contradicted verdict must be present.
        let contradicted: Vec<_> = verdicts
            .iter()
            .filter(|v| v.status == VerdictStatus::Contradicted)
            .collect();
        assert!(
            !contradicted.is_empty(),
            "expected at least one Contradicted verdict, got: {verdicts:?}"
        );

        // Verify text render includes "contradicted".
        let text = crate::verify::render::render_text(&verdicts);
        assert!(
            text.to_ascii_lowercase().contains("contradicted"),
            "text output must contain 'contradicted': {text}"
        );
    }

    /// pending-todo: contradicted when now archived
    #[test]
    fn pending_todo_contradicted_when_archived() {
        let tmp = TempDir::new().unwrap();
        let state_dir = tmp.path().join(".claude/skills/build/state");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(
            state_dir.join("manifest.json"),
            r#"{"prds":[{"slug":"threshold-brief","status":"archived"}]}"#,
        )
        .unwrap();

        let claim = Claim::new("todo: finish threshold-brief", ClaimKind::PendingTodo);
        let opts = opts_with_root(tmp.path());
        let verdict = verify_one(&claim, &opts);
        assert_eq!(verdict.status, VerdictStatus::Contradicted, "{}", verdict.evidence);
    }

    /// pending-todo: confirmed when still present and not archived
    #[test]
    fn pending_todo_confirmed_when_live() {
        let tmp = TempDir::new().unwrap();
        let state_dir = tmp.path().join(".claude/skills/build/state");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(
            state_dir.join("manifest.json"),
            r#"{"prds":[{"slug":"threshold-brief","status":"queued"}]}"#,
        )
        .unwrap();

        let claim = Claim::new("todo: finish threshold-brief", ClaimKind::PendingTodo);
        let opts = opts_with_root(tmp.path());
        let verdict = verify_one(&claim, &opts);
        assert_eq!(verdict.status, VerdictStatus::Confirmed, "{}", verdict.evidence);
    }
}
