#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration tests for the job runner against a real database.

use kryvora_audit::{count, verify_chain, ChainStatus};
use kryvora_core::Error;
use kryvora_db::repo::{JobRepository, SqliteJobRepository};
use kryvora_db::{apply_migrations, open};
use kryvora_jobs::{run_job, CancelToken, JobOutcome, JobRequest};
use rusqlite::Connection;

fn fresh_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("k.sqlite");
    let mut conn = open(&path).unwrap();
    apply_migrations(&mut conn).unwrap();
    (dir, conn)
}

fn request(job_type: &str) -> JobRequest {
    JobRequest {
        job_type: job_type.to_string(),
        case_id: None,
        evidence_id: None,
        configuration: None,
        actor: Some("tester".into()),
    }
}

#[test]
fn successful_job_writes_state_and_audit() {
    let (_dir, conn) = fresh_db();

    let outcome = run_job(
        &conn,
        request("hash"),
        CancelToken::new(),
        Box::new(|_ctx| Ok(())),
    )
    .unwrap();

    let job_id = match outcome {
        JobOutcome::Succeeded { job_id } => job_id,
        other => panic!("expected Succeeded, got {other:?}"),
    };

    let repo = SqliteJobRepository::new(&conn);
    let row = repo.get(&job_id).unwrap().expect("job must exist");
    assert_eq!(row.state, "succeeded");
    assert!(row.started_at.is_some());
    assert!(row.completed_at.is_some());

    // Audit chain should contain: JobCreated, JobStarted, JobCompleted.
    assert_eq!(count(&conn).unwrap(), 3);
    assert_eq!(
        verify_chain(&conn).unwrap(),
        ChainStatus::Intact { length: 3 }
    );
}

#[test]
fn failed_job_records_error_and_audits() {
    let (_dir, conn) = fresh_db();

    let outcome = run_job(
        &conn,
        request("hash"),
        CancelToken::new(),
        Box::new(|_ctx| Err(Error::Internal("body exploded".into()))),
    )
    .unwrap();

    let job_id = *outcome.job_id();
    assert!(matches!(outcome, JobOutcome::Failed { .. }));

    let repo = SqliteJobRepository::new(&conn);
    let row = repo.get(&job_id).unwrap().unwrap();
    assert_eq!(row.state, "failed");
    assert!(row
        .error_message
        .as_deref()
        .unwrap_or("")
        .contains("body exploded"));

    // Audit chain: JobCreated, JobStarted, JobFailed. Chain intact.
    assert_eq!(
        verify_chain(&conn).unwrap(),
        ChainStatus::Intact { length: 3 }
    );
}

#[test]
fn cancelled_job_records_state_and_audits() {
    let (_dir, conn) = fresh_db();

    let token = CancelToken::new();
    let token_for_body = token.clone();

    let outcome = run_job(
        &conn,
        request("hash"),
        token,
        Box::new(move |_ctx| {
            token_for_body.cancel();
            Ok(())
        }),
    )
    .unwrap();

    let job_id = *outcome.job_id();
    assert!(matches!(outcome, JobOutcome::Cancelled { .. }));

    let repo = SqliteJobRepository::new(&conn);
    let row = repo.get(&job_id).unwrap().unwrap();
    assert_eq!(row.state, "cancelled");

    // Audit chain: JobCreated, JobStarted, JobFailed(cancelled).
    assert_eq!(
        verify_chain(&conn).unwrap(),
        ChainStatus::Intact { length: 3 }
    );
}

#[test]
fn progress_is_persisted() {
    let (_dir, conn) = fresh_db();

    let outcome = run_job(
        &conn,
        request("hash"),
        CancelToken::new(),
        Box::new(|ctx| {
            ctx.report_progress(0.25);
            ctx.report_progress(0.75);
            Ok(())
        }),
    )
    .unwrap();

    let job_id = *outcome.job_id();
    let repo = SqliteJobRepository::new(&conn);
    let row = repo.get(&job_id).unwrap().unwrap();
    assert!((row.progress - 0.75).abs() < 1e-9);
}

#[test]
fn partial_job_persists_reason_and_audit_terminal_state() {
    let (_dir, conn) = fresh_db();
    let outcome = run_job(
        &conn,
        request("scan"),
        CancelToken::new(),
        Box::new(|ctx| {
            ctx.mark_partial("candidate limit reached");
            Ok(())
        }),
    )
    .unwrap();

    let (job_id, reason) = match outcome {
        JobOutcome::Partial { job_id, reason } => (job_id, reason),
        other => panic!("expected Partial, got {other:?}"),
    };
    assert_eq!(reason, "candidate limit reached");
    let row = SqliteJobRepository::new(&conn).get(&job_id).unwrap().unwrap();
    assert_eq!(row.state, "partial");
    assert_eq!(row.error_message.as_deref(), Some("candidate limit reached"));
    assert_eq!(
        verify_chain(&conn).unwrap(),
        ChainStatus::Intact { length: 3 }
    );
}
