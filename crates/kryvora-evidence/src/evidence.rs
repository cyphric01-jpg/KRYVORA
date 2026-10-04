//! The `Evidence` domain type and the registration / verification
//! operations on top of it.

use crate::source_type::SourceType;
use kryvora_core::{CaseId, Error, EvidenceId, IntegrityState, Result};
use kryvora_db::repo::{EvidenceRepository, EvidenceRow, NewEvidence, SqliteEvidenceRepository};
use kryvora_integrity::Hash;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

/// The KRYVORA tool version recorded against every registered evidence
/// item. Callers pass a value; this crate does not hard-code a version
/// string, so the same crate can be used by tests that want to
/// distinguish runs.
pub type ToolVersion = String;

/// The domain type for a piece of registered evidence.
///
/// `Evidence` is the caller-facing shape. It is converted to and from
/// [`EvidenceRow`] at the persistence boundary. Fields that the storage
/// layer holds as strings (timestamps, hash hex) are held here as
/// their native types, so callers do not parse or format anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub id: EvidenceId,
    pub case_id: CaseId,
    pub source_type: SourceType,
    pub source_path: String,
    pub size_bytes: u64,
    pub hash: Hash,
    pub integrity_state: IntegrityState,
    pub read_only: bool,
    pub tool_version: ToolVersion,
    pub acquisition_metadata: Option<serde_json::Value>,
    pub notes: Option<String>,
    pub created_at: OffsetDateTime,
}

/// The input to [`register_evidence`].
///
/// This exists as a struct rather than a long argument list so that
/// callers can construct it incrementally and so that adding a field
/// later is not a breaking change at every call site.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationRequest {
    pub case_id: CaseId,
    pub source_type: SourceType,
    pub source_path: String,
    pub size_bytes: u64,
    pub hash: Hash,
    pub read_only: bool,
    pub tool_version: ToolVersion,
    pub acquisition_metadata: Option<serde_json::Value>,
    pub notes: Option<String>,
}

/// Errors produced by this crate above and beyond
/// [`kryvora_core::Error`].
///
/// Currently a thin wrapper used for documentation. Persistence errors
/// flow through as [`kryvora_core::Error`]; this type is reserved for
/// evidence-specific validation failures that do not fit an existing
/// `kryvora_core::Error` variant.
#[derive(Debug, Error)]
pub enum EvidenceError {
    #[error("evidence registration requires a non-empty source_path")]
    EmptySourcePath,

    #[error("evidence hash algorithm {algorithm} is not supported for storage")]
    UnsupportedHashAlgorithm { algorithm: String },

    #[error("evidence size_bytes {supplied} does not match hash input size {hashed}")]
    SizeMismatch { supplied: u64, hashed: u64 },

    #[error("acquisition metadata is not valid JSON: {0}")]
    InvalidMetadata(String),
}

/// Register a new piece of evidence.
///
/// The caller is responsible for computing `hash` from the source. This
/// function performs **no file I/O**; it validates the request, maps it
/// to a [`NewEvidence`] row, and inserts it through the repository.
///
/// On success, returns the freshly assigned [`EvidenceId`]. The
/// corresponding row can be retrieved with [`get_evidence`] or
/// [`verify_evidence`].
///
/// # Errors
///
/// * [`Error::InvalidInput`] — source path empty, hash algorithm not
///   supported by the storage layer, supplied size differs from the
///   hash input size, or metadata cannot be serialized.
/// * [`Error::Database`] / [`Error::InvalidInput`] — from the
///   repository (see [`kryvora_db::repo`]).
pub fn register_evidence(conn: &Connection, request: &RegistrationRequest) -> Result<EvidenceId> {
    if request.source_path.trim().is_empty() {
        return Err(Error::InvalidInput(
            EvidenceError::EmptySourcePath.to_string(),
        ));
    }

    if request.hash.algorithm().as_str() != "sha256" {
        return Err(Error::InvalidInput(
            EvidenceError::UnsupportedHashAlgorithm {
                algorithm: request.hash.algorithm().as_str().to_string(),
            }
            .to_string(),
        ));
    }

    if request.size_bytes != request.hash.input_size() {
        return Err(Error::InvalidInput(
            EvidenceError::SizeMismatch {
                supplied: request.size_bytes,
                hashed: request.hash.input_size(),
            }
            .to_string(),
        ));
    }

    let metadata_json = match &request.acquisition_metadata {
        None => None,
        Some(v) => Some(serde_json::to_string(v).map_err(|e| {
            Error::InvalidInput(EvidenceError::InvalidMetadata(e.to_string()).to_string())
        })?),
    };

    let row = NewEvidence {
        case_id: request.case_id,
        source_type: request.source_type.as_str().to_string(),
        source_path: request.source_path.clone(),
        size_bytes: request.size_bytes,
        hash_algorithm: request.hash.algorithm().as_str().to_string(),
        hash_digest: request.hash.digest_hex().to_string(),
        integrity_state: integrity_state_to_str(IntegrityState::Verified).to_string(),
        read_only: request.read_only,
        tool_version: request.tool_version.clone(),
        acquisition_metadata: metadata_json,
        notes: request.notes.clone(),
    };

    let repo = SqliteEvidenceRepository::new(conn);
    repo.insert(&row)
}

