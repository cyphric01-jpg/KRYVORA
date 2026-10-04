// file: crates/kryvora-sanitize/src/result.rs
//! The result of a single-file sanitization.

use crate::outcome::SanitizeOutcome;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

/// The result of sanitizing a single file.
///
/// `outcome` is authoritative. `bytes_overwritten` is the number of
/// bytes actually written by this operation, not a metadata size, not
/// a hash length, not a constant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanitizationResult {
    pub outcome: SanitizeOutcome,
    pub bytes_overwritten: u64,
    #[serde(with = "duration_secs")]
    pub elapsed: Duration,
    /// Set when the outcome is not `Success`.
    pub reason: Option<String>,
}

impl SanitizationResult {
    #[must_use]
    pub fn success(bytes_overwritten: u64, elapsed: Duration) -> Self {
        Self {
            outcome: SanitizeOutcome::Success,
            bytes_overwritten,
            elapsed,
            reason: None,
        }
    }

    #[must_use]
    pub fn not_verified(bytes_overwritten: u64, elapsed: Duration, reason: String) -> Self {
        Self {
            outcome: SanitizeOutcome::NotVerified,
            bytes_overwritten,
            elapsed,
            reason: Some(reason),
        }
    }

    #[must_use]
    pub fn partial(bytes_overwritten: u64, elapsed: Duration, reason: String) -> Self {
        Self {
            outcome: SanitizeOutcome::Partial,
            bytes_overwritten,
            elapsed,
            reason: Some(reason),
        }
    }

    #[must_use]
    pub fn failed(elapsed: Duration, reason: String) -> Self {
        Self {
            outcome: SanitizeOutcome::Failed,
            bytes_overwritten: 0,
            elapsed,
            reason: Some(reason),
        }
    }

    #[must_use]
    pub fn unsupported(reason: String) -> Self {
        Self {
            outcome: SanitizeOutcome::Unsupported,
            bytes_overwritten: 0,
            elapsed: Duration::from_secs(0),
            reason: Some(reason),
        }
    }

    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self.outcome, SanitizeOutcome::Success)
    }
}

/// Errors from sanitization above and beyond [`kryvora_core::Error`].
#[derive(Debug, Error)]
pub enum SanitizeError {
    #[error("target is not a regular file: {0}")]
    NotAFile(String),

    #[error("target is read-only: {0}")]
    ReadOnly(String),

    #[error("could not obtain random bytes: {0}")]
    RandomUnavailable(String),

    #[error("original digest could not be computed: {0}")]
    HashFailed(String),

    #[error("verification failed: original digest is still present after overwrite")]
    VerificationFailed,

    #[error("path is a protected system path: {0}")]
    SystemPath(String),

    #[error("unsupported target: {0}")]
    Unsupported(String),
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        d.as_secs_f64().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = f64::deserialize(d)?;
        Ok(Duration::from_secs_f64(secs.max(0.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_carries_outcome_and_bytes() {
        let r = SanitizationResult::success(1234, Duration::from_millis(10));
        assert_eq!(r.outcome, SanitizeOutcome::Success);
        assert_eq!(r.bytes_overwritten, 1234);
        assert!(r.reason.is_none());
        assert!(r.is_success());
    }

    #[test]
    fn not_verified_carries_reason() {
        let r = SanitizationResult::not_verified(100, Duration::from_millis(5), "ssd media".into());
        assert_eq!(r.outcome, SanitizeOutcome::NotVerified);
        assert_eq!(r.bytes_overwritten, 100);
        assert!(r.reason.as_deref().unwrap().contains("ssd"));
        assert!(!r.is_success());
    }

    #[test]
    fn failed_carries_zero_bytes() {
        let r = SanitizationResult::failed(Duration::from_millis(1), "oops".into());
        assert_eq!(r.bytes_overwritten, 0);
        assert_eq!(r.outcome, SanitizeOutcome::Failed);
    }

    #[test]
    fn unsupported_carries_reason_and_zero_bytes() {
        let r = SanitizationResult::unsupported("device target".into());
        assert_eq!(r.outcome, SanitizeOutcome::Unsupported);
        assert_eq!(r.bytes_overwritten, 0);
        assert!(r.reason.is_some());
    }

    #[test]
    fn result_roundtrips_through_json() {
        let r = SanitizationResult::success(100, Duration::from_millis(250));
        let s = serde_json::to_string(&r).unwrap();
        let back: SanitizationResult = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }
}
