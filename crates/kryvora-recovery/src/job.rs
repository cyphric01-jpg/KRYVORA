//! The recovery job: validate candidates, classify, score confidence.

use crate::artifact::{ArtifactCategory, RecoveryMethod};
use crate::confidence::{ConfidenceAssessment, ConfidenceReason, ConfidenceSignal};
use crate::result::{RecoveryError, RecoveryReport, RecoveryResult};
use crate::validator::{validator_for, ValidationOutcome};
use kryvora_audit::{append, EventDraft, EventType};
use kryvora_carving::CarvedCandidate;
use kryvora_core::{CaseId, Error, ReconstructionState, RecoveryResultId, Result, ValidationState};
use kryvora_integrity::calculate_hash;
use kryvora_jobs::{run_job, CancelToken, JobOutcome, JobRequest};
use rusqlite::Connection;
use serde_json::json;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;

pub const MAX_RECOVERY_CANDIDATE_BYTES: u64 = 64 * 1024 * 1024;

/// Slot to pass the recovery report back out of the job body.
type ReportSlot = Arc<Mutex<Option<RecoveryReport>>>;

/// Run a recovery pass over a set of candidates.
///
/// The source reader is used to read candidate bytes. It must be
/// seekable so that the scanner's absolute offsets can be honored.
///
/// Emits a `RecoveryStarted` event at the beginning, and one
/// `ArtifactValidated` event per artifact that passes validation.
/// Candidates that fail validation do not produce an
/// `ArtifactValidated` event; they are reflected in the report's
/// `candidates_rejected` count.
///
/// # Errors
///
/// Any error from the job runner, the audit chain, or the source
/// reader. A candidate that cannot be read produces a `RecoveryError`
/// converted into a core error; this aborts the job.
pub fn run_recovery_job<R: Read + Seek + 'static>(
    conn: &Connection,
    source: R,
    candidates: Vec<CarvedCandidate>,
    evidence_id: Option<kryvora_core::EvidenceId>,
    actor: Option<String>,
) -> Result<Option<RecoveryReport>> {
    run_recovery_job_for_case(conn, source, candidates, evidence_id, None, actor)
}

pub fn run_recovery_job_for_case<R: Read + Seek + 'static>(
    conn: &Connection,
    source: R,
    candidates: Vec<CarvedCandidate>,
    evidence_id: Option<kryvora_core::EvidenceId>,
    case_id: Option<CaseId>,
    actor: Option<String>,
) -> Result<Option<RecoveryReport>> {
    let cancel = CancelToken::new();
    let slot: ReportSlot = Arc::new(Mutex::new(None));
    let slot_for_body = Arc::clone(&slot);

    let candidate_count = candidates.len();
    // Clone the actor for the closure; keep the original for the
    // post-run_job audit append.
    let actor_for_body = actor.clone();

    let outcome = run_job(
        conn,
        JobRequest {
            job_type: "recover".into(),
            case_id,
            evidence_id,
            configuration: Some(json!({
                "candidate_count": candidate_count,
            })),
            actor: actor.clone(),
        },
        cancel,
        Box::new(move |ctx| {
            let mut source = source;
            let mut results: Vec<RecoveryResult> = Vec::new();
            let mut rejected = 0u64;

            for (i, candidate) in candidates.iter().enumerate() {
                if ctx.is_cancelled() {
                    break;
                }

                let fraction = (i + 1) as f64 / candidates.len().max(1) as f64;
                ctx.report_progress(fraction);

                match process_candidate(
                    &mut source,
                    candidate,
                    evidence_id,
                    actor_for_body.as_deref(),
                    conn,
                    ctx.job_id,
                ) {
                    Ok(Some(result)) => results.push(result),
                    Ok(None) => rejected += 1,
                    Err(e) => {
                        return Err(Error::Internal(format!(
                            "candidate at offset {} could not be processed: {e}",
                            candidate.offset
                        )));
                    }
                }
            }

            let report = RecoveryReport {
                case_id,
                job_id: Some(ctx.job_id),
                candidates_considered: candidates.len() as u64,
                candidates_validated: results.len() as u64,
                candidates_rejected: rejected,
                results,
            };

            *slot_for_body
                .lock()
                .map_err(|_| Error::Internal("recovery slot poisoned".into()))? = Some(report);
            Ok(())
        }),
    )?;

    match outcome {
        JobOutcome::Succeeded { job_id } | JobOutcome::Partial { job_id, .. } => {
            let mut report = slot
                .lock()
                .map_err(|_| Error::Internal("recovery slot poisoned".into()))?
                .take()
                .ok_or_else(|| Error::Internal("recovery produced no report".into()))?;
            report.job_id = Some(job_id);

            append(
                conn,
                &EventDraft {
                    event_type: EventType::RecoveryStarted,
                    actor,
                    object_id: None,
                    job_id: Some(job_id),
                    details: json!({
                        "candidates_considered": report.candidates_considered,
                        "candidates_validated": report.candidates_validated,
                        "candidates_rejected": report.candidates_rejected,
                    }),
                },
            )?;

            Ok(Some(report))
        }
        JobOutcome::Cancelled { .. } => Ok(None),
        JobOutcome::Failed { message, .. } => Err(Error::Internal(message)),
    }
}

