//! Session identity resolution for the threshold ledger.
//!
//! Attempts to read the agent-session ID from `/proc` (agentns surface) when
//! available and non-zero. Falls back to `hostname:pid:short-uuid` when the
//! kernel surface is absent or reports all-zeros.
//!
//! The `IdSource` trait makes both branches unit-testable without a live agentns.

use std::fmt;

/// A resolved session identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionId(String);

impl SessionId {
    /// Construct a session ID from a raw string (used by tests / mock sources).
    #[must_use]
    pub fn from_raw(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A source of agent-session identity.
///
/// This trait exists so tests can inject either the real kernel path or
/// a mock without touching `/proc`.
pub trait IdSource: Send + Sync {
    /// Try to return a non-zero session ID from the agentns surface.
    /// Returns `None` when the surface is absent, unreadable, or all-zeros.
    fn agentns_id(&self) -> Option<String>;

    /// Return a fallback ID string (hostname:pid:short-token).
    fn fallback_id(&self) -> String;
}

/// Resolves the session ID using the provided `IdSource`.
///
/// Prefers the agentns id when non-`None`; falls back to `fallback_id()`.
#[must_use]
pub fn resolve(source: &dyn IdSource) -> SessionId {
    let raw = source.agentns_id().unwrap_or_else(|| source.fallback_id());
    SessionId(raw)
}

// ─── Real kernel source ───────────────────────────────────────────────────────

/// Reads the agentns session ID from `/proc/self/` and derives a
/// hostname+pid fallback when the kernel surface is absent.
#[derive(Debug, Default)]
pub struct RealIdSource;

impl IdSource for RealIdSource {
    fn agentns_id(&self) -> Option<String> {
        // Probe known agentns proc paths for a session id.
        // Current kernel surface (agentns, pkgrel ≥ 12) exposes one of:
        //   /proc/self/agent_session_id   (future)
        //   /proc/self/agent_ns/id        (earlier sketch)
        // Both may be absent; we try them in order.
        let candidates = [
            "/proc/self/agent_session_id",
            "/proc/self/agent_ns/id",
        ];
        for path in candidates {
            if let Ok(raw) = std::fs::read_to_string(path) {
                let trimmed = raw.trim();
                // Reject all-zeros or empty
                if !trimmed.is_empty()
                    && trimmed != "0"
                    && !trimmed.chars().all(|c| c == '0' || c == '-')
                {
                    return Some(trimmed.to_owned());
                }
            }
        }
        None
    }

    fn fallback_id(&self) -> String {
        let hostname = hostname_string();
        let pid = std::process::id();
        // Four random hex bytes as a short disambiguator
        let token = short_token();
        format!("{hostname}:{pid}:{token}")
    }
}

fn hostname_string() -> String {
    // Read from /proc/sys/kernel/hostname for portability
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|_| "unknown".to_owned())
}

/// Generate a short 8-hex-char token from the current time's nanoseconds.
/// Not cryptographic; good enough for session disambiguation.
fn short_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{ns:08x}")
}

// ─── Mock source for tests ────────────────────────────────────────────────────

/// A mock `IdSource` for unit tests.
///
/// Injects a specific agentns id or forces the fallback path.
#[derive(Debug)]
pub struct MockIdSource {
    agentns: Option<String>,
    fallback: String,
}

impl MockIdSource {
    /// Create a mock that returns the given agentns id (non-None → agentns path).
    #[must_use]
    pub fn with_agentns(id: impl Into<String>) -> Self {
        Self {
            agentns: Some(id.into()),
            fallback: "fallback:0:00000000".to_owned(),
        }
    }

    /// Create a mock that simulates absent/zero agentns (forces fallback path).
    #[must_use]
    pub fn fallback_only(fallback: impl Into<String>) -> Self {
        Self {
            agentns: None,
            fallback: fallback.into(),
        }
    }
}

impl IdSource for MockIdSource {
    fn agentns_id(&self) -> Option<String> {
        self.agentns.clone()
    }
    fn fallback_id(&self) -> String {
        self.fallback.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_prefers_agentns_id() {
        let src = MockIdSource::with_agentns("test-session-abc123");
        let id = resolve(&src);
        assert_eq!(id.to_string(), "test-session-abc123");
    }

    #[test]
    fn resolve_falls_back_when_agentns_absent() {
        let src = MockIdSource::fallback_only("myhost:1234:deadbeef");
        let id = resolve(&src);
        assert_eq!(id.to_string(), "myhost:1234:deadbeef");
    }

    #[test]
    fn session_id_display() {
        let id = SessionId::from_raw("some-id");
        assert_eq!(format!("{id}"), "some-id");
    }

    #[test]
    fn real_id_source_fallback_not_empty() {
        let src = RealIdSource;
        // On a normal Linux box, /proc/self/agent_session_id won't exist,
        // so we'll hit the fallback.  Just verify it's non-empty.
        let id = resolve(&src);
        assert!(!id.to_string().is_empty(), "session id must not be empty");
    }
}
