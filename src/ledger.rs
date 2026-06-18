//! Append-only question ledger for the threshold session hand-off channel.
//!
//! ## Store format
//!
//! The ledger lives at `$XDG_STATE_HOME/threshold/ledger.jsonl` (one JSON
//! object per line, never rewritten).  There are two record types:
//!
//! **Question record** (appended by `threshold ask`):
//! ```json
//! {"id":"<uuid-like>","ts":"<ISO8601>","asked_by_session":"<sid>","question":"...","tags":["a","b"]}
//! ```
//!
//! **Answer record** (appended by `threshold answer`):
//! ```json
//! {"id":"<same-id>","ts":"<ISO8601>","answered_by_session":"<sid>","answer":"..."}
//! ```
//!
//! A question is *open* when no answer record with the same `id` exists.
//!
//! ## JSON schema
//!
//! The open-question schema is documented in [`OpenQuestion`].

use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

// ─── Record types ─────────────────────────────────────────────────────────────

/// A raw record deserialized from a JSONL line. Only the fields used by each
/// variant are populated; unknown fields are ignored.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LedgerRecord {
    /// Stable ID shared between question and answer.
    pub id: String,
    /// ISO 8601 timestamp of this record.
    pub ts: String,
    // Question-only fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asked_by_session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    // Answer-only fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answered_by_session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
}

/// An open (unanswered) question — the schema for `threshold open --format json`.
///
/// ## JSON Schema (threshold.ledger.open.v1)
///
/// ```json
/// {
///   "schema": "threshold.ledger.open.v1",
///   "open_questions": [
///     {
///       "id": "<string>",
///       "ts": "<ISO8601>",
///       "asked_by_session": "<string>",
///       "question": "<string>",
///       "tags": ["<string>", ...]
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenQuestion {
    /// Stable question ID.
    pub id: String,
    /// Timestamp the question was asked (ISO 8601).
    pub ts: String,
    /// Session that asked this question.
    pub asked_by_session: String,
    /// The question text.
    pub question: String,
    /// Optional tags.
    pub tags: Vec<String>,
}

/// The JSON output of `threshold open --format json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenQuestionsOutput {
    /// Schema version.
    pub schema: String,
    /// Open (unanswered) questions, newest first.
    pub open_questions: Vec<OpenQuestion>,
}

// ─── Ledger path ──────────────────────────────────────────────────────────────

/// Resolve the ledger path.
///
/// `override_path` is used in tests to point at a temp file.
/// Otherwise: `$XDG_STATE_HOME/threshold/ledger.jsonl`, falling back to
/// `~/.local/state/threshold/ledger.jsonl`.
#[must_use]
pub fn ledger_path(override_path: Option<&Path>) -> PathBuf {
    if let Some(p) = override_path {
        return p.to_owned();
    }
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            dirs_from_env().map(|home| home.join(".local/state"))
        })
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    base.join("threshold/ledger.jsonl")
}

fn dirs_from_env() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

// ─── Core operations ──────────────────────────────────────────────────────────

/// Append a question record to the ledger.
///
/// Returns the new question's `id`.
///
/// # Errors
///
/// Returns an error if the ledger file or its parent directory cannot be
/// created or written to.
pub fn ask(
    path: &Path,
    session_id: &str,
    question: &str,
    tags: Vec<String>,
) -> Result<String> {
    let id = new_id();
    let ts = Utc::now().to_rfc3339();
    let record = LedgerRecord {
        id: id.clone(),
        ts,
        asked_by_session: Some(session_id.to_owned()),
        question: Some(question.to_owned()),
        tags,
        answered_by_session: None,
        answer: None,
    };
    append_record(path, &record)?;
    Ok(id)
}

/// Append an answer record to the ledger for `question_id`.
///
/// # Errors
///
/// Returns `Err` with a distinct message if:
/// - `question_id` is not found among question records
/// - `question_id` is already answered
/// - file I/O fails
pub fn answer(
    path: &Path,
    session_id: &str,
    question_id: &str,
    answer_text: &str,
) -> Result<()> {
    let records = read_all(path)?;

    // Find the question record
    let question_exists = records.iter().any(|r| {
        r.id == question_id && r.question.is_some()
    });
    if !question_exists {
        bail!("question id '{}' not found in ledger", question_id);
    }

    // Check not already answered
    let already_answered = records.iter().any(|r| {
        r.id == question_id && r.answer.is_some()
    });
    if already_answered {
        bail!(
            "question id '{}' has already been answered — use 'threshold open' to see unanswered questions",
            question_id
        );
    }

    let ts = Utc::now().to_rfc3339();
    let record = LedgerRecord {
        id: question_id.to_owned(),
        ts,
        asked_by_session: None,
        question: None,
        tags: vec![],
        answered_by_session: Some(session_id.to_owned()),
        answer: Some(answer_text.to_owned()),
    };
    append_record(path, &record)
}

