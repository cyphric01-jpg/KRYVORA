// file: crates/kryvora-cli/src/commands/sanitize_folder.rs
//! `kryvora sanitize-folder` — real recursive directory sanitization.

use kryvora_core::Error;
use kryvora_db::{apply_migrations, open};
use kryvora_jobs::{run_job, CancelToken, JobOutcome, JobRequest};
use kryvora_policy::{evaluate, OperationKind, OperationRequest, PolicyDecision};
use kryvora_sanitize::{
    sanitize_directory, SanitizeDirectoryReport, SanitizeOptions, SanitizeOutcome,
};
use kryvora_storage::inspect_directory;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Run a recursive directory sanitization as a KRYVORA job.
///
/// The caller must have obtained confirmation. This function
/// re-evaluates the policy decision with `user_confirmed = true` and
/// only then runs the job.
///
/// # Errors
///
/// * [`Error::NotFound`] — target does not exist.
/// * [`Error::InvalidInput`] — target is not a directory.
/// * [`Error::PolicyViolation`] — policy refused the operation.
/// * [`Error::JobCancelled`] — the job was cancelled.
pub fn execute(
    db_path: &Path,
    target: &Path,
    confirm: bool,
    actor: Option<&str>,
) -> kryvora_core::Result<()> {
    let profile = inspect_directory(target)?;

    let request = OperationRequest::new(
        OperationKind::SanitizeDirectory,
        format!("sanitize directory {}", profile.display),
    )?;
    let decision = evaluate(&profile, &request)?;
    let assessment = decision.assessment().clone();

    for w in &assessment.warnings {
        eprintln!("warning: {w:?}");
    }

    if let PolicyDecision::Refused { reason, .. } = &decision {
        return Err(Error::PolicyViolation(reason.clone()));
    }

    if !assessment.confirmations.is_empty() && !confirm {
        let prompt = match &assessment.confirmations[0] {
            kryvora_policy::Confirmation::Basic { prompt } => prompt.clone(),
            kryvora_policy::Confirmation::TypedPhrase { prompt, phrase } => {
                format!("{prompt} (required phrase: {phrase:?})")
            }
            kryvora_policy::Confirmation::DeviceIdentity { prompt, .. } => prompt.clone(),
        };
        return Err(Error::PolicyViolation(format!(
            "confirmation required but not provided: {prompt}"
        )));
    }

    let confirmed_request = request.clone().confirmed();
    let decision = evaluate(&profile, &confirmed_request)?;
    if let PolicyDecision::Refused { reason, .. } = &decision {
        return Err(Error::PolicyViolation(reason.clone()));
    }

    let mut conn = open(db_path)?;
    apply_migrations(&mut conn)?;

    let target_path: PathBuf = target.to_path_buf();
    let actor_owned = actor.map(str::to_string);
    let cancel = CancelToken::new();

    let options = SanitizeOptions {
        buffer_size: 1024 * 1024,
        unlink_after: true,
        actor: actor_owned.clone(),
        ..SanitizeOptions::default()
    };

    let slot: Arc<Mutex<Option<SanitizeDirectoryReport>>> = Arc::new(Mutex::new(None));
    let slot_for_body = Arc::clone(&slot);

    let outcome = run_job(
        &conn,
        JobRequest {
            job_type: "sanitize_directory".into(),
            case_id: None,
            evidence_id: None,
            configuration: Some(serde_json::json!({
                "target": target_path.display().to_string(),
            })),
            actor: actor_owned.clone(),
        },
        cancel,
        Box::new(move |ctx| {
            let report = sanitize_directory(ctx.conn(), &target_path, &options)?;
            *slot_for_body
                .lock()
                .map_err(|_| Error::Internal("sanitize slot poisoned".into()))? = Some(report);
            Ok(())
        }),
    )?;

    match &outcome {
        JobOutcome::Succeeded { job_id } | JobOutcome::Partial { job_id, .. } => {
            let report = slot
                .lock()
                .map_err(|_| Error::Internal("sanitize slot poisoned".into()))?
                .take()
                .ok_or_else(|| Error::Internal("sanitize job completed without a report".into()))?;
            print_report(target, job_id, &report);
        }
        JobOutcome::Cancelled { job_id } => {
            println!("=== KRYVORA sanitize-folder ===");
            println!("target:  {}", target.display());
            println!("job:     {job_id}");
            println!("outcome: cancelled");
            println!();
            return Err(Error::JobCancelled);
        }
        JobOutcome::Failed { job_id, message } => {
            println!("=== KRYVORA sanitize-folder ===");
            println!("target:  {}", target.display());
            println!("job:     {job_id}");
            println!("outcome: failed");
            println!("reason:  {message}");
            println!();
            return Err(Error::Internal(message.clone()));
        }
    }

    Ok(())
}

fn outcome_label(o: SanitizeOutcome) -> &'static str {
    match o {
        SanitizeOutcome::Success => "success",
        SanitizeOutcome::Partial => "partial",
        SanitizeOutcome::Failed => "failed",
        SanitizeOutcome::NotVerified => "not_verified",
        SanitizeOutcome::Unsupported => "unsupported",
    }
}

fn print_report(target: &Path, job_id: &kryvora_core::JobId, report: &SanitizeDirectoryReport) {
    println!("=== KRYVORA sanitize-folder ===");
    println!("target:           {}", target.display());
    println!("job:              {job_id}");
    println!("outcome:          {}", outcome_label(report.outcome));
    println!("files discovered: {}", report.files_discovered);
    println!("files processed:  {}", report.files_processed);
    println!("files removed:    {}", report.files_removed);
    println!("files failed:     {}", report.files_failed);
    println!("original bytes:   {}", report.total_original_bytes);
    println!("bytes written:    {}", report.total_bytes_written);
    println!("elapsed (s):      {:.3}", report.elapsed.as_secs_f64());
    if let Some(reason) = &report.reason {
        println!("reason:           {reason}");
    }
    if !report.failures.is_empty() {
        println!();
        println!("per-file failures:");
        for f in &report.failures {
            println!("  {} — {}", f.path, f.reason);
        }
    }
    println!();
}
