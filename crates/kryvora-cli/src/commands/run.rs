//! `kryvora run` — the v0.1 vertical slice.

use crate::output;
use kryvora_audit::{append, verify_chain, ChainStatus, EventDraft, EventType};
use kryvora_core::Error;
use kryvora_db::repo::{CaseRepository, NewCase, SqliteCaseRepository};
use kryvora_db::{apply_migrations, open};
use kryvora_evidence::{
    get_evidence, register_evidence, verify_evidence, RegistrationRequest, SourceType,
};
use kryvora_integrity::calculate_hash;
use serde_json::json;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// The KRYVORA tool version recorded against every registered
/// evidence item. Kept in one place so it can be bumped centrally.
pub const TOOL_VERSION: &str = "0.1.0";

/// Execute the full v0.1 workflow.
///
/// # Errors
///
/// Returns any error from opening the database, reading the source
/// file, computing the hash, registering evidence, or appending audit
/// events. Every error is surfaced; none are swallowed.
pub fn execute(
    db_path: &Path,
    case_title: &str,
    examiner: Option<&str>,
    source: &Path,
    notes: Option<&str>,
) -> kryvora_core::Result<()> {
    // --- 0. Preconditions ------------------------------------------------
    if !source.is_file() {
        return Err(Error::InvalidInput(format!(
            "source is not a regular file: {}",
            source.display()
        )));
    }

    let source_display = source.display().to_string();

    // --- 1. Open database and apply migrations ---------------------------
    let mut conn = open(db_path)?;
    apply_migrations(&mut conn)?;

    // --- 2. Create case --------------------------------------------------
    let case_repo = SqliteCaseRepository::new(&conn);
    let case_id = case_repo.insert(&NewCase {
        title: case_title.to_string(),
        examiner: examiner.map(str::to_string),
        notes: None,
    })?;

    append(
        &conn,
        &EventDraft {
            event_type: EventType::CaseCreated,
            actor: examiner.map(str::to_string),
            object_id: Some(case_id.to_string()),
            job_id: None,
            details: json!({
                "title": case_title,
                "examiner": examiner,
            }),
        },
    )?;

    // --- 3. Stream the source file and compute SHA-256 -------------------
    let (hash, size_bytes) = hash_file(source)?;

    append(
        &conn,
        &EventDraft {
            event_type: EventType::HashCalculated,
            actor: examiner.map(str::to_string),
            object_id: Some(source_display.clone()),
            job_id: None,
            details: json!({
                "algorithm": hash.algorithm().as_str(),
                "digest":    hash.digest_hex(),
                "size":      size_bytes,
            }),
        },
    )?;

    // --- 4. Register evidence --------------------------------------------
    let request =
        build_registration_request(case_id, &source_display, size_bytes, hash.clone(), notes);

    let evidence_id = register_evidence(&conn, &request)?;

    append(
        &conn,
        &EventDraft {
            event_type: EventType::EvidenceRegistered,
            actor: examiner.map(str::to_string),
            object_id: Some(evidence_id.to_string()),
            job_id: None,
            details: json!({
                "case_id": case_id.to_string(),
                "source":  source_display,
                "sha256":  hash.digest_hex(),
            }),
        },
    )?;

    // --- 5. Verify integrity ---------------------------------------------
    // Re-open and re-hash. This is the actual forensic verification
    // step: two independent reads of the source must produce the same
    // digest.
    let (verify_hash, _) = hash_file(source)?;
    let integrity = verify_evidence(&conn, &evidence_id, &verify_hash)?;

    append(
        &conn,
        &EventDraft {
            event_type: EventType::IntegrityVerified,
            actor: examiner.map(str::to_string),
            object_id: Some(evidence_id.to_string()),
            job_id: None,
            details: json!({
                "state": format!("{integrity:?}").to_lowercase(),
                "sha256": verify_hash.digest_hex(),
            }),
        },
    )?;

    // --- 6. Verify the audit chain ---------------------------------------
    let chain_status = verify_chain(&conn)?;

    // --- 7. Report --------------------------------------------------------
    let evidence = get_evidence(&conn, &evidence_id)?;
    output::print_run_report(
        &case_id.to_string(),
        &evidence_id.to_string(),
        &evidence,
        &chain_status,
        db_path,
    );

    // A broken chain at this point is a bug, not a user error: we just
    // appended four events through the API. Report it as an internal
    // error so the caller does not silently accept a bad state.
    if !matches!(chain_status, ChainStatus::Intact { .. }) {
        return Err(Error::Internal(format!(
            "audit chain is not intact after run: {chain_status:?}"
        )));
    }

    Ok(())
}

/// Open the source, stream it through SHA-256, and return the hash
/// plus the total bytes read.
fn hash_file(path: &Path) -> kryvora_core::Result<(kryvora_integrity::Hash, u64)> {
    let file = File::open(path).map_err(Error::Io)?;
    let reader = BufReader::new(file);
    let hash = calculate_hash(reader)?;
    Ok((hash.clone(), hash.input_size()))
}

/// Build a [`RegistrationRequest`] for the given inputs.
fn build_registration_request(
    case_id: kryvora_core::CaseId,
    source_path: &str,
    size_bytes: u64,
    hash: kryvora_integrity::Hash,
    notes: Option<&str>,
) -> RegistrationRequest {
    RegistrationRequest {
        case_id,
        source_type: SourceType::File,
        source_path: source_path.to_string(),
        size_bytes,
        hash,
        read_only: true,
        tool_version: TOOL_VERSION.to_string(),
        acquisition_metadata: Some(json!({
            "acquired_by": "kryvora-cli",
            "tool_version": TOOL_VERSION,
        })),
        notes: notes.map(str::to_string),
    }
}