/// Return all open (unanswered) questions, newest first.
///
/// # Errors
///
/// Returns an error if the ledger file exists but cannot be read or parsed.
/// Returns `Ok([])` if the file is absent.
pub fn open_questions(path: &Path) -> Result<Vec<OpenQuestion>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let records = read_all(path)?;

    // Collect answered IDs
    let answered: HashSet<&str> = records
        .iter()
        .filter(|r| r.answer.is_some())
        .map(|r| r.id.as_str())
        .collect();

    let mut questions: Vec<OpenQuestion> = records
        .iter()
        .filter(|r| r.question.is_some() && !answered.contains(r.id.as_str()))
        .map(|r| OpenQuestion {
            id: r.id.clone(),
            ts: r.ts.clone(),
            asked_by_session: r.asked_by_session.clone().unwrap_or_default(),
            question: r.question.clone().unwrap_or_default(),
            tags: r.tags.clone(),
        })
        .collect();

    // Newest first (reverse chronological by ts string — ISO 8601 sorts lexicographically)
    questions.sort_by(|a, b| b.ts.cmp(&a.ts));
    Ok(questions)
}

// ─── Append-only guarantee ────────────────────────────────────────────────────

/// Read all records from the ledger. Returns `Ok([])` if file is absent.
fn read_all(path: &Path) -> Result<Vec<LedgerRecord>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let f = std::fs::File::open(path)
        .with_context(|| format!("cannot open ledger at {}", path.display()))?;
    let reader = std::io::BufReader::new(f);
    let mut records = Vec::new();
    for (lineno, line) in reader.lines().enumerate() {
        let line = line
            .with_context(|| format!("ledger read error at line {lineno}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let record: LedgerRecord = serde_json::from_str(&line)
            .with_context(|| format!("ledger parse error at line {lineno}: {line}"))?;
        records.push(record);
    }
    Ok(records)
}

/// Append a single record to the ledger file (creates parent dirs and file if needed).
fn append_record(path: &Path, record: &LedgerRecord) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create ledger directory {}", parent.display()))?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("cannot open ledger for append at {}", path.display()))?;
    let line = serde_json::to_string(record).context("cannot serialize ledger record")?;
    writeln!(f, "{line}").with_context(|| format!("cannot write to ledger at {}", path.display()))?;
    Ok(())
}

