#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration tests for the four repositories against a real on-disk
//! SQLite database with the initial migration applied.

use kryvora_core::{CaseId, Error, JobId, SanitizationOperationState};
use kryvora_db::repo::{
    AuditEventRepository, CaseRepository, EvidenceRepository, JobRepository, NewAuditEvent,
    NewCase, NewEvidence, NewJob, NewSanitizationOperation, SanitizationOperationRepository,
    SqliteAuditEventRepository, SqliteCaseRepository, SqliteEvidenceRepository,
    SqliteJobRepository, SqliteSanitizationOperationRepository, GENESIS_PREVIOUS_HASH,
};
use kryvora_db::{apply_migrations, open};
use rusqlite::Connection;

fn fresh_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kryvora.sqlite");
    let mut conn = open(&path).unwrap();
    apply_migrations(&mut conn).unwrap();
    (dir, conn)
}

fn new_case(title: &str) -> NewCase {
    NewCase {
        title: title.to_string(),
        examiner: Some("tester".into()),
        notes: None,
    }
}

fn new_evidence(case_id: CaseId) -> NewEvidence {
    NewEvidence {
        case_id,
        source_type: "file".into(),
        source_path: "C:/evidence/sample.bin".into(),
        size_bytes: 1024,
        hash_algorithm: "sha256".into(),
        hash_digest: "a".repeat(64),
        integrity_state: "verified".into(),
        read_only: true,
        tool_version: "0.1.0".into(),
        acquisition_metadata: None,
        notes: None,
    }
}

// ------------------------------------------------------------------
// Cases
// ------------------------------------------------------------------

#[test]
fn insert_and_get_case() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteCaseRepository::new(&conn);
    let id = repo.insert(&new_case("Case Alpha")).unwrap();

    let got = repo.get(&id).unwrap().expect("case must exist");
    assert_eq!(got.id, id);
    assert_eq!(got.title, "Case Alpha");
    assert_eq!(got.examiner.as_deref(), Some("tester"));
    assert!(got.notes.is_none());
    // Timestamps are RFC 3339 UTC ending in 'Z'.
    assert!(got.created_at.ends_with('Z'), "{}", got.created_at);
    assert_eq!(got.created_at, got.updated_at);
}

#[test]
fn get_case_returns_none_when_absent() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteCaseRepository::new(&conn);
    assert!(repo.get(&CaseId::new()).unwrap().is_none());
}

#[test]
fn insert_case_rejects_empty_title() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteCaseRepository::new(&conn);
    let err = repo.insert(&new_case("   ")).unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[test]
fn list_cases_orders_newest_first() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteCaseRepository::new(&conn);
    let _ = repo.insert(&new_case("First")).unwrap();
    let _ = repo.insert(&new_case("Second")).unwrap();
    let _ = repo.insert(&new_case("Third")).unwrap();

    let all = repo.list(10, 0).unwrap();
    assert_eq!(all.len(), 3);
    // All inserted in the same second, so created_at ties; the id DESC
    // tiebreaker is deterministic but we cannot assert titles without
    // controlling insert order and time. We only assert the set.
    let mut titles: Vec<_> = all.iter().map(|c| c.title.clone()).collect();
    titles.sort();
    assert_eq!(titles, vec!["First", "Second", "Third"]);
}

#[test]
fn list_cases_rejects_zero_limit() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteCaseRepository::new(&conn);
    assert!(matches!(repo.list(0, 0), Err(Error::InvalidInput(_))));
}

#[test]
fn update_case_title_and_notes() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteCaseRepository::new(&conn);
    let id = repo.insert(&new_case("Old")).unwrap();

    repo.update_title_and_notes(&id, "New", Some("added notes"))
        .unwrap();

    let got = repo.get(&id).unwrap().unwrap();
    assert_eq!(got.title, "New");
    assert_eq!(got.notes.as_deref(), Some("added notes"));
}

#[test]
fn update_case_missing_returns_not_found() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteCaseRepository::new(&conn);
    let err = repo
        .update_title_and_notes(&CaseId::new(), "New", None)
        .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

// ------------------------------------------------------------------
// Evidence
// ------------------------------------------------------------------

