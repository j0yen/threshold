//! Text rendering for verify verdicts.

use crate::verify::verdict::{ClaimVerdict, VerdictStatus};

/// Render verdicts as human-readable text.
#[must_use]
pub fn render_text(verdicts: &[ClaimVerdict]) -> String {
    if verdicts.is_empty() {
        return "=== Letter Verification ===\n(no claims extracted)\n".to_owned();
    }

    let mut out = String::with_capacity(1024);
    out.push_str("=== Letter Verification ===\n\n");

    let confirmed: Vec<_> = verdicts
        .iter()
        .filter(|v| v.status == VerdictStatus::Confirmed)
        .collect();
    let contradicted: Vec<_> = verdicts
        .iter()
        .filter(|v| v.status == VerdictStatus::Contradicted)
        .collect();
    let stale: Vec<_> = verdicts
        .iter()
        .filter(|v| v.status == VerdictStatus::Stale)
        .collect();
    let unverifiable: Vec<_> = verdicts
        .iter()
        .filter(|v| v.status == VerdictStatus::Unverifiable)
        .collect();

    if !confirmed.is_empty() {
        out.push_str("--- Confirmed ---\n");
        for v in &confirmed {
            render_verdict(&mut out, v);
        }
        out.push('\n');
    }

    if !contradicted.is_empty() {
        out.push_str("--- Contradicted (false claims — act with caution) ---\n");
        for v in &contradicted {
            render_verdict(&mut out, v);
        }
        out.push('\n');
    }

    if !stale.is_empty() {
        out.push_str("--- Stale (superseded) ---\n");
        for v in &stale {
            render_verdict(&mut out, v);
        }
        out.push('\n');
    }

    if !unverifiable.is_empty() {
        out.push_str("--- Unverifiable (narrative or missing tooling) ---\n");
        for v in &unverifiable {
            render_verdict(&mut out, v);
        }
        out.push('\n');
    }

    let total = verdicts.len();
    let con = confirmed.len();
    let ctr = contradicted.len();
    let stl = stale.len();
    let unv = unverifiable.len();
    out.push_str(&format!(
        "Summary: {total} claims — {con} confirmed, {ctr} contradicted, {stl} stale, {unv} unverifiable\n"
    ));

    out
}

fn render_verdict(out: &mut String, v: &ClaimVerdict) {
    let badge = match v.status {
        VerdictStatus::Confirmed => "✓",
        VerdictStatus::Contradicted => "✗",
        VerdictStatus::Stale => "~",
        VerdictStatus::Unverifiable => "?",
    };
    out.push_str(&format!("  {badge} {}\n", v.claim));
    out.push_str(&format!("    evidence: {}\n", v.evidence));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::claim::{Claim, ClaimKind};
    use crate::verify::verdict::{ClaimVerdict, VerdictStatus};

    #[test]
    fn empty_verdicts_renders_gracefully() {
        let text = render_text(&[]);
        assert!(text.contains("no claims"));
    }

    #[test]
    fn all_four_statuses_render() {
        let claim = Claim::new("foo", ClaimKind::Narrative);
        let verdicts = vec![
            ClaimVerdict::new(&claim, VerdictStatus::Confirmed, "ev1"),
            ClaimVerdict::new(&claim, VerdictStatus::Contradicted, "ev2"),
            ClaimVerdict::new(&claim, VerdictStatus::Stale, "ev3"),
            ClaimVerdict::new(&claim, VerdictStatus::Unverifiable, "ev4"),
        ];
        let text = render_text(&verdicts);
        assert!(text.contains("Confirmed"));
        assert!(text.contains("Contradicted"));
        assert!(text.contains("Stale"));
        assert!(text.contains("Unverifiable"));
        assert!(text.contains("Summary: 4 claims"));
    }

    #[test]
    fn contradicted_in_summary_line() {
        let claim = Claim::new("foo", ClaimKind::PushedRepo);
        let verdicts = vec![ClaimVerdict::new(
            &claim,
            VerdictStatus::Contradicted,
            "no such repo",
        )];
        let text = render_text(&verdicts);
        // Must appear in the "Contradicted" section header.
        assert!(text.contains("Contradicted"), "text={text}");
    }
}