/// Generate a short unique ID.
///
/// Uses timestamp + process id + a nonce counter to avoid collisions even
/// when multiple records are written in the same nanosecond.
fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pid = std::process::id();
    format!("{ts:016x}-{pid:08x}-{n:04x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn temp_ledger() -> (NamedTempFile, PathBuf) {
        let f = NamedTempFile::new().expect("temp file");
        let p = f.path().to_owned();
        // Remove the file so the ledger starts empty (append will recreate)
        std::fs::remove_file(&p).ok();
        (f, p)
    }

    #[test]
    fn ask_then_open_roundtrip() {
        let (_f, path) = temp_ledger();
        let id = ask(&path, "session-A", "What should I do next?", vec![]).unwrap();
        let open = open_questions(&path).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, id);
        assert_eq!(open[0].question, "What should I do next?");
    }

    #[test]
    fn answer_removes_from_open() {
        let (_f, path) = temp_ledger();
        let id = ask(&path, "session-A", "Question one?", vec![]).unwrap();
        let open_before = open_questions(&path).unwrap();
        assert_eq!(open_before.len(), 1);

        answer(&path, "session-B", &id, "My answer").unwrap();
        let open_after = open_questions(&path).unwrap();
        assert_eq!(open_after.len(), 0);
    }

    #[test]
    fn append_only_original_record_unchanged() {
        let (_f, path) = temp_ledger();
        let id = ask(&path, "s1", "Is this append-only?", vec!["test".to_owned()]).unwrap();

        // Read raw bytes before answering
        let before = std::fs::read_to_string(&path).unwrap();
        let first_line = before.lines().next().unwrap().to_owned();

        answer(&path, "s2", &id, "Yes, absolutely.").unwrap();

        // Read raw bytes after answering
        let after = std::fs::read_to_string(&path).unwrap();
        let first_line_after = after.lines().next().unwrap().to_owned();

        // The original question record must be byte-for-byte unchanged
        assert_eq!(
            first_line, first_line_after,
            "answering must not modify the original question record"
        );

        // There must now be two lines
        assert_eq!(after.lines().count(), 2, "answer appends a new record");
    }

    #[test]
    fn answer_unknown_id_fails_with_distinct_message() {
        let (_f, path) = temp_ledger();
        let err = answer(&path, "s1", "nonexistent-id", "oops")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("not found"),
            "expected 'not found' in error, got: {err}"
        );
    }

    #[test]
    fn answer_already_answered_fails_with_distinct_message() {
        let (_f, path) = temp_ledger();
        let id = ask(&path, "s1", "Q?", vec![]).unwrap();
        answer(&path, "s2", &id, "First answer").unwrap();

        let err = answer(&path, "s3", &id, "Second answer")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("already been answered"),
            "expected 'already been answered' in error, got: {err}"
        );
    }

    #[test]
    fn empty_ledger_returns_empty_open_questions() {
        let (_f, path) = temp_ledger();
        let open = open_questions(&path).unwrap();
        assert!(open.is_empty());
    }

    #[test]
    fn absent_ledger_returns_empty_open_questions() {
        let path = PathBuf::from("/tmp/threshold-ledger-test-absent-xyzzy-99999.jsonl");
        std::fs::remove_file(&path).ok(); // ensure absent
        let open = open_questions(&path).unwrap();
        assert!(open.is_empty());
    }

    #[test]
    fn open_format_json_stable_schema() {
        let (_f, path) = temp_ledger();
        ask(&path, "s1", "First Q?", vec!["a".to_owned()]).unwrap();
        ask(&path, "s2", "Second Q?", vec![]).unwrap();

        let open = open_questions(&path).unwrap();
        let output = OpenQuestionsOutput {
            schema: "threshold.ledger.open.v1".to_owned(),
            open_questions: open,
        };
        let json_str = serde_json::to_string(&output).unwrap();
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(json["schema"], "threshold.ledger.open.v1");
        let qs = json["open_questions"].as_array().unwrap();
        assert_eq!(qs.len(), 2);
        for q in qs {
            assert!(q.get("id").is_some(), "id field missing");
            assert!(q.get("ts").is_some(), "ts field missing");
            assert!(q.get("asked_by_session").is_some(), "asked_by_session missing");
            assert!(q.get("question").is_some(), "question field missing");
            assert!(q.get("tags").is_some(), "tags field missing");
        }
    }

    #[test]
    fn open_questions_newest_first() {
        let (_f, path) = temp_ledger();
        // Insert with a small sleep to ensure different timestamps
        ask(&path, "s1", "Older question", vec![]).unwrap();
        // Ensure different ms timestamp by busy-spinning briefly
        let start = std::time::Instant::now();
        while start.elapsed().as_millis() < 2 {}
        ask(&path, "s2", "Newer question", vec![]).unwrap();

        let open = open_questions(&path).unwrap();
        assert_eq!(open.len(), 2);
        // Newer question should come first (newest-first sort)
        assert_eq!(open[0].question, "Newer question");
        assert_eq!(open[1].question, "Older question");
    }

    #[test]
    fn multiple_open_one_answered() {
        let (_f, path) = temp_ledger();
        let id1 = ask(&path, "s1", "Q1?", vec![]).unwrap();
        let _id2 = ask(&path, "s1", "Q2?", vec![]).unwrap();
        let _id3 = ask(&path, "s1", "Q3?", vec![]).unwrap();

        answer(&path, "s2", &id1, "Answer to Q1").unwrap();

        let open = open_questions(&path).unwrap();
        assert_eq!(open.len(), 2, "Q2 and Q3 should remain open after answering Q1");
        assert!(open.iter().all(|q| q.id != id1), "Q1 must not appear in open list");
    }
}