/// Process a single candidate. Returns `Ok(Some(result))` if it
/// validated, `Ok(None)` if it was rejected, and `Err` on I/O failure.
fn process_candidate<R: Read + Seek>(
    source: &mut R,
    candidate: &CarvedCandidate,
    evidence_id: Option<kryvora_core::EvidenceId>,
    actor: Option<&str>,
    conn: &Connection,
    job_id: kryvora_core::JobId,
) -> Result<Option<RecoveryResult>> {
    if candidate.length == 0 || candidate.length > MAX_RECOVERY_CANDIDATE_BYTES {
        return Ok(None);
    }
    let Some(candidate_end) = candidate.offset.checked_add(candidate.length) else {
        return Ok(None);
    };
    let source_length = source.seek(SeekFrom::End(0)).map_err(|error| {
        Error::Internal(RecoveryError::CandidateReadFailed(error.to_string()).to_string())
    })?;
    if candidate_end > source_length {
        return Ok(None);
    }
    let Ok(candidate_capacity) = usize::try_from(candidate.length) else {
        return Ok(None);
    };

    // 1. Read candidate bytes.
    source
        .seek(SeekFrom::Start(candidate.offset))
        .map_err(|e| {
            Error::Internal(RecoveryError::CandidateReadFailed(e.to_string()).to_string())
        })?;
    let mut buf = Vec::new();
    buf.try_reserve_exact(candidate_capacity)
        .map_err(|error| Error::Internal(format!("candidate allocation failed: {error}")))?;
    buf.resize(candidate_capacity, 0);
    source.read_exact(&mut buf).map_err(|e| {
        Error::Internal(RecoveryError::CandidateReadFailed(e.to_string()).to_string())
    })?;

    // 2. Find a validator.
    let Some(validator) = validator_for(&candidate.format) else {
        return Ok(None);
    };

    // 3. Validate.
    let outcome = validator.validate(&buf);

    // 4. Compute artifact hash regardless of validation outcome. It is
    //    cheap and useful for reports even on rejected candidates.
    let hash = calculate_hash(Cursor::new(&buf))?;

    // 5. Only promote candidates whose validation state is Valid or
    //    Partial. Invalid or Inconclusive are rejected. This is the
    //    "do not promote false positives" rule.
    match outcome.state {
        ValidationState::Valid | ValidationState::Partial => {
            let confidence = assess_confidence(candidate, &buf, &outcome);
            let category = ArtifactCategory::from_format(&candidate.format);

            let result = RecoveryResult {
                id: RecoveryResultId::new(),
                evidence_id,
                source_offset: candidate.offset,
                source_length: candidate.length,
                detected_type: candidate.format.clone(),
                category,
                validation_state: outcome.state,
                confidence,
                recovery_method: RecoveryMethod::SignatureCarving,
                reconstruction_state: ReconstructionState::Contiguous,
                artifact_sha256: hash.digest_hex().to_string(),
                created_at: OffsetDateTime::now_utc(),
                job_id: Some(job_id),
                provenance_id: None,
                report_id: None,
                validation_facts: outcome.structural_facts.clone(),
            };

            append(
                conn,
                &EventDraft {
                    event_type: EventType::ArtifactValidated,
                    actor: actor.map(str::to_string),
                    object_id: Some(result.id.to_string()),
                    job_id: Some(job_id),
                    details: json!({
                        "format": result.detected_type,
                        "offset": result.source_offset,
                        "length": result.source_length,
                        "validation_state": format!("{:?}", result.validation_state).to_lowercase(),
                        "confidence": format!("{:?}", result.confidence.level).to_lowercase(),
                    }),
                },
            )?;

            Ok(Some(result))
        }
        ValidationState::Invalid | ValidationState::Inconclusive | ValidationState::Unknown => {
            Ok(None)
        }
    }
}

/// Assess confidence from a validation outcome.
///
/// Every candidate that reaches this function has already passed the
/// carving layer's size bounds and has already had its signature
/// confirmed by the scan. The remaining signals come from the format
/// validator's structural outcome.
fn assess_confidence(
    candidate: &CarvedCandidate,
    bytes: &[u8],
    outcome: &ValidationOutcome,
) -> ConfidenceAssessment {
    let signature = kryvora_carving::signatures::all_signatures()
        .iter()
        .find(|signature| signature.format == candidate.format);
    let signature_valid = signature.is_some_and(|signature| {
        bytes.starts_with(signature.header)
            && signature.footer.is_some_and(|footer| bytes.ends_with(footer))
    });
    let structural_valid = outcome.state == ValidationState::Valid;
    let format_validated = matches!(outcome.state, ValidationState::Valid);
    let size_consistent = signature.is_some_and(|signature| {
        candidate.length >= signature.min_size && candidate.length <= signature.max_size
    });

    let reasons = vec![
        ConfidenceReason::new(ConfidenceSignal::SignatureValid, signature_valid),
        ConfidenceReason::new(ConfidenceSignal::StructuralValid, structural_valid),
        ConfidenceReason::new(ConfidenceSignal::FormatValidated, format_validated),
        ConfidenceReason::new(ConfidenceSignal::SizeConsistent, size_consistent),
        ConfidenceReason::new(ConfidenceSignal::FragmentConsistent, false).with_note(
            "fragment relationships are not assessed; fragmented-file recovery is unsupported",
        ),
        ConfidenceReason::new(ConfidenceSignal::SourceTraceable, true),
        ConfidenceReason::new(ConfidenceSignal::ReconstructionCertain, true),
    ];

    ConfidenceAssessment::from_reasons(reasons)
}

#[cfg(test)]
mod tests {
    // Integration-level tests live in `tests/recovery_job.rs`.
}
