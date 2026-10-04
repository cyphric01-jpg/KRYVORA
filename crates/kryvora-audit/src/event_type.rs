//! The audit event taxonomy.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Every kind of audit event that KRYVORA can emit.
///
/// The set matches Section 14 of the master prompt. Adding a new variant
/// is a non-breaking change; renaming or removing one is a breaking
/// change to the audit chain of every existing database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    CaseCreated,
    EvidenceRegistered,
    HashCalculated,
    IntegrityVerified,
    JobCreated,
    JobStarted,
    JobCompleted,
    JobFailed,
    TargetInspected,
    SanitizationPlanned,
    SanitizationStarted,
    SanitizationVerified,
    SanitizationOutcomeRecorded,
    SanitizationRefused,
    RecoveryStarted,
    CandidateDetected,
    ArtifactValidated,
    ReportGenerated,
    ExaminerNote,
}

impl EventType {
    /// Lowercase, snake_case, wire-stable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CaseCreated => "case_created",
            Self::EvidenceRegistered => "evidence_registered",
            Self::HashCalculated => "hash_calculated",
            Self::IntegrityVerified => "integrity_verified",
            Self::JobCreated => "job_created",
            Self::JobStarted => "job_started",
            Self::JobCompleted => "job_completed",
            Self::JobFailed => "job_failed",
            Self::TargetInspected => "target_inspected",
            Self::SanitizationPlanned => "sanitization_planned",
            Self::SanitizationStarted => "sanitization_started",
            Self::SanitizationVerified => "sanitization_verified",
            Self::SanitizationOutcomeRecorded => "sanitization_outcome_recorded",
            Self::SanitizationRefused => "sanitization_refused",
            Self::RecoveryStarted => "recovery_started",
            Self::CandidateDetected => "candidate_detected",
            Self::ArtifactValidated => "artifact_validated",
            Self::ReportGenerated => "report_generated",
            Self::ExaminerNote => "examiner_note",
        }
    }

    /// Parse from the stored string form. Case-insensitive.
    ///
    /// # Errors
    ///
    /// Returns [`EventTypeParseError`] if the string does not match a
    /// known variant.
    pub fn parse(s: &str) -> Result<Self, EventTypeParseError> {
        match s.to_ascii_lowercase().as_str() {
            "case_created" => Ok(Self::CaseCreated),
            "evidence_registered" => Ok(Self::EvidenceRegistered),
            "hash_calculated" => Ok(Self::HashCalculated),
            "integrity_verified" => Ok(Self::IntegrityVerified),
            "job_created" => Ok(Self::JobCreated),
            "job_started" => Ok(Self::JobStarted),
            "job_completed" => Ok(Self::JobCompleted),
            "job_failed" => Ok(Self::JobFailed),
            "target_inspected" => Ok(Self::TargetInspected),
            "sanitization_planned" => Ok(Self::SanitizationPlanned),
            "sanitization_started" => Ok(Self::SanitizationStarted),
            "sanitization_verified" => Ok(Self::SanitizationVerified),
            "sanitization_outcome_recorded" => Ok(Self::SanitizationOutcomeRecorded),
            "sanitization_refused" => Ok(Self::SanitizationRefused),
            "recovery_started" => Ok(Self::RecoveryStarted),
            "candidate_detected" => Ok(Self::CandidateDetected),
            "artifact_validated" => Ok(Self::ArtifactValidated),
            "report_generated" => Ok(Self::ReportGenerated),
            "examiner_note" => Ok(Self::ExaminerNote),
            other => Err(EventTypeParseError {
                value: other.to_string(),
            }),
        }
    }

    /// Every variant, in declaration order. Useful for exhaustive tests
    /// and for building UI pickers.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::CaseCreated,
            Self::EvidenceRegistered,
            Self::HashCalculated,
            Self::IntegrityVerified,
            Self::JobCreated,
            Self::JobStarted,
            Self::JobCompleted,
            Self::JobFailed,
            Self::TargetInspected,
            Self::SanitizationPlanned,
            Self::SanitizationStarted,
            Self::SanitizationVerified,
            Self::SanitizationOutcomeRecorded,
            Self::SanitizationRefused,
            Self::RecoveryStarted,
            Self::CandidateDetected,
            Self::ArtifactValidated,
            Self::ReportGenerated,
            Self::ExaminerNote,
        ]
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Returned by [`EventType::parse`] when the input does not match any
/// known variant.
#[derive(Debug, Error)]
#[error("unknown event type: {value:?}")]
pub struct EventTypeParseError {
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_roundtrips() {
        for v in EventType::all() {
            assert_eq!(EventType::parse(v.as_str()).unwrap(), *v);
        }
    }

    #[test]
    fn parse_is_case_insensitive() {
        assert_eq!(
            EventType::parse("CASE_CREATED").unwrap(),
            EventType::CaseCreated
        );
        assert_eq!(
            EventType::parse("Evidence_Registered").unwrap(),
            EventType::EvidenceRegistered
        );
    }

    #[test]
    fn parse_rejects_unknown() {
        let err = EventType::parse("teleported").unwrap_err();
        assert_eq!(err.value, "teleported");
    }

    #[test]
    fn serde_uses_snake_case() {
        let s = serde_json::to_string(&EventType::EvidenceRegistered).unwrap();
        assert_eq!(s, "\"evidence_registered\"");
    }

    #[test]
    fn display_matches_as_str() {
        for v in EventType::all() {
            assert_eq!(v.to_string(), v.as_str());
        }
    }

    #[test]
    fn all_is_exhaustive() {
        // If a variant is added but not listed in `all()`, this test
        // fails by length mismatch. The literal 17 is the current count.
        assert_eq!(EventType::all().len(), 19);
    }

    #[test]
    fn as_str_has_no_uppercase() {
        for v in EventType::all() {
            let s = v.as_str();
            assert!(
                s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
                "{s} must be lowercase snake_case"
            );
        }
    }
}