/// Fetch an [`Evidence`] by id.
///
/// # Errors
///
/// * [`Error::NotFound`] — no evidence with that id exists.
/// * Other [`Error`] variants from the repository.
pub fn get_evidence(conn: &Connection, id: &EvidenceId) -> Result<Evidence> {
    let repo = SqliteEvidenceRepository::new(conn);
    let row = repo
        .get(id)?
        .ok_or_else(|| Error::NotFound(format!("evidence {id}")))?;
    row_to_evidence(row)
}

/// Recompute the hash of a source and compare it to the recorded hash.
///
/// The caller opens the source, streams it through
/// [`kryvora_integrity::calculate_hash`], and passes the result here.
/// This function only compares. It does not touch the filesystem.
///
/// # Errors
///
/// * [`Error::NotFound`] — no evidence with that id exists.
/// * Other [`Error`] variants from the repository.
pub fn verify_evidence(
    conn: &Connection,
    id: &EvidenceId,
    actual: &Hash,
) -> Result<IntegrityState> {
    let recorded = get_evidence(conn, id)?;

    if recorded.hash.digest_matches(actual) && recorded.size_bytes == actual.input_size() {
        Ok(IntegrityState::Verified)
    } else {
        Ok(IntegrityState::Mismatch)
    }
}

/// Convert a database row into the domain type.
fn row_to_evidence(row: EvidenceRow) -> Result<Evidence> {
    let source_type = SourceType::parse(&row.source_type).map_err(|e| {
        Error::Internal(format!(
            "stored evidence {} has invalid source_type: {e}",
            row.id
        ))
    })?;

    let hash = Hash::new(
        kryvora_integrity::Algorithm::Sha256,
        row.hash_digest.clone(),
        row.size_bytes,
        parse_rfc3339(&row.created_at)?,
    )
    .map_err(|e| Error::Internal(format!("stored evidence {} has invalid hash: {e}", row.id)))?;

    let integrity_state = parse_integrity_state(&row.integrity_state).ok_or_else(|| {
        Error::Internal(format!(
            "stored evidence {} has invalid integrity_state: {}",
            row.id, row.integrity_state
        ))
    })?;

    let acquisition_metadata = match row.acquisition_metadata.as_deref() {
        None => None,
        Some(s) => Some(serde_json::from_str(s).map_err(|e| {
            Error::Internal(format!(
                "stored evidence {} has invalid acquisition_metadata: {e}",
                row.id
            ))
        })?),
    };

    Ok(Evidence {
        id: row.id,
        case_id: row.case_id,
        source_type,
        source_path: row.source_path,
        size_bytes: row.size_bytes,
        hash,
        integrity_state,
        read_only: row.read_only,
        tool_version: row.tool_version,
        acquisition_metadata,
        notes: row.notes,
        created_at: parse_rfc3339(&row.created_at)?,
    })
}

fn parse_rfc3339(s: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
        .map_err(|e| Error::Internal(format!("invalid RFC 3339 timestamp {s:?}: {e}")))
}

fn integrity_state_to_str(s: IntegrityState) -> &'static str {
    match s {
        IntegrityState::Unknown => "unknown",
        IntegrityState::Pending => "pending",
        IntegrityState::Verified => "verified",
        IntegrityState::Mismatch => "mismatch",
        IntegrityState::Failed => "failed",
    }
}

fn parse_integrity_state(s: &str) -> Option<IntegrityState> {
    match s {
        "unknown" => Some(IntegrityState::Unknown),
        "pending" => Some(IntegrityState::Pending),
        "verified" => Some(IntegrityState::Verified),
        "mismatch" => Some(IntegrityState::Mismatch),
        "failed" => Some(IntegrityState::Failed),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_state_roundtrip() {
        for s in [
            IntegrityState::Unknown,
            IntegrityState::Pending,
            IntegrityState::Verified,
            IntegrityState::Mismatch,
            IntegrityState::Failed,
        ] {
            assert_eq!(parse_integrity_state(integrity_state_to_str(s)), Some(s));
        }
    }

    #[test]
    fn parse_integrity_state_rejects_unknown() {
        assert_eq!(parse_integrity_state("good"), None);
    }

    #[test]
    fn parse_rfc3339_accepts_canonical() {
        let s = "2026-09-28T12:55:00Z";
        let t = parse_rfc3339(s).unwrap();
        assert_eq!(t.year(), 2026);
        assert_eq!(t.month() as u8, 9);
        assert_eq!(t.day(), 28);
    }

    #[test]
    fn parse_rfc3339_rejects_garbage() {
        assert!(parse_rfc3339("not-a-timestamp").is_err());
    }
}
