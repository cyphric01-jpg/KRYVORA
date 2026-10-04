// file: crates/kryvora-cli/src/commands/report.rs
//! `kryvora report` — generate a report for a case from real data.

use kryvora_core::{Confidence, Error, ReconstructionState, ValidationState};
use kryvora_db::repo::{
    AuditEventRepository, CaseRepository, EvidenceRepository, RecoveryResultRepository, ReportKind,
    ReportRepository, SqliteAuditEventRepository, SqliteCaseRepository, SqliteEvidenceRepository,
    SqliteRecoveryResultRepository, SqliteReportRepository,
};
use kryvora_db::{apply_migrations, open};
use kryvora_recovery::{
    ArtifactCategory, ConfidenceAssessment, ConfidenceReason, RecoveryMethod, RecoveryReport,
    RecoveryResult,
};
use kryvora_report::generate_recovery_report;
use rusqlite::Connection;
use std::path::Path;
use time::OffsetDateTime;

/// Generate a recovery report for a case using real data.
///
/// # Errors
///
/// * [`Error::NotFound`] — the case does not exist.
/// * Any repository, I/O, or report error.
pub fn execute(db_path: &Path, case_id_str: &str, out_path: &Path) -> kryvora_core::Result<()> {
    let mut conn = open(db_path)?;
    apply_migrations(&mut conn)?;

    let uuid = parse_case_id(case_id_str)
        .ok_or_else(|| Error::InvalidInput(format!("case id is not valid: {case_id_str}")))?;
    let case_id = kryvora_core::CaseId::from_uuid(uuid);

    let case_repo = SqliteCaseRepository::new(&conn);
    let case_row = case_repo
        .get(&case_id)?
        .ok_or_else(|| Error::NotFound(format!("case {case_id}")))?;

    let results = collect_results_for_case(&conn, &case_id)?;
    let considered = results.len() as u64;
    let validated = results
        .iter()
        .filter(|r| {
            matches!(
                r.validation_state,
                ValidationState::Valid | ValidationState::Partial
            )
        })
        .count() as u64;
    let rejected = considered.saturating_sub(validated);

    let report = RecoveryReport {
        case_id: Some(case_id),
        job_id: None,
        candidates_considered: considered,
        candidates_validated: validated,
        candidates_rejected: rejected,
        results,
    };

    let audit_count = audit_event_count(&conn)?;

    let title = format!("Recovery Report — {}", case_row.title);
    let generated =
        generate_recovery_report(out_path, &title, Some(&case_id.to_string()), &report)?;

    let repo = SqliteReportRepository::new(&conn);
    let report_id = repo.insert(&kryvora_db::repo::NewReport {
        case_id: Some(case_id),
        job_id: None,
        kind: ReportKind::Recovery,
        path: generated.path.display().to_string(),
        sha256: generated.sha256.clone(),
        byte_size: generated.byte_size,
    })?;

    println!("=== KRYVORA report ===");
    println!("case:              {case_id}");
    println!("report id:         {report_id}");
    println!("path:              {}", generated.path.display());
    println!("byte size:         {}", generated.byte_size);
    println!("sha256:            {}", generated.sha256);
    println!("artifacts in file: {}", report.results.len());
    println!("audit events seen: {audit_count}");
    println!();

    Ok(())
}

fn collect_results_for_case(
    conn: &Connection,
    case_id: &kryvora_core::CaseId,
) -> kryvora_core::Result<Vec<RecoveryResult>> {
    let ev_repo = SqliteEvidenceRepository::new(conn);
    let evidence_items = ev_repo.list_by_case(case_id)?;
    let rr_repo = SqliteRecoveryResultRepository::new(conn);

    let mut out: Vec<RecoveryResult> = Vec::new();
    for ev in evidence_items {
        let rows = rr_repo.list_by_evidence(&ev.id)?;
        for row in rows {
            let confidence_reasons: Vec<ConfidenceReason> =
                serde_json::from_str(&row.confidence_reasons).unwrap_or_default();
            let level = parse_confidence(&row.confidence_level);
            let facts: serde_json::Value =
                serde_json::from_str(&row.validation_facts).unwrap_or(serde_json::json!({}));

            let category = match row.category.as_str() {
                "image" => ArtifactCategory::Image,
                "video" => ArtifactCategory::Video,
                "audio" => ArtifactCategory::Audio,
                "document" => ArtifactCategory::Document,
                "archive" => ArtifactCategory::Archive,
                "database" => ArtifactCategory::Database,
                "application_artifact" => ArtifactCategory::ApplicationArtifact,
                _ => ArtifactCategory::Unknown,
            };

            let validation_state = match row.validation_state.as_str() {
                "valid" => ValidationState::Valid,
                "partial" => ValidationState::Partial,
                "invalid" => ValidationState::Invalid,
                "inconclusive" => ValidationState::Inconclusive,
                _ => ValidationState::Unknown,
            };

            let recovery_method = match row.recovery_method.as_str() {
                "fragment_reassembly" => RecoveryMethod::FragmentReassembly,
                "header_only" => RecoveryMethod::HeaderOnly,
                _ => RecoveryMethod::SignatureCarving,
            };

            let reconstruction_state = match row.reconstruction_state.as_str() {
                "reconstructed" => ReconstructionState::Reconstructed,
                "partially_reconstructed" => ReconstructionState::PartiallyReconstructed,
                "uncertain" => ReconstructionState::Uncertain,
                "invalid" => ReconstructionState::Invalid,
                _ => ReconstructionState::Contiguous,
            };

            out.push(RecoveryResult {
                id: row.id,
                evidence_id: Some(ev.id),
                source_offset: row.source_offset,
                source_length: row.source_length,
                detected_type: row.detected_type,
                category,
                validation_state,
                confidence: ConfidenceAssessment {
                    level,
                    reasons: confidence_reasons,
                },
                recovery_method,
                reconstruction_state,
                artifact_sha256: row.artifact_sha256,
                created_at: OffsetDateTime::now_utc(),
                job_id: row.job_id,
                provenance_id: row.provenance_id,
                report_id: None,
                validation_facts: facts,
            });
        }
    }

    Ok(out)
}

fn audit_event_count(conn: &Connection) -> kryvora_core::Result<u64> {
    let repo = SqliteAuditEventRepository::new(conn);
    repo.count()
}

fn parse_case_id(s: &str) -> Option<uuid::Uuid> {
    if let Ok(u) = uuid::Uuid::parse_str(s) {
        return Some(u);
    }
    let (_, tail) = s.rsplit_once('-')?;
    if tail.len() == 32 {
        uuid::Uuid::parse_str(tail).ok()
    } else {
        None
    }
}

fn parse_confidence(s: &str) -> Confidence {
    match s {
        "high" => Confidence::High,
        "moderate" => Confidence::Moderate,
        "low" => Confidence::Low,
        _ => Confidence::Uncertain,
    }
}
