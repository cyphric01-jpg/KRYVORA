//! The job runner.

use crate::cancel::CancelToken;
use kryvora_audit::{append, EventDraft, EventType};
use kryvora_core::{Error, JobId, Result};
use kryvora_db::repo::{JobRepository, NewJob, SqliteJobRepository};
use rusqlite::Connection;
use serde_json::json;
use std::cell::RefCell;
use thiserror::Error;

/// Errors from the job runner above and beyond [`kryvora_core::Error`].
#[derive(Debug, Error)]
pub enum JobRunnerError {
    #[error("job body returned an error: {0}")]
    BodyFailed(String),
}

/// A request to run a job.
#[derive(Debug, Clone)]
pub struct JobRequest {
    pub job_type: String,
    pub case_id: Option<kryvora_core::CaseId>,
    pub evidence_id: Option<kryvora_core::EvidenceId>,
    pub configuration: Option<serde_json::Value>,
    pub actor: Option<String>,
}

/// What the caller's job body receives: a handle to report progress
/// and to query cancellation.
///
/// The lifetime `'a` is the lifetime of the connection that
/// [`run_job`] was called with. The context is not `Send` and does not
/// need to be: `run_job` runs the body on the calling thread.
#[derive(Debug)]
pub struct JobContext<'a> {
    pub job_id: JobId,
    cancel: CancelToken,
    conn: &'a Connection,
    partial_reason: RefCell<Option<String>>,
}

impl JobContext<'_> {
    #[must_use]
    pub fn job_id(&self) -> &JobId {
        &self.job_id
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// Mark a successful body result as partial while preserving its output.
    pub fn mark_partial(&self, reason: impl Into<String>) {
        *self.partial_reason.borrow_mut() = Some(reason.into());
    }

    /// Report progress in 0.0..=1.0. Best-effort: an error updating
    /// the database is logged to stderr but does not abort the job.
    /// Progress is a UI convenience, not a correctness invariant.
    pub fn report_progress(&self, progress: f64) {
        let progress = progress.clamp(0.0, 1.0);
        let repo = SqliteJobRepository::new(self.conn);
        if let Err(e) = repo.update_progress(&self.job_id, progress) {
            eprintln!("warning: could not update job progress: {e}");
        }
    }

    /// Store an opaque checkpoint blob.
    ///
    /// # Errors
    ///
    /// Propagates any repository error.
    pub fn set_checkpoint(&self, checkpoint: &str) -> Result<()> {
        let repo = SqliteJobRepository::new(self.conn);
        repo.update_checkpoint(&self.job_id, Some(checkpoint))
    }

    /// Access the underlying connection.
    ///
    /// The job body may use this for audit events, repository calls,
    /// or any other operation that needs to run against the same
    /// database the job itself is recorded in. The lifetime is tied
    /// to the `run_job` call.
    #[must_use]
    pub fn conn(&self) -> &Connection {
        self.conn
    }
}

/// The caller's job body: takes a [`JobContext`] and returns a result.
pub type JobBody<'a> = dyn FnOnce(&JobContext<'a>) -> Result<()> + 'a;

/// The result of running a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobOutcome {
    Succeeded { job_id: JobId },
    Partial { job_id: JobId, reason: String },
    Cancelled { job_id: JobId },
    Failed { job_id: JobId, message: String },
}

impl JobOutcome {
    #[must_use]
    pub fn job_id(&self) -> &JobId {
        match self {
            Self::Succeeded { job_id }
            | Self::Partial { job_id, .. }
            | Self::Cancelled { job_id }
            | Self::Failed { job_id, .. } => job_id,
        }
    }
}

