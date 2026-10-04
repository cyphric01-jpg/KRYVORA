//! The recovery result value type and the recovery report.

use crate::artifact::{ArtifactCategory, RecoveryMethod};
use crate::confidence::ConfidenceAssessment;
use kryvora_core::{
    CaseId, EvidenceId, JobId, ProvenanceId, ReconstructionState, RecoveryResultId, ReportId,
    ValidationState,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

/// A single recovered and validated artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryResult {
    pub id: RecoveryResultId,
    pub evidence_id: Option<EvidenceId>,
    pub source_offset: u64,
    pub source_length: u64,
    pub detected_type: String,
    pub category: ArtifactCategory,
    pub validation_state: ValidationState,
    pub confidence: ConfidenceAssessment,
    pub recovery_method: RecoveryMethod,
    pub reconstruction_state: ReconstructionState,
    pub artifact_sha256: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub job_id: Option<JobId>,
    pub provenance_id: Option<ProvenanceId>,
    pub report_id: Option<ReportId>,
    pub validation_facts: serde_json::Value,
}

/// The full report of a recovery pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryReport {
    pub case_id: Option<CaseId>,
    pub job_id: Option<JobId>,
    pub candidates_considered: u64,
    pub candidates_validated: u64,
    pub candidates_rejected: u64,
    pub results: Vec<RecoveryResult>,
}

impl RecoveryReport {
    /// Number of artifacts that were successfully validated.
    #[must_use]
    pub fn validated_count(&self) -> u64 {
        self.results.len() as u64
    }
}

/// Errors from recovery above and beyond [`kryvora_core::Error`].
#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("candidate bytes could not be read: {0}")]
    CandidateReadFailed(String),

    #[error("no validator registered for format {0:?}")]
    NoValidator(String),

    #[error("candidate length {length} exceeds practical limit {limit} for format {format}")]
    CandidateTooLarge {
        format: String,
        length: u64,
        limit: u64,
    },

    #[error("candidate offset {offset} is beyond the source length {source_length}")]
    OffsetOutOfRange { offset: u64, source_length: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confidence::ConfidenceAssessment;

    fn dummy_result() -> RecoveryResult {
        RecoveryResult {
            id: RecoveryResultId::new(),
            evidence_id: None,
            source_offset: 0,
            source_length: 100,
            detected_type: "jpeg".into(),
            category: ArtifactCategory::Image,
            validation_state: ValidationState::Valid,
            confidence: ConfidenceAssessment::all_satisfied(),
            recovery_method: RecoveryMethod::SignatureCarving,
            reconstruction_state: ReconstructionState::Contiguous,
            artifact_sha256: "0".repeat(64),
            created_at: OffsetDateTime::now_utc(),
            job_id: None,
            provenance_id: None,
            report_id: None,
            validation_facts: serde_json::json!({}),
        }
    }

    #[test]
    fn report_validated_count_matches_results_len() {
        let report = RecoveryReport {
            case_id: None,
            job_id: None,
            candidates_considered: 3,
            candidates_validated: 2,
            candidates_rejected: 1,
            results: vec![dummy_result(), dummy_result()],
        };
        assert_eq!(report.validated_count(), 2);
    }

    #[test]
    fn result_roundtrips_through_json() {
        let r = dummy_result();
        let s = serde_json::to_string(&r).unwrap();
        let back: RecoveryResult = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }
}
