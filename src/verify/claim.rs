//! Claim extraction from a reflective letter.
//!
//! [`extract_claims`] parses raw letter text into a [`Vec<Claim>`] by splitting
//! on bullet/numbered list lines and classifying each by keyword heuristics.

use serde::{Deserialize, Serialize};

/// The semantic category of a claim extracted from a reflective letter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    /// A repo was pushed to remote (keyword: pushed, push, published).
    PushedRepo,
    /// A PRD was shipped/completed (keyword: shipped, completed, archived prd, finished prd).
    ShippedPrd,
    /// A daemon is running (keyword: running, started, daemon, service, live).
    DaemonUp,
    /// A peer is present on the agorabus (keyword: peer, connected, agorabus).
    PeerPresent,
    /// An agent is in-flight (keyword: in-flight, inflight, delegated, dispatched).
    InFlightAgent,
    /// A TODO / pending task (keyword: todo, pending, next, need to, should).
    PendingTodo,
    /// A narrative / non-checkable statement. Always `unverifiable`.
    Narrative,
}

/// A single claim extracted from the letter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    /// Raw text of the claim (trimmed, stripped of bullet markers).
    pub text: String,
    /// Semantic category.
    pub kind: ClaimKind,
}

impl Claim {
    /// Create a new claim.
    #[must_use]
    pub fn new(text: impl Into<String>, kind: ClaimKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }
}

/// Parse letter text into discrete claims.
///
/// Heuristic v1: split on bullet/numbered list lines, classify each line by
/// keyword. Lines that match no checkable keyword are classified `Narrative`.
///
/// # Examples
///
/// ```
/// use threshold::verify::claim::{extract_claims, ClaimKind};
/// let text = "- pushed j0yen/foo to github\n- todo: write tests\n- it was a good session";
/// let claims = extract_claims(text);
/// assert_eq!(claims[0].kind, ClaimKind::PushedRepo);
/// assert_eq!(claims[1].kind, ClaimKind::PendingTodo);
/// assert_eq!(claims[2].kind, ClaimKind::Narrative);
/// ```
#[must_use]
pub fn extract_claims(text: &str) -> Vec<Claim> {
    let mut claims = Vec::new();

    for raw_line in text.lines() {
        let stripped = strip_bullet(raw_line.trim());
        if stripped.is_empty() {
            continue;
        }
        let kind = classify(stripped);
        claims.push(Claim::new(stripped.to_owned(), kind));
    }

    claims
}

/// Remove common bullet / list markers from the start of a line.
fn strip_bullet(s: &str) -> &str {
    // Handle numbered list: "1. ", "2) ", etc.
    let after_num = {
        let mut chars = s.chars();
        let first = chars.next();
        let second = chars.next();
        let third = chars.next();
        match (first, second, third) {
            (Some(c), Some('.' | ')'), Some(' ')) if c.is_ascii_digit() => &s[3..],
            _ => s,
        }
    };

    // Bullet markers
    let trimmed = after_num
        .strip_prefix("- ")
        .or_else(|| after_num.strip_prefix("* "))
        .or_else(|| after_num.strip_prefix("+ "))
        .or_else(|| after_num.strip_prefix("• "))
        .unwrap_or(after_num);

    trimmed.trim()
}

/// Classify a stripped claim line into a [`ClaimKind`].
fn classify(line: &str) -> ClaimKind {
    let lower = line.to_ascii_lowercase();

    // Prefer more specific patterns first.

    if contains_any(&lower, &["pushed", "push to", "published to github", "gh repo create"]) {
        return ClaimKind::PushedRepo;
    }

    if contains_any(
        &lower,
        &[
            "shipped prd",
            "completed prd",
            "archived prd",
            "finished prd",
            "prd shipped",
            "prd completed",
            "prd archived",
            "prd finished",
        ],
    ) {
        return ClaimKind::ShippedPrd;
    }

    if contains_any(
        &lower,
        &["daemon running", "service running", "daemon started", "service started", "daemon is up", "is live", "is running"],
    ) {
        return ClaimKind::DaemonUp;
    }

    if contains_any(&lower, &["agorabus peer", "peer connected", "peer is present", "peer present"]) {
        return ClaimKind::PeerPresent;
    }

    if contains_any(
        &lower,
        &["in-flight", "inflight", "delegated", "dispatched", "agent running", "agent in progress"],
    ) {
        return ClaimKind::InFlightAgent;
    }

    if contains_any(
        &lower,
        &[
            "todo:", "todo ",
            "pending:", "pending ",
            "next step",
            "next: ",
            "need to ",
            "needs to ",
            "should ",
            "still need",
            "not yet",
            "haven't",
            "have not",
        ],
    ) {
        return ClaimKind::PendingTodo;
    }

    ClaimKind::Narrative
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bullet_stripped_correctly() {
        assert_eq!(strip_bullet("- foo"), "foo");
        assert_eq!(strip_bullet("* foo"), "foo");
        assert_eq!(strip_bullet("+ foo"), "foo");
        assert_eq!(strip_bullet("• foo"), "foo");
        assert_eq!(strip_bullet("1. foo"), "foo");
        assert_eq!(strip_bullet("no bullet"), "no bullet");
    }

    #[test]
    fn pushed_repo_classified() {
        let text = "- pushed j0yen/threshold to github";
        let claims = extract_claims(text);
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].kind, ClaimKind::PushedRepo);
    }

    #[test]
    fn shipped_prd_classified() {
        let text = "- PRD shipped for threshold-brief\n- prd archived: homeward-api";
        let claims = extract_claims(text);
        assert_eq!(claims[0].kind, ClaimKind::ShippedPrd);
        assert_eq!(claims[1].kind, ClaimKind::ShippedPrd);
    }

    #[test]
    fn daemon_up_classified() {
        let text = "- wm-audio daemon is up";
        let claims = extract_claims(text);
        assert_eq!(claims[0].kind, ClaimKind::DaemonUp);
    }

    #[test]
    fn pending_todo_classified() {
        let text = "- todo: write more tests\n- should review the docs\n- still need to push";
        let claims = extract_claims(text);
        assert!(claims.iter().all(|c| c.kind == ClaimKind::PendingTodo));
    }

    #[test]
    fn narrative_classified() {
        let text = "- it was a productive session\n- the work felt good";
        let claims = extract_claims(text);
        assert!(claims.iter().all(|c| c.kind == ClaimKind::Narrative));
    }

    #[test]
    fn mixed_letter_all_kinds() {
        let text = "- pushed j0yen/foo\n\
                    - PRD shipped for foo-bar\n\
                    - todo: update docs\n\
                    - it was a good day";
        let claims = extract_claims(text);
        assert_eq!(claims.len(), 4);
        assert_eq!(claims[0].kind, ClaimKind::PushedRepo);
        assert_eq!(claims[1].kind, ClaimKind::ShippedPrd);
        assert_eq!(claims[2].kind, ClaimKind::PendingTodo);
        assert_eq!(claims[3].kind, ClaimKind::Narrative);
    }

    #[test]
    fn empty_lines_skipped() {
        let text = "\n\n- real claim\n\n";
        let claims = extract_claims(text);
        assert_eq!(claims.len(), 1);
    }
}
