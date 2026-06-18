//! [`LedgerSource`] — surfaces open questions from the threshold ledger
//! as `SignalKind::Owed` signals in the arrival briefing.

use std::path::{Path, PathBuf};

use crate::ledger;
use crate::signal::{Signal, SignalKind, SignalSource};

/// Reads the threshold ledger and emits open (unanswered) questions as
/// `Owed` signals so they appear in the *Owed to you* section of `threshold brief`.
///
/// Degrades to `Ok(vec![])` when the ledger is absent or empty.
pub struct LedgerSource {
    ledger_path: PathBuf,
}

impl LedgerSource {
    /// Create a new `LedgerSource`.
    ///
    /// When `source_root` is `Some`, the ledger is loaded from
    /// `<source_root>/threshold/ledger.jsonl` (testing seam).
    /// Otherwise uses the real XDG state path.
    #[must_use]
    pub fn new(source_root: Option<&Path>) -> Self {
        let path = if let Some(root) = source_root {
            root.join("threshold/ledger.jsonl")
        } else {
            ledger::ledger_path(None)
        };
        Self { ledger_path: path }
    }
}

impl SignalSource for LedgerSource {
    fn name(&self) -> &str {
        "ledger"
    }

    fn collect(&self) -> Result<Vec<Signal>, anyhow::Error> {
        let questions = match ledger::open_questions(&self.ledger_path) {
            Ok(qs) => qs,
            // Degraded: malformed ledger → empty contribution
            Err(_) => return Ok(vec![]),
        };

        // Cap at 5 most-recent open questions to avoid flooding the briefing
        const MAX_LEDGER_ITEMS: usize = 5;
        let questions: Vec<_> = questions.into_iter().take(MAX_LEDGER_ITEMS).collect();

        let signals = questions
            .into_iter()
            .enumerate()
            .map(|(i, q)| {
                // Priority: first question gets 70, decreasing by 5 per slot
                let priority = 70u8.saturating_sub((i * 5) as u8);
                let title = format!("Open question: {}", q.question);
                let body = format!(
                    "Asked by: {}\nID: {}\nTags: {}",
                    q.asked_by_session,
                    q.id,
                    if q.tags.is_empty() {
                        "(none)".to_owned()
                    } else {
                        q.tags.join(", ")
                    }
                );
                Signal::new(SignalKind::Owed, title, body, priority, "ledger")
            })
            .collect();

        Ok(signals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger;
    use tempfile::TempDir;

    fn fixture_dir_with_questions(n: usize) -> TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let ledger_dir = dir.path().join("threshold");
        std::fs::create_dir_all(&ledger_dir).unwrap();
        let path = ledger_dir.join("ledger.jsonl");
        for i in 0..n {
            ledger::ask(&path, "test-session", &format!("Question {i}?"), vec![]).unwrap();
        }
        dir
    }

    #[test]
    fn ledger_source_emits_owed_signals() {
        let dir = fixture_dir_with_questions(3);
        let src = LedgerSource::new(Some(dir.path()));
        let signals = src.collect().unwrap();
        assert_eq!(signals.len(), 3);
        assert!(signals.iter().all(|s| s.kind == SignalKind::Owed));
        assert!(signals.iter().all(|s| s.source == "ledger"));
    }

    #[test]
    fn ledger_source_degrades_on_absent_ledger() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = LedgerSource::new(Some(dir.path()));
        let signals = src.collect().unwrap();
        assert!(signals.is_empty(), "absent ledger must degrade to empty");
    }

    #[test]
    fn ledger_source_caps_at_five_items() {
        let dir = fixture_dir_with_questions(10);
        let src = LedgerSource::new(Some(dir.path()));
        let signals = src.collect().unwrap();
        assert_eq!(signals.len(), 5, "ledger source must cap at 5 items");
    }

    #[test]
    fn answered_questions_not_emitted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ledger_dir = dir.path().join("threshold");
        std::fs::create_dir_all(&ledger_dir).unwrap();
        let path = ledger_dir.join("ledger.jsonl");

        let id = ledger::ask(&path, "s1", "Q to answer?", vec![]).unwrap();
        ledger::ask(&path, "s1", "Q to keep open?", vec![]).unwrap();
        ledger::answer(&path, "s2", &id, "Answered!").unwrap();

        let src = LedgerSource::new(Some(dir.path()));
        let signals = src.collect().unwrap();
        assert_eq!(signals.len(), 1, "answered questions must not be emitted");
        assert!(signals[0].title.contains("keep open"), "only open question should be emitted");
    }
}
