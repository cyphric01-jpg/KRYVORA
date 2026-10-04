//! Report generation: render HTML, write to disk, compute SHA-256.

use crate::html::{render_recovery_html, render_sanitization_html};
use kryvora_core::{Error, Result};
use kryvora_integrity::calculate_hash;
use kryvora_recovery::RecoveryReport;
use kryvora_sanitize::SanitizationResult;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// The result of generating a report.
#[derive(Debug, Clone)]
pub struct GeneratedReport {
    /// Where the report file was written.
    pub path: PathBuf,
    /// Size of the report file in bytes.
    pub byte_size: u64,
    /// Lowercase hex SHA-256 of the report file's bytes.
    pub sha256: String,
}

/// Errors specific to report generation.
#[derive(Debug, Error)]
pub enum ReportError {
    #[error("report path must not be empty")]
    EmptyPath,
    #[error("report directory does not exist: {0}")]
    DirectoryMissing(String),
}

/// Generate a recovery report at `path`.
///
/// The HTML is rendered, written to disk as UTF-8, hashed, and returned
/// as a [`GeneratedReport`]. The caller is responsible for persisting
/// the metadata via `kryvora-db`.
///
/// # Errors
///
/// * [`Error::InvalidInput`] — path is empty.
/// * [`Error::Io`] — the file could not be written.
pub fn generate_recovery_report(
    path: impl AsRef<Path>,
    title: &str,
    case_id: Option<&str>,
    report: &RecoveryReport,
) -> Result<GeneratedReport> {
    let html = render_recovery_html(title, case_id, report);
    write_and_hash(path.as_ref(), &html)
}

/// Generate a sanitization report at `path`.
///
/// # Errors
///
/// Same as [`generate_recovery_report`].
pub fn generate_sanitization_report(
    path: impl AsRef<Path>,
    title: &str,
    target: &str,
    result: &SanitizationResult,
) -> Result<GeneratedReport> {
    let html = render_sanitization_html(title, target, result);
    write_and_hash(path.as_ref(), &html)
}

pub(crate) fn write_and_hash(path: &Path, html: &str) -> Result<GeneratedReport> {
    if path.as_os_str().is_empty() {
        return Err(Error::InvalidInput(ReportError::EmptyPath.to_string()));
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            return Err(Error::InvalidInput(
                ReportError::DirectoryMissing(parent.display().to_string()).to_string(),
            ));
        }
    }

    let bytes = html.as_bytes();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(Error::Io)?;
    file.write_all(bytes).map_err(Error::Io)?;
    file.sync_all().map_err(Error::Io)?;

    let hash = calculate_hash(Cursor::new(bytes))?;

    Ok(GeneratedReport {
        path: path.to_path_buf(),
        byte_size: bytes.len() as u64,
        sha256: hash.digest_hex().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kryvora_core::{ReconstructionState, RecoveryResultId, ValidationState};
    use kryvora_recovery::{
        ArtifactCategory, ConfidenceAssessment, ConfidenceReason, ConfidenceSignal, RecoveryMethod,
        RecoveryReport, RecoveryResult,
    };
    use std::fs;
    use time::OffsetDateTime;

    fn dummy_result() -> RecoveryResult {
        RecoveryResult {
            id: RecoveryResultId::new(),
            evidence_id: None,
            source_offset: 0,
            source_length: 100,
            detected_type: "jpeg".into(),
            category: ArtifactCategory::Image,
            validation_state: ValidationState::Valid,
            confidence: ConfidenceAssessment::from_reasons(vec![ConfidenceReason::new(
                ConfidenceSignal::SignatureValid,
                true,
            )]),
            recovery_method: RecoveryMethod::SignatureCarving,
            reconstruction_state: ReconstructionState::Contiguous,
            artifact_sha256: "a".repeat(64),
            created_at: OffsetDateTime::now_utc(),
            job_id: None,
            provenance_id: None,
            report_id: None,
            validation_facts: serde_json::json!({}),
        }
    }

    #[test]
    fn generates_and_hashes_recovery_report() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rec.html");

        let report = RecoveryReport {
            case_id: None,
            job_id: None,
            candidates_considered: 1,
            candidates_validated: 1,
            candidates_rejected: 0,
            results: vec![dummy_result()],
        };

        let generated = generate_recovery_report(&path, "Test", Some("CASE-1"), &report).unwrap();

        assert!(path.is_file());
        assert_eq!(generated.byte_size, fs::metadata(&path).unwrap().len());

        // Recompute hash from disk and confirm it matches.
        let bytes = fs::read(&path).unwrap();
        let recomputed = kryvora_integrity::calculate_hash(Cursor::new(&bytes)).unwrap();
        assert_eq!(recomputed.digest_hex(), generated.sha256);
        assert_eq!(generated.sha256.len(), 64);
    }

    #[test]
    fn missing_directory_is_rejected() {
        let path = PathBuf::from("C:/definitely/not/here/report.html");
        let report = RecoveryReport {
            case_id: None,
            job_id: None,
            candidates_considered: 0,
            candidates_validated: 0,
            candidates_rejected: 0,
            results: vec![],
        };
        let err = generate_recovery_report(&path, "T", None, &report).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    }

    #[test]
    fn empty_path_is_rejected() {
        let report = RecoveryReport {
            case_id: None,
            job_id: None,
            candidates_considered: 0,
            candidates_validated: 0,
            candidates_rejected: 0,
            results: vec![],
        };
        let err = generate_recovery_report("", "T", None, &report).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    }

    #[test]
    fn existing_report_target_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing.html");
        fs::write(&path, b"keep this file").unwrap();
        let report = RecoveryReport {
            case_id: None,
            job_id: None,
            candidates_considered: 0,
            candidates_validated: 0,
            candidates_rejected: 0,
            results: vec![],
        };

        assert!(generate_recovery_report(&path, "T", None, &report).is_err());
        assert_eq!(fs::read(path).unwrap(), b"keep this file");
    }
}
