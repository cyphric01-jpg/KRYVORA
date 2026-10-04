//! Explicit state enums.
//!
//! Wherever a value can be unknown, inconclusive, or partial, we model
//! that explicitly. Booleans are not used for state that has more than
//! two honest outcomes.

use serde::{Deserialize, Serialize};

/// Lifecycle of a long-running job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Created,
    Queued,
    Running,
    Verifying,
    Succeeded,
    Failed,
    Partial,
    Cancelled,
    Inconclusive,
}

/// Lifecycle of a persisted destructive sanitization operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SanitizationOperationState {
    Planned,
    Running,
    Completed,
    Failed,
    Refused,
}

impl SanitizationOperationState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Refused => "refused",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "planned" => Some(Self::Planned),
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "refused" => Some(Self::Refused),
            _ => None,
        }
    }
}

impl JobState {
    /// Is this a terminal state (no further transitions)?
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Partial | Self::Cancelled | Self::Inconclusive
        )
    }
}

/// Integrity of a piece of evidence relative to its recorded hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityState {
    Unknown,
    Pending,
    Verified,
    Mismatch,
    Failed,
}

/// Validation outcome for a recovered candidate or reconstructed artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationState {
    Unknown,
    Valid,
    Invalid,
    Partial,
    Inconclusive,
}

/// Explainable confidence for a recovery result.
///
/// This enum is intentionally coarse. The reasons live in
/// `ConfidenceReasons`, which will be added alongside the recovery engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Moderate,
    Low,
    Uncertain,
}

/// How a fragment was placed into a reconstructed artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconstructionState {
    Contiguous,
    Reconstructed,
    PartiallyReconstructed,
    Uncertain,
    Invalid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_job_states() {
        assert!(JobState::Succeeded.is_terminal());
        assert!(JobState::Failed.is_terminal());
        assert!(JobState::Partial.is_terminal());
        assert!(JobState::Cancelled.is_terminal());
        assert!(JobState::Inconclusive.is_terminal());
        assert!(!JobState::Created.is_terminal());
        assert!(!JobState::Queued.is_terminal());
        assert!(!JobState::Running.is_terminal());
        assert!(!JobState::Verifying.is_terminal());
    }

    #[test]
    fn states_serialize_snake_case() {
        let j = serde_json::to_string(&JobState::Inconclusive).unwrap();
        assert_eq!(j, "\"inconclusive\"");
        let i = serde_json::to_string(&IntegrityState::Mismatch).unwrap();
        assert_eq!(i, "\"mismatch\"");
    }
}
