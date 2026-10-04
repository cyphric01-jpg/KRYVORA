//! `kryvora sanitize-file` — single-file sanitization.

use kryvora_core::Error;
use kryvora_db::{apply_migrations, open};
use kryvora_jobs::{run_job, CancelToken, JobOutcome, JobRequest};
use kryvora_policy::{evaluate, OperationKind, OperationRequest, PolicyDecision};
use kryvora_sanitize::{sanitize_file, SanitizationResult, SanitizeOptions, SanitizeOutcome};
use kryvora_storage::inspect_file;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Sanitize one regular file as a KRYVORA job.
///
/// The caller must have obtained confirmation. This function
/// re-evaluates the policy decision with `user_confirmed = true` and
/// only then runs the job.
///
/// # Errors
///
/// * [`Error::NotFound`] — target does not exist.
/// * [`Error::InvalidInput`] — target is not a regular file.
/// * [`Error::PolicyViolation`] — policy refused the operation.
/// * [`Error::JobCancelled`] — the job was cancelled.
pub fn execute(
    db_path: &Path,
    target: &Path,
    confirm: bool,
    actor: Option<&str>,
) -> kryvora_core::Result<()> {
    let profile = inspect_file(target)?;

    let request = OperationRequest::new(
        OperationKind::SanitizeFile,
        format!("sanitize file {}", profile.display),
    )?;
    let decision = evaluate(&profile, &request)?;
    let assessment = decision.assessment().clone();

    for warning in &assessment.warnings {
        eprintln!("warning: {warning:?}");
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

    let confirmed_request = request.confirmed();
    let decision = evaluate(&profile, &confirmed_request)?;
    if let PolicyDecision::Refused { reason, .. } = &decision {
        return Err(Error::PolicyViolation(reason.clone()));
    }

    let mut conn = open(db_path)?;
    apply_migrations(&mut conn)?;

    let target_path: PathBuf = target.to_path_buf();
    let actor_owned = actor.map(str::to_string);
    let options = SanitizeOptions {
        buffer_size: 1024 * 1024,
        unlink_after: true,
        actor: actor_owned.clone(),
        ..SanitizeOptions::default()
    };

    let slot: Arc<Mutex<Option<SanitizationResult>>> = Arc::new(Mutex::new(None));
    let slot_for_body = Arc::clone(&slot);

    let outcome = run_job(
        &conn,
        JobRequest {
            job_type: "sanitize_file".into(),
            case_id: None,
            evidence_id: None,
            configuration: Some(serde_json::json!({
                "target": target_path.display().to_string(),
            })),
            actor: actor_owned,
        },
        CancelToken::new(),
        Box::new(move |ctx| {
            let result = sanitize_file(ctx.conn(), &target_path, &options)?;
            *slot_for_body
                .lock()
                .map_err(|_| Error::Internal("sanitize slot poisoned".into()))? = Some(result);
            Ok(())
        }),
    )?;

    match &outcome {
        JobOutcome::Succeeded { job_id } | JobOutcome::Partial { job_id, .. } => {
            let result = slot
                .lock()
                .map_err(|_| Error::Internal("sanitize slot poisoned".into()))?
                .take()
                .ok_or_else(|| Error::Internal("sanitize job completed without a result".into()))?;
            print_result(target, job_id, &result);
        }
        JobOutcome::Cancelled { job_id } => {
            println!("=== KRYVORA sanitize-file ===");
            println!("target:  {}", target.display());
            println!("job:     {job_id}");
            println!("outcome: cancelled");
            println!();
            return Err(Error::JobCancelled);
        }
        JobOutcome::Failed { job_id, message } => {
            println!("=== KRYVORA sanitize-file ===");
            println!("target:  {}", target.display());
            println!("job:     {job_id}");
            println!("outcome: job_failed");
            println!("reason:  {message}");
            println!();
            return Err(Error::Internal(message.clone()));
        }
    }

    Ok(())
}

fn outcome_label(outcome: SanitizeOutcome) -> &'static str {
    match outcome {
        SanitizeOutcome::Success => "success",
        SanitizeOutcome::Partial => "partial",
        SanitizeOutcome::Failed => "failed",
        SanitizeOutcome::NotVerified => "not_verified",
        SanitizeOutcome::Unsupported => "unsupported",
    }
}

fn print_result(target: &Path, job_id: &kryvora_core::JobId, result: &SanitizationResult) {
    println!("=== KRYVORA sanitize-file ===");
    println!("target:           {}", target.display());
    println!("job:              {job_id}");
    println!("outcome:          {}", outcome_label(result.outcome));
    println!("bytes overwritten: {}", result.bytes_overwritten);
    println!("elapsed (s):      {:.3}", result.elapsed.as_secs_f64());
    if let Some(reason) = &result.reason {
        println!("reason:           {reason}");
    }
    println!();
}
