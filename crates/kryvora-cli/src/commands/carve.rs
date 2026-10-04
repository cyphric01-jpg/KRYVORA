// file: crates/kryvora-cli/src/commands/carve.rs
//! `kryvora carve` — carve a source into candidates, validate them,
//! and persist recovery results with provenance.

use kryvora_carving::{run_carving_job_for_evidence, ScanOptions};
use kryvora_core::Error;
use kryvora_db::repo::{
    CaseRepository, EvidenceRepository, NewCase, NewEvidence, SqliteCaseRepository,
    SqliteEvidenceRepository,
};
use kryvora_db::{apply_migrations, open};
use kryvora_integrity::calculate_hash;
use kryvora_provenance::{persist_recovery_result, ChainInputs};
use kryvora_recovery::{run_recovery_job_for_case, RecoveryResult};
use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::path::Path;

/// The KRYVORA tool version recorded against evidence registered by
/// this command.
pub const TOOL_VERSION: &str = "0.1.0";

/// Execute a carve + recover + persist pipeline against a source file.
///
/// # Errors
///
/// * [`Error::NotFound`] — source path does not exist.
/// * [`Error::InvalidInput`] — source is not a regular file.
/// * Any repository, job, or I/O error.
pub fn execute(
    db_path: &Path,
    source: &Path,
    case_title: &str,
    examiner: Option<&str>,
    window_size: Option<usize>,
) -> kryvora_core::Result<()> {
    if !source.is_file() {
        return Err(Error::InvalidInput(format!(
            "source is not a regular file: {}",
            source.display()
        )));
    }

    let mut source_file = File::open(source).map_err(Error::Io)?;
    if !source_file.metadata().map_err(Error::Io)?.is_file() {
        return Err(Error::InvalidInput(format!(
            "source is not a regular file: {}",
            source.display()
        )));
    }
    let source_path = source.canonicalize().map_err(Error::Io)?;
    let source_display = source_path.display().to_string();

    let mut conn = open(db_path)?;
    apply_migrations(&mut conn)?;
    conn.execute_batch("SAVEPOINT kryvora_carve_operation")
        .map_err(|e| Error::Database(format!("begin carve operation: {e}")))?;

    let hash = calculate_hash(&mut source_file)?;
    let size_bytes = hash.input_size();

    let case_repo = SqliteCaseRepository::new(&conn);
    let case_id = case_repo.insert(&NewCase {
        title: case_title.to_string(),
        examiner: examiner.map(str::to_string),
        notes: None,
    })?;

    let evidence_repo = SqliteEvidenceRepository::new(&conn);
    let evidence_id = evidence_repo.insert(&NewEvidence {
        case_id,
        source_type: "file".into(),
        source_path: source_display.clone(),
        size_bytes,
        hash_algorithm: hash.algorithm().as_str().to_string(),
        hash_digest: hash.digest_hex().to_string(),
        integrity_state: "verified".into(),
        read_only: true,
        tool_version: TOOL_VERSION.to_string(),
        acquisition_metadata: None,
        notes: None,
    })?;

    let carve_opts = ScanOptions {
        window_size: window_size.unwrap_or(1024 * 1024),
        header_preview_len: 16,
        source_size_bytes: Some(size_bytes),
        ..ScanOptions::default()
    };

    source_file.seek(SeekFrom::Start(0)).map_err(Error::Io)?;
    let carve_reader = BufReader::new(source_file.try_clone().map_err(Error::Io)?);

    let carve_report = match run_carving_job_for_evidence(
        &conn,
        carve_reader,
        carve_opts,
        examiner.map(str::to_string),
        Some(case_id),
        Some(evidence_id),
    )? {
        Some(r) => r,
        None => {
            eprintln!("carve job was cancelled");
            return Err(Error::JobCancelled);
        }
    };

    println!("=== KRYVORA carve ===");
    println!("case:                {case_id}");
    println!("evidence:            {evidence_id}");
    println!("source:              {source_display}");
    println!("size:                {size_bytes} bytes");
    println!("candidates found:    {}", carve_report.candidates_found);
    println!("truncated headers:   {}", carve_report.truncated_headers);
    println!();

    if carve_report.candidates.is_empty() {
        println!("No candidates found. Nothing to recover.");
        conn.execute_batch("RELEASE SAVEPOINT kryvora_carve_operation")
            .map_err(|e| Error::Database(format!("commit carve operation: {e}")))?;
        return Ok(());
    }

    source_file.seek(SeekFrom::Start(0)).map_err(Error::Io)?;
    let candidates = carve_report.candidates;

    let recovery_report = match run_recovery_job_for_case(
        &conn,
        source_file.try_clone().map_err(Error::Io)?,
        candidates,
        Some(evidence_id),
        Some(case_id),
        examiner.map(str::to_string),
    )? {
        Some(r) => r,
        None => {
            eprintln!("recovery job was cancelled");
            return Err(Error::JobCancelled);
        }
    };

    source_file.seek(SeekFrom::Start(0)).map_err(Error::Io)?;
    let post_scan_hash = calculate_hash(&mut source_file)?;
    if post_scan_hash.input_size() != size_bytes || post_scan_hash.digest_hex() != hash.digest_hex()
    {
        return Err(Error::IntegrityMismatch {
            expected: hash.digest_hex().to_string(),
            actual: post_scan_hash.digest_hex().to_string(),
        });
    }

    println!("=== KRYVORA recover ===");
    println!(
        "candidates considered: {}",
        recovery_report.candidates_considered
    );
    println!(
        "candidates validated:  {}",
        recovery_report.candidates_validated
    );
    println!(
        "candidates rejected:   {}",
        recovery_report.candidates_rejected
    );
    println!();

    // Persist provenance + recovery results.
    let mut persisted: u64 = 0;
    for result in &recovery_report.results {
        let inputs = ChainInputs {
            case_id,
            evidence_id,
            job_id: recovery_report.job_id,
            candidate_object_id: candidate_object_id(result),
            actor_note: Some("kryvora carve".to_string()),
            artifact_path: None,
        };
        persist_recovery_result(&conn, &inputs, result)?;
        persisted += 1;
    }

    for r in &recovery_report.results {
        println!(
            "  {} @ offset {} length {} confidence {:?} sha256={}",
            r.detected_type,
            r.source_offset,
            r.source_length,
            r.confidence.level,
            &r.artifact_sha256[..16.min(r.artifact_sha256.len())]
        );
    }
    println!();
    println!("persisted recovery results: {persisted}");
    println!();

    conn.execute_batch("RELEASE SAVEPOINT kryvora_carve_operation")
        .map_err(|e| Error::Database(format!("commit carve operation: {e}")))?;
    Ok(())
}

/// Parse a `--window-size` argument string.
#[allow(dead_code)]
fn parse_window_size(s: &str) -> Option<usize> {
    s.trim().parse::<usize>().ok().filter(|n| *n > 0)
}

fn candidate_object_id(result: &RecoveryResult) -> String {
    format!(
        "CANDIDATE-{}-{}",
        result.source_offset, result.source_length
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use kryvora_core::{ReconstructionState, RecoveryResultId, ValidationState};
    use kryvora_recovery::{ArtifactCategory, ConfidenceAssessment, RecoveryMethod};
    use time::OffsetDateTime;

    #[test]
    fn provenance_candidate_id_uses_the_accepted_result_offset() {
        let result = RecoveryResult {
            id: RecoveryResultId::new(),
            evidence_id: None,
            source_offset: 731,
            source_length: 42,
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
        };

        assert_eq!(candidate_object_id(&result), "CANDIDATE-731-42");
    }
}
