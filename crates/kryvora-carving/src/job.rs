//! Run a carving scan as a job.

use crate::candidate::CarvingReport;
use crate::scanner::{scan_source_with_checkpoint, ScanOptions};
use kryvora_audit::{append, EventDraft, EventType};
use kryvora_core::{CaseId, Error, EvidenceId, Result};
use kryvora_jobs::{run_job, CancelToken, JobOutcome, JobRequest};
use rusqlite::Connection;
use serde_json::json;
use std::io::Read;
use std::sync::{Arc, Mutex};

/// Shared slot for the report produced inside the job body and
/// consumed after `run_job` returns.
type ReportSlot = Arc<Mutex<Option<CarvingReport>>>;

/// Run a carving scan as a KRYVORA job.
///
/// The source is opened by the caller and passed as a `Read`. The job
/// reports progress once per scan pass. Cancellation is checked before
/// the scan begins; the scanner reads bounded windows and is fast
/// enough that finer-grained cancellation is deferred to a later
/// batch.
///
/// On success, appends a `RecoveryStarted` audit event with summary
/// counts. On error, the job runner appends `JobFailed`.
///
/// Returns the [`CarvingReport`] on success, `None` if the job was
/// cancelled.
///
/// # Errors
///
/// Any error from the job runner, the audit chain, or the scanner.
pub fn run_carving_job<R: Read + 'static>(
    conn: &Connection,
    source: R,
    options: ScanOptions,
    actor: Option<String>,
) -> Result<Option<CarvingReport>> {
    run_carving_job_for_evidence(conn, source, options, actor, None, None)
}

pub fn run_carving_job_for_evidence<R: Read + 'static>(
    conn: &Connection,
    source: R,
    options: ScanOptions,
    actor: Option<String>,
    case_id: Option<CaseId>,
    evidence_id: Option<EvidenceId>,
) -> Result<Option<CarvingReport>> {
    run_carving_job_for_evidence_with_cancel(
        conn,
        source,
        options,
        actor,
        case_id,
        evidence_id,
        CancelToken::new(),
    )
}

pub fn run_carving_job_for_evidence_with_cancel<R: Read + 'static>(
    conn: &Connection,
    source: R,
    options: ScanOptions,
    actor: Option<String>,
    case_id: Option<CaseId>,
    evidence_id: Option<EvidenceId>,
    cancel: CancelToken,
) -> Result<Option<CarvingReport>> {

    let slot: ReportSlot = Arc::new(Mutex::new(None));
    let slot_for_body = Arc::clone(&slot);
    let source_desc = "carving_source".to_string();

    let outcome = run_job(
        conn,
        JobRequest {
            job_type: "carve".into(),
            case_id,
            evidence_id,
            configuration: Some(json!({
                "window_size": options.window_size,
                "source_size_bytes": options.source_size_bytes,
                "max_scan_bytes": options.max_scan_bytes,
                "max_candidates": options.max_candidates,
                "max_open_headers": options.max_open_headers,
                "source": source_desc,
            })),
            actor: actor.clone(),
        },
        cancel,
        Box::new(move |ctx| {
            let progress_total = options
                .source_size_bytes
                .unwrap_or(options.max_scan_bytes)
                .min(options.max_scan_bytes)
                .max(1) as f64;
            let report = scan_source_with_checkpoint(source, &options, |bytes_scanned| {
                ctx.report_progress(bytes_scanned as f64 / progress_total);
                !ctx.is_cancelled()
            })?;
            ctx.report_progress(1.0);
            if report.scan_limit_reached
                || report.candidate_limit_reached
                || report.headers_dropped_by_limit > 0
            {
                ctx.mark_partial(format!(
                    "scan incomplete: bytes_limit={}, candidate_limit={}, headers_dropped={}",
                    report.scan_limit_reached,
                    report.candidate_limit_reached,
                    report.headers_dropped_by_limit
                ));
            }
            *slot_for_body
                .lock()
                .map_err(|_| Error::Internal("carving slot poisoned".into()))? = Some(report);
            Ok(())
        }),
    )?;

    match outcome {
        JobOutcome::Succeeded { job_id } | JobOutcome::Partial { job_id, .. } => {
            let mut report = slot
                .lock()
                .map_err(|_| Error::Internal("carving slot poisoned".into()))?
                .take()
                .ok_or_else(|| Error::Internal("carving produced no report".into()))?;
            report.job_id = Some(job_id);

            append(
                conn,
                &EventDraft {
                    event_type: EventType::RecoveryStarted,
                    actor,
                    object_id: None,
                    job_id: Some(job_id),
                    details: json!({
                        "bytes_scanned": report.bytes_scanned,
                        "candidates_found": report.candidates_found,
                        "truncated_headers": report.truncated_headers,
                        "scan_limit_reached": report.scan_limit_reached,
                        "candidate_limit_reached": report.candidate_limit_reached,
                    }),
                },
            )?;

            Ok(Some(report))
        }
        JobOutcome::Cancelled { .. } => Ok(None),
        JobOutcome::Failed { message, .. } => Err(Error::Internal(message)),
    }
}
