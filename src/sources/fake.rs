//! `FakeSource` — deterministic in-memory source for testing.
//!
//! `FakeSource` implements [`SignalSource`] by returning a fixed
//! `Vec<Signal>` supplied at construction time. It is the primary tool
//! for AC2 and AC3 tests.

use crate::signal::{Signal, SignalSource};

/// A deterministic, in-memory [`SignalSource`] for use in tests.
pub struct FakeSource {
    name: String,
    signals: Vec<Signal>,
}

impl FakeSource {
    /// Create a `FakeSource` that will return the given signals.
    #[must_use]
    pub fn new(name: impl Into<String>, signals: Vec<Signal>) -> Self {
        Self {
            name: name.into(),
            signals,
        }
    }
}

impl SignalSource for FakeSource {
    fn collect(&self) -> Result<Vec<Signal>, anyhow::Error> {
        Ok(self.signals.clone())
    }

    fn name(&self) -> &str {
        &self.name
    }
}