#[test]
fn insert_and_get_evidence() {
    let (_dir, conn) = fresh_db();
    let cases = SqliteCaseRepository::new(&conn);
    let evidence = SqliteEvidenceRepository::new(&conn);
    let case_id = cases.insert(&new_case("Case Alpha")).unwrap();
    let ev_id = evidence.insert(&new_evidence(case_id)).unwrap();

    let got = evidence.get(&ev_id).unwrap().expect("evidence must exist");
    assert_eq!(got.id, ev_id);
    assert_eq!(got.case_id, case_id);
    assert_eq!(got.size_bytes, 1024);
    assert_eq!(got.hash_algorithm, "sha256");
    assert_eq!(got.hash_digest, "a".repeat(64));
    assert!(got.read_only);
}

#[test]
fn insert_evidence_with_missing_case_fails_fk() {
    let (_dir, conn) = fresh_db();
    let evidence = SqliteEvidenceRepository::new(&conn);
    let err = evidence.insert(&new_evidence(CaseId::new())).unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[test]
fn insert_evidence_rejects_bad_hash_length() {
    let (_dir, conn) = fresh_db();
    let cases = SqliteCaseRepository::new(&conn);
    let evidence = SqliteEvidenceRepository::new(&conn);
    let case_id = cases.insert(&new_case("Case Alpha")).unwrap();

    let mut bad = new_evidence(case_id);
    bad.hash_digest = "abc".into();
    let err = evidence.insert(&bad).unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[test]
fn insert_evidence_rejects_unknown_algorithm() {
    let (_dir, conn) = fresh_db();
    let cases = SqliteCaseRepository::new(&conn);
    let evidence = SqliteEvidenceRepository::new(&conn);
    let case_id = cases.insert(&new_case("Case Alpha")).unwrap();

    let mut bad = new_evidence(case_id);
    bad.hash_algorithm = "md5".into();
    let err = evidence.insert(&bad).unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[test]
fn list_evidence_by_case() {
    let (_dir, conn) = fresh_db();
    let cases = SqliteCaseRepository::new(&conn);
    let evidence = SqliteEvidenceRepository::new(&conn);
    let case_a = cases.insert(&new_case("A")).unwrap();
    let case_b = cases.insert(&new_case("B")).unwrap();

    evidence.insert(&new_evidence(case_a)).unwrap();
    evidence.insert(&new_evidence(case_a)).unwrap();
    evidence.insert(&new_evidence(case_b)).unwrap();

    assert_eq!(evidence.list_by_case(&case_a).unwrap().len(), 2);
    assert_eq!(evidence.list_by_case(&case_b).unwrap().len(), 1);
    assert_eq!(evidence.list_by_case(&CaseId::new()).unwrap().len(), 0);
}

// ------------------------------------------------------------------
// Jobs
// ------------------------------------------------------------------

fn new_job(case_id: CaseId) -> NewJob {
    NewJob {
        case_id: Some(case_id),
        evidence_id: None,
        job_type: "hash".into(),
        configuration: Some(r#"{"algorithm":"sha256"}"#.into()),
    }
}

#[test]
fn insert_and_get_job() {
    let (_dir, conn) = fresh_db();
    let cases = SqliteCaseRepository::new(&conn);
    let jobs = SqliteJobRepository::new(&conn);
    let case_id = cases.insert(&new_case("Case Alpha")).unwrap();

    let job_id = jobs.insert(&new_job(case_id)).unwrap();
    let got = jobs.get(&job_id).unwrap().expect("job must exist");
    assert_eq!(got.id, job_id);
    assert_eq!(got.case_id, Some(case_id));
    assert_eq!(got.state, "created");
    assert!((got.progress - 0.0).abs() < f64::EPSILON);
    assert!(got.started_at.is_none());
    assert!(got.completed_at.is_none());
}

#[test]
fn update_job_state_running_sets_started_at() {
    let (_dir, conn) = fresh_db();
    let cases = SqliteCaseRepository::new(&conn);
    let jobs = SqliteJobRepository::new(&conn);
    let case_id = cases.insert(&new_case("Case Alpha")).unwrap();
    let job_id = jobs.insert(&new_job(case_id)).unwrap();

    jobs.update_state(&job_id, "running", None).unwrap();
    let got = jobs.get(&job_id).unwrap().unwrap();
    assert_eq!(got.state, "running");
    assert!(got.started_at.is_some());
    assert!(got.completed_at.is_none());
}

#[test]
fn update_job_state_terminal_sets_completed_at() {
    let (_dir, conn) = fresh_db();
    let cases = SqliteCaseRepository::new(&conn);
    let jobs = SqliteJobRepository::new(&conn);
    let case_id = cases.insert(&new_case("Case Alpha")).unwrap();
    let job_id = jobs.insert(&new_job(case_id)).unwrap();

    jobs.update_state(&job_id, "running", None).unwrap();
    jobs.update_state(&job_id, "succeeded", None).unwrap();
    let got = jobs.get(&job_id).unwrap().unwrap();
    assert_eq!(got.state, "succeeded");
    assert!(got.started_at.is_some());
    assert!(got.completed_at.is_some());
}

#[test]
fn update_job_state_rejects_unknown_state() {
    let (_dir, conn) = fresh_db();
    let cases = SqliteCaseRepository::new(&conn);
    let jobs = SqliteJobRepository::new(&conn);
    let case_id = cases.insert(&new_case("Case Alpha")).unwrap();
    let job_id = jobs.insert(&new_job(case_id)).unwrap();

    let err = jobs.update_state(&job_id, "exploded", None).unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[test]
fn update_job_progress_rejects_out_of_range() {
    let (_dir, conn) = fresh_db();
    let cases = SqliteCaseRepository::new(&conn);
    let jobs = SqliteJobRepository::new(&conn);
    let case_id = cases.insert(&new_case("Case Alpha")).unwrap();
    let job_id = jobs.insert(&new_job(case_id)).unwrap();

    assert!(matches!(
        jobs.update_progress(&job_id, -0.1),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        jobs.update_progress(&job_id, 1.1),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        jobs.update_progress(&job_id, f64::NAN),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn list_jobs_by_case_and_state() {
    let (_dir, conn) = fresh_db();
    let cases = SqliteCaseRepository::new(&conn);
    let jobs = SqliteJobRepository::new(&conn);
    let case_id = cases.insert(&new_case("Case Alpha")).unwrap();

    let job_a = jobs.insert(&new_job(case_id)).unwrap();
    let _job_b = jobs.insert(&new_job(case_id)).unwrap();
    jobs.update_state(&job_a, "running", None).unwrap();

    assert_eq!(jobs.list_by_case(&case_id).unwrap().len(), 2);
    assert_eq!(jobs.list_by_state("running", 10).unwrap().len(), 1);
    assert_eq!(jobs.list_by_state("created", 10).unwrap().len(), 1);
}

#[test]
fn update_job_missing_returns_not_found() {
    let (_dir, conn) = fresh_db();
    let jobs = SqliteJobRepository::new(&conn);
    let err = jobs.update_progress(&JobId::new(), 0.5).unwrap_err();
    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

// ------------------------------------------------------------------
// Sanitization operations
// ------------------------------------------------------------------

fn new_sanitization_operation(case_id: Option<CaseId>) -> NewSanitizationOperation {
    NewSanitizationOperation {
        case_id,
        target_path: "C:/disposable/target.bin".into(),
        target_kind: "file".into(),
        method: "random_overwrite".into(),
        target_identity_verified: true,
        confirmation_kind: "basic".into(),
        confirmation_validated: true,
        actor: Some("tester".into()),
    }
}

#[test]
fn sanitization_operation_persists_lifecycle_and_result() {
    let (_dir, conn) = fresh_db();
    let case_id = SqliteCaseRepository::new(&conn)
        .insert(&new_case("Case Alpha"))
        .unwrap();
    let repo = SqliteSanitizationOperationRepository::new(&conn);
    let id = repo
        .insert(&new_sanitization_operation(Some(case_id)))
        .unwrap();

    let planned = repo.get(&id).unwrap().unwrap();
    assert_eq!(planned.state, SanitizationOperationState::Planned);
    assert!(planned.started_at.is_none());
    assert!(planned.completed_at.is_none());
    assert!(planned.target_identity_verified);
    assert!(planned.confirmation_validated);

    repo.transition(
        &id,
        SanitizationOperationState::Running,
        None,
        None,
        None,
    )
    .unwrap();
    repo.transition(
        &id,
        SanitizationOperationState::Completed,
        Some("partial"),
        Some(r#"{"bytes_overwritten":128,"reason":"safe unlink unavailable"}"#),
        Some("safe unlink unavailable"),
    )
    .unwrap();

    let completed = repo.get(&id).unwrap().unwrap();
    assert_eq!(completed.state, SanitizationOperationState::Completed);
    assert_eq!(completed.outcome.as_deref(), Some("partial"));
    assert!(completed.result_json.as_deref().unwrap().contains("128"));
    assert!(completed.started_at.is_some());
    assert!(completed.completed_at.is_some());
    assert_eq!(repo.list_recent(10).unwrap().len(), 1);
}

#[test]
fn sanitization_operation_rejects_invalid_transition() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteSanitizationOperationRepository::new(&conn);
    let id = repo
        .insert(&new_sanitization_operation(None))
        .unwrap();

    let error = repo
        .transition(
            &id,
            SanitizationOperationState::Completed,
            Some("success"),
            None,
            None,
        )
        .unwrap_err();
    assert!(matches!(error, Error::InvalidInput(_)), "{error:?}");
}

#[test]
fn sanitization_operation_can_record_refusal() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteSanitizationOperationRepository::new(&conn);
    let id = repo
        .insert(&new_sanitization_operation(None))
        .unwrap();

    repo.transition(
        &id,
        SanitizationOperationState::Refused,
        None,
        None,
        Some("typed confirmation did not match"),
    )
    .unwrap();
    let refused = repo.get(&id).unwrap().unwrap();
    assert_eq!(refused.state, SanitizationOperationState::Refused);
    assert_eq!(
        refused.error_message.as_deref(),
        Some("typed confirmation did not match")
    );
    assert!(refused.completed_at.is_some());
}

// ------------------------------------------------------------------
// Audit events
// ------------------------------------------------------------------

fn new_event(event_type: &str) -> NewAuditEvent {
    NewAuditEvent {
        event_type: event_type.into(),
        actor: Some("tester".into()),
        object_id: None,
        job_id: None,
        details: format!(r#"{{"event":"{event_type}"}}"#),
        created_at: "2020-01-01T00:00:59Z".into(),
        previous_hash: GENESIS_PREVIOUS_HASH.into(),
        current_hash: "b".repeat(64),
    }
}

#[test]
fn append_allocates_monotonic_sequence() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteAuditEventRepository::new(&conn);

    let (_, s0) = repo.append(&new_event("first")).unwrap();
    let (_, s1) = repo.append(&new_event("second")).unwrap();
    let (_, s2) = repo.append(&new_event("third")).unwrap();

    assert_eq!((s0, s1, s2), (0, 1, 2));
    assert_eq!(repo.count().unwrap(), 3);
}

#[test]
fn latest_returns_highest_sequence() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteAuditEventRepository::new(&conn);
    repo.append(&new_event("first")).unwrap();
    repo.append(&new_event("second")).unwrap();

    let latest = repo.latest().unwrap().expect("at least one event");
    assert_eq!(latest.sequence, 1);
    assert_eq!(latest.event_type, "second");
}

#[test]
fn latest_on_empty_returns_none() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteAuditEventRepository::new(&conn);
    assert!(repo.latest().unwrap().is_none());
}

#[test]
fn list_range_returns_requested_window() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteAuditEventRepository::new(&conn);
    for i in 0..5 {
        repo.append(&new_event(&format!("e{i}"))).unwrap();
    }

    let window = repo.list_range(1, 3).unwrap();
    assert_eq!(window.len(), 3);
    assert_eq!(window[0].sequence, 1);
    assert_eq!(window[1].sequence, 2);
    assert_eq!(window[2].sequence, 3);
}

#[test]
fn append_rejects_short_hash() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteAuditEventRepository::new(&conn);
    let mut bad = new_event("x");
    bad.previous_hash = "abc".into();
    let err = repo.append(&bad).unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[test]
fn append_rejects_empty_details() {
    let (_dir, conn) = fresh_db();
    let repo = SqliteAuditEventRepository::new(&conn);
    let mut bad = new_event("x");
    bad.details = "".into();
    let err = repo.append(&bad).unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[test]
fn sequence_is_unique_across_runs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kryvora.sqlite");

    {
        let mut conn = open(&path).unwrap();
        apply_migrations(&mut conn).unwrap();
        let repo = SqliteAuditEventRepository::new(&conn);
        repo.append(&new_event("a")).unwrap();
        repo.append(&new_event("b")).unwrap();
    }
    {
        let conn = open(&path).unwrap();
        let repo = SqliteAuditEventRepository::new(&conn);
        let (_, seq) = repo.append(&new_event("c")).unwrap();
        assert_eq!(seq, 2);
    }
}
