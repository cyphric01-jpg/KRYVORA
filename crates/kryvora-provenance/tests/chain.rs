#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration tests for chain persistence.

use kryvora_core::{Confidence, ReconstructionState, RecoveryResultId, ValidationState};
use kryvora_db::repo::{
    CaseRepository, EvidenceRepository, NewCase, NewEvidence, RecoveryResultRepository,
    SqliteCaseRepository, SqliteEvidenceRepository, SqliteRecoveryResultRepository,
};
use kryvora_db::{apply_migrations, open};
use kryvora_provenance::chain::candidate_object_id;
use kryvora_provenance::{
    ensure_evidence_root, persist_recovery_result, ChainInputs, ProvenanceGraph,
};
use kryvora_recovery::{
    ArtifactCategory, ConfidenceAssessment, ConfidenceReason, ConfidenceSignal, RecoveryMethod,
    RecoveryResult,
};
use rusqlite::Connection;
use time::OffsetDateTime;

fn fresh_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("c.sqlite");
    let mut conn = open(&path).unwrap();
    apply_migrations(&mut conn).unwrap();
    (dir, conn)
}

fn make_case_and_evidence(conn: &Connection) -> (kryvora_core::CaseId, kryvora_core::EvidenceId) {
    let case_id = SqliteCaseRepository::new(conn)
        .insert(&NewCase {
            title: "case".into(),
            examiner: None,
            notes: None,
        })
        .unwrap();
    let evidence_id = SqliteEvidenceRepository::new(conn)
        .insert(&NewEvidence {
            case_id,
            source_type: "file".into(),
            source_path: "C:/evidence/x.bin".into(),
            size_bytes: 4096,
            hash_algorithm: "sha256".into(),
            hash_digest: "a".repeat(64),
            integrity_state: "verified".into(),
            read_only: true,
            tool_version: "0.1.0".into(),
            acquisition_metadata: None,
            notes: None,
        })
        .unwrap();
    (case_id, evidence_id)
}

fn dummy_recovery_result() -> RecoveryResult {
    RecoveryResult {
        id: RecoveryResultId::new(),
        evidence_id: None,
        source_offset: 100,
        source_length: 200,
        detected_type: "jpeg".into(),
        category: ArtifactCategory::Image,
        validation_state: ValidationState::Valid,
        confidence: ConfidenceAssessment::from_reasons(vec![
            ConfidenceReason::new(ConfidenceSignal::SignatureValid, true),
            ConfidenceReason::new(ConfidenceSignal::StructuralValid, true),
            ConfidenceReason::new(ConfidenceSignal::FormatValidated, true),
            ConfidenceReason::new(ConfidenceSignal::SizeConsistent, true),
            ConfidenceReason::new(ConfidenceSignal::FragmentConsistent, true),
            ConfidenceReason::new(ConfidenceSignal::SourceTraceable, true),
            ConfidenceReason::new(ConfidenceSignal::ReconstructionCertain, true),
        ]),
        recovery_method: RecoveryMethod::SignatureCarving,
        reconstruction_state: ReconstructionState::Contiguous,
        artifact_sha256: "b".repeat(64),
        created_at: OffsetDateTime::now_utc(),
        job_id: None,
        provenance_id: None,
        report_id: None,
        validation_facts: serde_json::json!({ "width": 100, "height": 200 }),
    }
}

#[test]
fn persist_creates_full_chain() {
    let (_dir, conn) = fresh_db();
    let (case_id, evidence_id) = make_case_and_evidence(&conn);

    let result = dummy_recovery_result();
    let inputs = ChainInputs {
        case_id,
        evidence_id,
        job_id: None,
        candidate_object_id: candidate_object_id(result.source_offset, result.source_length),
        actor_note: Some("auto".into()),
        artifact_path: None,
    };

    let artifact_node = persist_recovery_result(&conn, &inputs, &result).unwrap();

    // Walk the chain.
    let graph = ProvenanceGraph::new(&conn);
    let path = graph.path_to_root(&artifact_node).unwrap();
    assert_eq!(path.elements.len(), 3);
    assert_eq!(path.root().unwrap().object_id, evidence_id.to_string());

    // Recovery result row exists and is linked.
    let repo = SqliteRecoveryResultRepository::new(&conn);
    let rows = repo.list_by_evidence(&evidence_id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, result.id);
    assert_eq!(rows[0].provenance_id, Some(artifact_node));
    assert_eq!(rows[0].detected_type, "jpeg");
    assert_eq!(rows[0].category, "image");
    assert_eq!(rows[0].confidence_level, "high");

    // Confirm the level is actually High.
    assert_eq!(result.confidence.level, Confidence::High);
}

#[test]
fn persist_reuses_evidence_root_across_results() {
    let (_dir, conn) = fresh_db();
    let (case_id, evidence_id) = make_case_and_evidence(&conn);

    let inputs_a = ChainInputs {
        case_id,
        evidence_id,
        job_id: None,
        candidate_object_id: candidate_object_id(100, 200),
        actor_note: None,
        artifact_path: None,
    };
    let inputs_b = ChainInputs {
        case_id,
        evidence_id,
        job_id: None,
        candidate_object_id: candidate_object_id(500, 300),
        actor_note: None,
        artifact_path: None,
    };

    let r1 = dummy_recovery_result();
    let r2 = dummy_recovery_result();

    let _ = persist_recovery_result(&conn, &inputs_a, &r1).unwrap();
    let _ = persist_recovery_result(&conn, &inputs_b, &r2).unwrap();

    // There must be exactly one evidence root.
    use kryvora_db::repo::{ProvenanceRepository, SqliteProvenanceRepository};
    let repo = SqliteProvenanceRepository::new(&conn);
    let roots = repo
        .find_by_object(&case_id, &evidence_id.to_string())
        .unwrap();
    assert_eq!(roots.len(), 1);

    // And two recovery results.
    let rr = SqliteRecoveryResultRepository::new(&conn);
    assert_eq!(rr.count_by_evidence(&evidence_id).unwrap(), 2);
}

#[test]
fn ensure_evidence_root_reuses_existing_root() {
    let (_dir, conn) = fresh_db();
    let (case_id, evidence_id) = make_case_and_evidence(&conn);

    let first = ensure_evidence_root(&conn, &case_id, &evidence_id).unwrap();
    let second = ensure_evidence_root(&conn, &case_id, &evidence_id).unwrap();

    assert_eq!(first, second);
    use kryvora_db::repo::{ProvenanceRepository, SqliteProvenanceRepository};
    let roots = SqliteProvenanceRepository::new(&conn)
        .find_by_object(&case_id, &evidence_id.to_string())
        .unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].id, first);
}