/// Run a job to completion, a cancel request, or an error.
///
/// Steps:
/// 1. Insert a row in `jobs` with state `created`.
/// 2. Append `JobCreated` to the audit chain.
/// 3. Transition to `running`, set `started_at`.
/// 4. Append `JobStarted`.
/// 5. Invoke `body`. The body may call `ctx.report_progress(..)` and
///    `ctx.set_checkpoint(..)`, and must poll `ctx.is_cancelled()` at
///    its own checkpoints.
/// 6. On `Ok`, transition to `succeeded`, append `JobCompleted`.
///    On cancellation requested, transition to `cancelled`. On `Err`,
///    transition to `failed`, append `JobFailed`.
///
/// # Errors
///
/// Returns the initial database error if creating the job row or
/// appending the first audit event fails. Once the body has started,
/// its own error is folded into the returned `JobOutcome::Failed`.
pub fn run_job<'a>(
    conn: &'a Connection,
    request: JobRequest,
    cancel: CancelToken,
    body: Box<JobBody<'a>>,
) -> Result<JobOutcome> {
    let repo = SqliteJobRepository::new(conn);

    let config_json = match &request.configuration {
        None => None,
        Some(v) => Some(serde_json::to_string(v).map_err(|e| {
            Error::InvalidInput(format!("job configuration is not serializable: {e}"))
        })?),
    };

    let job_id = repo.insert(&NewJob {
        case_id: request.case_id,
        evidence_id: request.evidence_id,
        job_type: request.job_type.clone(),
        configuration: config_json,
    })?;

    append(
        conn,
        &EventDraft {
            event_type: EventType::JobCreated,
            actor: request.actor.clone(),
            object_id: Some(job_id.to_string()),
            job_id: Some(job_id),
            details: json!({
                "job_type": request.job_type,
                "case_id": request.case_id.map(|c| c.to_string()),
                "evidence_id": request.evidence_id.map(|e| e.to_string()),
            }),
        },
    )?;

    repo.update_state(&job_id, "running", None)?;

    append(
        conn,
        &EventDraft {
            event_type: EventType::JobStarted,
            actor: request.actor.clone(),
            object_id: Some(job_id.to_string()),
            job_id: Some(job_id),
            details: json!({}),
        },
    )?;

    let ctx = JobContext {
        job_id,
        cancel: cancel.clone(),
        conn,
        partial_reason: RefCell::new(None),
    };

    // If the body panics, the job row is left in "running". This is a
    // known limitation: to recover from a panicking body we would need
    // `std::panic::catch_unwind`, which requires the closure to be
    // `UnwindSafe`. Production job bodies are not expected to panic
    // (workspace lints deny `panic!` in production code), and a
    // panicking body indicates a bug that should surface loudly.
    let body_result = body(&ctx);
    let partial_reason = ctx.partial_reason.into_inner();

    if cancel.is_cancelled() {
        repo.update_state(&job_id, "cancelled", None)?;
        append(
            conn,
            &EventDraft {
                event_type: EventType::JobFailed,
                actor: request.actor.clone(),
                object_id: Some(job_id.to_string()),
                job_id: Some(job_id),
                details: json!({ "reason": "cancelled" }),
            },
        )?;
        return Ok(JobOutcome::Cancelled { job_id });
    }

    match body_result {
        Ok(()) if partial_reason.is_some() => {
            let reason = partial_reason.unwrap_or_else(|| "job completed partially".into());
            repo.update_state(&job_id, "partial", Some(&reason))?;
            append(
                conn,
                &EventDraft {
                    event_type: EventType::JobCompleted,
                    actor: request.actor,
                    object_id: Some(job_id.to_string()),
                    job_id: Some(job_id),
                    details: json!({ "state": "partial", "reason": reason }),
                },
            )?;
            Ok(JobOutcome::Partial { job_id, reason })
        }
        Ok(()) => {
            repo.update_state(&job_id, "succeeded", None)?;
            append(
                conn,
                &EventDraft {
                    event_type: EventType::JobCompleted,
                    actor: request.actor.clone(),
                    object_id: Some(job_id.to_string()),
                    job_id: Some(job_id),
                    details: json!({}),
                },
            )?;
            Ok(JobOutcome::Succeeded { job_id })
        }
        Err(e) => {
            let message = e.to_string();
            repo.update_state(&job_id, "failed", Some(&message))?;
            append(
                conn,
                &EventDraft {
                    event_type: EventType::JobFailed,
                    actor: request.actor.clone(),
                    object_id: Some(job_id.to_string()),
                    job_id: Some(job_id),
                    details: json!({ "reason": message }),
                },
            )?;
            Ok(JobOutcome::Failed { job_id, message })
        }
    }
}

#[cfg(test)]
mod tests {
    // The unit-level tests here exercise the `JobContext` shape and
    // the three terminal outcomes. Integration tests with a real
    // database live in `tests/runner.rs`.

    use super::*;
    use kryvora_db::{apply_migrations, open};

    fn fresh() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("k.sqlite");
        let mut conn = open(&path).unwrap();
        apply_migrations(&mut conn).unwrap();
        (dir, conn)
    }

    fn req(job_type: &str) -> JobRequest {
        JobRequest {
            job_type: job_type.to_string(),
            case_id: None,
            evidence_id: None,
            configuration: None,
            actor: None,
        }
    }

    #[test]
    fn context_reports_and_checkpoints() {
        let (_dir, conn) = fresh();

        let token = CancelToken::new();
        let outcome = run_job(
            &conn,
            req("test"),
            token,
            Box::new(|ctx| {
                ctx.report_progress(0.5);
                ctx.set_checkpoint("halfway")?;
                Ok(())
            }),
        )
        .unwrap();

        assert!(matches!(outcome, JobOutcome::Succeeded { .. }));
    }

    #[test]
    fn cancellation_is_reported() {
        let (_dir, conn) = fresh();

        let token = CancelToken::new();
        let token_for_body = token.clone();
        let outcome = run_job(
            &conn,
            req("test"),
            token,
            Box::new(move |_ctx| {
                token_for_body.cancel();
                Ok(())
            }),
        )
        .unwrap();

        assert!(matches!(outcome, JobOutcome::Cancelled { .. }));
    }

    #[test]
    fn body_error_is_reported_as_failed() {
        let (_dir, conn) = fresh();

        let token = CancelToken::new();
        let outcome = run_job(
            &conn,
            req("test"),
            token,
            Box::new(|_ctx| Err(Error::Internal("boom".into()))),
        )
        .unwrap();

        match outcome {
            JobOutcome::Failed { message, .. } => assert!(message.contains("boom")),
            other => panic!("expected Failed, got {other:?}"),
        }
    }
}
