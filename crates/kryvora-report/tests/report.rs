#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration test: generate a report, persist its metadata, then
//! verify the file's hash matches the stored one.

use kryvora_core::{CaseId, Confidence, ReconstructionState, RecoveryResultId, ValidationState};
use kryvora_db::repo::{
    CaseRepository, NewCase, NewReport, ReportKind, ReportRepository, SqliteCaseRepository,
    SqliteReportRepository,
};
use kryvora_db::{apply_migrations, open};
use kryvora_integrity::calculate_hash;
use kryvora_recovery::{
    ArtifactCategory, ConfidenceAssessment, ConfidenceReason, ConfidenceSignal, RecoveryMethod,
    RecoveryReport, RecoveryResult,
};
use kryvora_report::generate_recovery_report;
use rusqlite::Connection;
use std::io::Cursor;
use time::OffsetDateTime;

fn fresh_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("k.sqlite");
    let mut conn = open(&path).unwrap();
    apply_migrations(&mut conn).unwrap();
    (dir, conn)
}

fn make_case(conn: &Connection, title: &str) -> CaseId {
    SqliteCaseRepository::new(conn)
        .insert(&NewCase {
            title: title.into(),
            examiner: None,
            notes: None,
        })
        .unwrap()
}

fn dummy_result() -> RecoveryResult {
    RecoveryResult {
        id: RecoveryResultId::new(),
        evidence_id: None,
        source_offset: 42,
        source_length: 99,
        detected_type: "jpeg".into(),
        category: ArtifactCategory::Image,
        validation_state: ValidationState::Valid,
        confidence: ConfidenceAssessment::from_reasons(vec![
            ConfidenceReason::new(ConfidenceSignal::SignatureValid, true),
            ConfidenceReason::new(ConfidenceSignal::StructuralValid, true),
            ConfidenceReason::new(ConfidenceSignal::FormatValidated, true),
            ConfidenceReason::new(ConfidenceSignal::SizeConsistent, true),
            ConfidenceReason::new(ConfidenceSignal::FragmentConsistent, false),
            ConfidenceReason::new(ConfidenceSignal::SourceTraceable, false),
            ConfidenceReason::new(ConfidenceSignal::ReconstructionCertain, false),
        ]),
        recovery_method: RecoveryMethod::SignatureCarving,
        reconstruction_state: ReconstructionState::Contiguous,
        artifact_sha256: "f".repeat(64),
        created_at: OffsetDateTime::now_utc(),
        job_id: None,
        provenance_id: None,
        report_id: None,
        validation_facts: serde_json::json!({}),
    }
}

#[test]
fn report_is_written_hashed_persisted_and_reverified() {
    let (dir, conn) = fresh_db();
    let case_id = make_case(&conn, "Case A");
    let report_path = dir.path().join("rec.html");

    let report = RecoveryReport {
        case_id: None,
        job_id: None,
        candidates_considered: 1,
        candidates_validated: 1,
        candidates_rejected: 0,
        results: vec![dummy_result()],
    };

    let generated = generate_recovery_report(
        &report_path,
        "Recovery Report",
        Some(&case_id.to_string()),
        &report,
    )
    .unwrap();

    // Persist metadata.
    let repo = SqliteReportRepository::new(&conn);
    let id = repo
        .insert(&NewReport {
            case_id: Some(case_id),
            job_id: None,
            kind: ReportKind::Recovery,
            path: generated.path.display().to_string(),
            sha256: generated.sha256.clone(),
            byte_size: generated.byte_size,
        })
        .unwrap();

    // Reload metadata.
    let row = repo.get(&id).unwrap().expect("report row must exist");
    assert_eq!(row.sha256, generated.sha256);
    assert_eq!(row.byte_size, generated.byte_size);
    assert_eq!(row.kind, ReportKind::Recovery);

    // Recompute from disk.
    let bytes = std::fs::read(&report_path).unwrap();
    let recomputed = calculate_hash(Cursor::new(&bytes)).unwrap();
    assert_eq!(recomputed.digest_hex(), row.sha256);

    // Missing source traceability is insufficient evidence for Low confidence.
    assert_eq!(report.results[0].confidence.level, Confidence::Uncertain);
}

#[test]
fn list_by_case_returns_inserted_reports() {
    let (dir, conn) = fresh_db();
    let case_id = make_case(&conn, "Case B");
    let repo = SqliteReportRepository::new(&conn);

    for i in 0..3 {
        let path = dir.path().join(format!("r{i}.html"));
        let generated = generate_recovery_report(
            &path,
            "R",
            None,
            &RecoveryReport {
                case_id: None,
                job_id: None,
                candidates_considered: 0,
                candidates_validated: 0,
                candidates_rejected: 0,
                results: vec![],
            },
        )
        .unwrap();
        repo.insert(&NewReport {
            case_id: Some(case_id),
            job_id: None,
            kind: ReportKind::Recovery,
            path: generated.path.display().to_string(),
            sha256: generated.sha256,
            byte_size: generated.byte_size,
        })
        .unwrap();
    }

    let all = repo.list_by_case(&case_id).unwrap();
    assert_eq!(all.len(), 3);
}
