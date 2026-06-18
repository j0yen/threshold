//! Verdict types for claim verification.

use serde::{Deserialize, Serialize};

use crate::verify::claim::{Claim, ClaimKind};

/// The result of verifying a single claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictStatus {
    /// Ground truth confirms the claim.
    Confirmed,
    /// The claim was once true but is now superseded.
    Stale,
    /// Ground truth contradicts the claim.
    Contradicted,
    /// The claim cannot be verified (narrative, unknown kind, or missing tooling).
    Unverifiable,
}

/// The verification verdict for a single claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimVerdict {
    /// Raw claim text.
    pub claim: String,
    /// Semantic kind of the claim.
    pub kind: ClaimKind,
    /// Verification result.
    pub status: VerdictStatus,
    /// Human-readable citation of what was checked.
    pub evidence: String,
}

impl ClaimVerdict {
    /// Construct a verdict from a [`Claim`].
    #[must_use]
    pub fn new(claim: &Claim, status: VerdictStatus, evidence: impl Into<String>) -> Self {
        Self {
            claim: claim.text.clone(),
            kind: claim.kind.clone(),
            status,
            evidence: evidence.into(),
        }
    }

    /// Returns `true` if this verdict is `contradicted`.
    #[must_use]
    pub fn is_contradicted(&self) -> bool {
        self.status == VerdictStatus::Contradicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contradicted_flag_works() {
        let claim = Claim::new("pushed j0yen/foo", ClaimKind::PushedRepo);
        let v = ClaimVerdict::new(&claim, VerdictStatus::Contradicted, "no such remote branch");
        assert!(v.is_contradicted());
    }

    #[test]
    fn confirmed_is_not_contradicted() {
        let claim = Claim::new("pushed j0yen/foo", ClaimKind::PushedRepo);
        let v = ClaimVerdict::new(&claim, VerdictStatus::Confirmed, "git log shows push");
        assert!(!v.is_contradicted());
    }
}
