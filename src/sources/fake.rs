//! `FakeSource` and `SlowSource` — deterministic in-memory sources for testing.
//!
//! `FakeSource` implements [`SignalSource`] by returning a fixed
//! `Vec<Signal>` supplied at construction time. It is the primary tool
//! for AC2 and AC3 tests.
//!
//! `SlowSource` blocks for a configurable duration before returning signals,
//! enabling tests to verify the time-bound behaviour of `collect_with_deadline`.

use std::time::Duration;

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

/// A [`SignalSource`] that sleeps for a fixed duration before returning.
///
/// Use this in tests to verify that `collect_with_deadline` returns partial
/// results when a source is pathologically slow.
pub struct SlowSource {
    name: String,
    delay: Duration,
    signals: Vec<Signal>,
}

impl SlowSource {
    /// Create a `SlowSource` that will sleep for `delay` before returning `signals`.
    #[must_use]
    pub fn new(name: impl Into<String>, delay: Duration, signals: Vec<Signal>) -> Self {
        Self {
            name: name.into(),
            delay,
            signals,
        }
    }
}

impl SignalSource for SlowSource {
    fn collect(&self) -> Result<Vec<Signal>, anyhow::Error> {
        std::thread::sleep(self.delay);
        Ok(self.signals.clone())
    }

    fn name(&self) -> &str {
        &self.name
    }
}
