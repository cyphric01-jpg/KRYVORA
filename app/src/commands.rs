// file: app/src/commands.rs
//! Tauri command handlers. Thin wrappers over workspace crates.

use crate::error::CommandError;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

// ---------- Read-only queries ----------

#[derive(Debug, Serialize)]
pub struct ChainStatusDto {
    pub state: String,
    pub length: u64,
    pub first_bad_sequence: Option<u64>,
    pub reason: Option<String>,
}

#[tauri::command]
pub fn verify_chain(state: State<'_, AppState>) -> Result<ChainStatusDto, CommandError> {
    let db = state.current_db();
    let conn = kryvora_db::open(db)?;
    let status = kryvora_audit::verify_chain(&conn)?;
    let dto = match status {
        kryvora_audit::ChainStatus::Empty => ChainStatusDto {
            state: "empty".into(),
            length: 0,
            first_bad_sequence: None,
            reason: None,
        },
        kryvora_audit::ChainStatus::Intact { length } => ChainStatusDto {
            state: "intact".into(),
            length,
            first_bad_sequence: None,
            reason: None,
        },
        kryvora_audit::ChainStatus::Broken {
            first_bad_sequence,
            reason,
        } => ChainStatusDto {
            state: "broken".into(),
            length: 0,
            first_bad_sequence: Some(first_bad_sequence),
            reason: Some(format!("{reason:?}")),
        },
    };
    Ok(dto)
}

#[derive(Debug, Deserialize)]
pub struct InspectSanitizeTargetInput {
    pub target: String,
    pub kind: kryvora_storage::TargetKind,
    pub case_id: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SanitizeTargetPreviewDto {
    pub operation_id: String,
    pub plan_id: String,
    pub profile: kryvora_storage::TargetProfile,
    pub assessment: kryvora_policy::SafetyAssessment,
    pub confirmation_phrase: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DriveInspectionInput {
    pub path: String,
    pub method: String,
    pub scope: String,
    pub case_id: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DriveInspectionDto {
    pub profile: kryvora_storage::TargetProfile,
    pub assessment: kryvora_storage::DeviceSafetyAssessment,
    pub can_plan: bool,
    pub execution_disabled: bool,
}

#[derive(Debug, Serialize)]
pub struct DrivePlanDto {
    pub plan_id: String,
    pub device_path: String,
    pub device_identity: String,
    pub method: String,
    pub scope: String,
    pub assessment: kryvora_storage::DeviceSafetyAssessment,
    pub expires_at: String,
    pub execution_disabled: bool,
}

#[tauri::command]
pub fn list_devices() -> Vec<kryvora_storage::TargetProfile> {
    kryvora_storage::list_devices()
}

#[tauri::command]
pub fn inspect_drive_target(
    state: State<'_, AppState>,
    input: DriveInspectionInput,
) -> Result<DriveInspectionDto, CommandError> {
    let candidate = match input.path.trim() {
        "" => {
            return Err(CommandError {
                kind: "invalid_input".into(),
                message: "device path must not be empty".into(),
            });
        }
        value => value,
    };

    let profile = match kryvora_storage::inspect_device_by_id(candidate) {
        Ok(profile) => profile,
        Err(error) => {
            let reason = error.to_string();
            return Err(CommandError {
                kind: "device_not_found".into(),
                message: reason,
            });
        }
    };

    let assessment = kryvora_storage::assess_device_target(&profile, &input.method, &input.scope);
    let can_plan = matches!(assessment.decision, kryvora_storage::DeviceSafetyDecision::AllowedForPlanning);
    if !can_plan {
        let reason = assessment
            .reason
            .clone()
            .unwrap_or_else(|| "device inspection did not pass a safe planning assessment".into());
        let _ = record_drive_refusal(
            &state,
            &profile,
            &input,
            &reason,
        );
        return Err(CommandError {
            kind: "device_policy_violation".into(),
            message: reason,
        });
    }

    Ok(DriveInspectionDto {
        profile,
        assessment,
        can_plan,
        execution_disabled: true,
    })
}

#[tauri::command]
pub fn plan_drive_sanitization(
    state: State<'_, AppState>,
    input: DriveInspectionInput,
) -> Result<DrivePlanDto, CommandError> {
    let profile = match kryvora_storage::inspect_device_by_id(input.path.trim()) {
        Ok(profile) => profile,
        Err(error) => {
            return Err(CommandError {
                kind: "device_not_found".into(),
                message: error.to_string(),
            });
        }
    };

    let assessment = kryvora_storage::assess_device_target(&profile, &input.method, &input.scope);
    if !matches!(assessment.decision, kryvora_storage::DeviceSafetyDecision::AllowedForPlanning) {
        let reason = assessment
            .reason
            .clone()
            .unwrap_or_else(|| "device plan is blocked by backend safety policy".into());
        let _ = record_drive_refusal(&state, &profile, &input, &reason);
        return Err(CommandError {
            kind: "device_policy_violation".into(),
            message: reason,
        });
    }

    let case_id = parse_optional_case_id(input.case_id.as_deref())?;
    let device_identity = profile.device_identity.clone().unwrap_or_else(|| profile.path.clone());
    let plan_id = state.issue_drive_plan(crate::state::DrivePlanRequest {
        device_path: profile.path.clone(),
        device_identity: device_identity.clone(),
        method: input.method.clone(),
        scope: input.scope.clone(),
        assessment: assessment.clone(),
        case_id,
        actor: input.actor.clone(),
    })?;
    let expires_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .checked_add(std::time::Duration::from_secs(900))
        .unwrap_or_default()
        .as_millis();

    Ok(DrivePlanDto {
        plan_id: plan_id.clone(),
        device_path: profile.path.clone(),
        device_identity,
        method: input.method.clone(),
        scope: input.scope.clone(),
        assessment,
        expires_at: expires_at_ms.to_string(),
        execution_disabled: true,
    })
}

fn record_drive_refusal(
    state: &AppState,
    profile: &kryvora_storage::TargetProfile,
    input: &DriveInspectionInput,
    reason: &str,
) -> Result<(), CommandError> {
    let case_id = parse_optional_case_id(input.case_id.as_deref())?;
    let mut conn = kryvora_db::open(state.current_db())?;
    kryvora_db::apply_migrations(&mut conn)?;
    kryvora_audit::append(
        &conn,
        &kryvora_audit::EventDraft {
            event_type: kryvora_audit::EventType::SanitizationRefused,
            actor: input.actor.clone(),
            object_id: Some(profile.path.clone()),
            job_id: None,
            details: serde_json::json!({
                "device_path": profile.path.clone(),
                "device_identity": profile.device_identity.clone(),
                "method": input.method.clone(),
                "scope": input.scope.clone(),
                "case_id": case_id.map(|id| id.to_string()),
                "reason": reason,
                "execution_disabled": true,
            }),
        },
    )?;
    Ok(())
}

#[tauri::command]
pub fn cancel_sanitization_plan(
    state: State<'_, AppState>,
    plan_id: String,
) -> Result<bool, CommandError> {
    let Some(plan) = state.take_sanitization_plan(&plan_id)? else {
        return Ok(false);
    };
    record_sanitization_refusal(&state, &plan, "target inspection cancelled by operator")?;
    Ok(true)
}

#[tauri::command]
pub fn inspect_sanitize_target(
    state: State<'_, AppState>,
    input: InspectSanitizeTargetInput,
) -> Result<SanitizeTargetPreviewDto, CommandError> {
    use kryvora_policy::{evaluate, Confirmation, OperationKind, OperationRequest, PolicyDecision};
    use kryvora_db::repo::SanitizationOperationRepository;
    use kryvora_storage::{TargetIdentity, TargetKind};
    use std::path::Path;

    let target = match inspect_sanitize_profile(Path::new(&input.target), input.kind) {
        Ok(profile) => profile,
        Err(error) => {
            record_unplanned_sanitization_refusal(
                &state,
                &SanitizeInput {
                    target: input.target.clone(),
                    confirm: false,
                    plan_id: None,
                    typed_confirmation: None,
                    case_id: input.case_id.clone(),
                    actor: input.actor.clone(),
                },
                input.kind,
                &error.message,
            )?;
            return Err(error);
        }
    };
    let operation_kind = match input.kind {
        TargetKind::File => OperationKind::SanitizeFile,
        TargetKind::Directory => OperationKind::SanitizeDirectory,
        _ => {
            return Err(CommandError {
                kind: "unsupported_target".into(),
                message: "only regular files and directories can be sanitized".into(),
            });
        }
    };
    let request = OperationRequest::new(operation_kind, target.path.clone())?;
    let decision = evaluate(&target, &request)?;
    let assessment = match decision {
        PolicyDecision::Permitted { assessment } => assessment,
        PolicyDecision::Refused { reason, .. } => {
            record_unplanned_sanitization_refusal(
                &state,
                &SanitizeInput {
                    target: target.path.clone(),
                    confirm: false,
                    plan_id: None,
                    typed_confirmation: None,
                    case_id: input.case_id.clone(),
                    actor: input.actor.clone(),
                },
                input.kind,
                &reason,
            )?;
            return Err(CommandError {
                kind: "policy_violation".into(),
                message: reason,
            });
        }
    };
    let confirmation_phrase = assessment.confirmations.iter().find_map(|confirmation| {
        match confirmation {
            Confirmation::TypedPhrase { phrase, .. } => Some(phrase.clone()),
            _ => None,
        }
    });
    let identity = TargetIdentity::capture(&target.path, input.kind)?;
    let case_id = parse_optional_case_id(input.case_id.as_deref())?;
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;
    let repo = kryvora_db::repo::SqliteSanitizationOperationRepository::new(&conn);
    let operation_id = repo.insert(&kryvora_db::repo::NewSanitizationOperation {
        case_id,
        target_path: target.path.clone(),
        target_kind: input.kind.as_str().to_string(),
        method: "random_overwrite".into(),
        target_identity_verified: false,
        confirmation_kind: if confirmation_phrase.is_some() { "typed_phrase" } else { "basic" }.into(),
        confirmation_validated: false,
        actor: input.actor.clone(),
    })?;
    kryvora_audit::append(
        &conn,
        &kryvora_audit::EventDraft {
            event_type: kryvora_audit::EventType::SanitizationPlanned,
            actor: input.actor.clone(),
            object_id: Some(operation_id.to_string()),
            job_id: None,
            details: serde_json::json!({
                "case_id": case_id.map(|id| id.to_string()),
                "target_path": target.path.clone(),
                "target_kind": input.kind.as_str(),
                "method": "random_overwrite",
            }),
        },
    )?;
    let plan_id = state.issue_sanitization_plan(
        std::path::PathBuf::from(&target.path),
        input.kind,
        identity,
        operation_id,
        case_id,
        input.actor,
    )?;

    Ok(SanitizeTargetPreviewDto {
        operation_id: operation_id.to_string(),
        plan_id,
        profile: target,
        assessment,
        confirmation_phrase,
    })
}

fn inspect_sanitize_profile(
    target: &std::path::Path,
    kind: kryvora_storage::TargetKind,
) -> Result<kryvora_storage::TargetProfile, CommandError> {
    let canonical = target.canonicalize()?;
    if let Some(reason) = kryvora_sanitize::is_system_path(&canonical) {
        return Err(CommandError {
            kind: "unsafe_target".into(),
            message: reason.message(),
        });
    }
    if kind == kryvora_storage::TargetKind::Directory
        && kryvora_storage::is_volume_root(&canonical)
    {
        return Err(CommandError {
            kind: "unsafe_target".into(),
            message: "filesystem roots and mount points cannot be sanitized".into(),
        });
    }
    match kind {
        kryvora_storage::TargetKind::File => kryvora_storage::inspect_file(canonical).map_err(Into::into),
        kryvora_storage::TargetKind::Directory => {
            kryvora_storage::inspect_directory(canonical).map_err(Into::into)
        }
        _ => Err(CommandError {
            kind: "unsupported_target".into(),
            message: "target inspection supports regular files and directories only".into(),
        }),
    }
}

#[derive(Debug, Serialize)]
pub struct CaseDto {
    pub id: String,
    pub title: String,
    pub examiner: Option<String>,
    pub created_at: String,
}

#[tauri::command]
pub fn list_cases(state: State<'_, AppState>) -> Result<Vec<CaseDto>, CommandError> {
    use kryvora_db::repo::{CaseRepository, SqliteCaseRepository};
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;
    let repo = SqliteCaseRepository::new(&conn);
    let rows = repo.list(200, 0)?;
    Ok(rows
        .into_iter()
        .map(|r| CaseDto {
            id: r.id.to_string(),
            title: r.title,
            examiner: r.examiner,
            created_at: r.created_at,
        })
        .collect())
}

#[derive(Debug, Serialize)]
pub struct EvidenceDto {
    pub id: String,
    pub case_id: String,
    pub source_path: String,
    pub size_bytes: u64,
    pub hash_digest: String,
    pub integrity_state: String,
}

#[tauri::command]
pub fn list_evidence_for_case(
    state: State<'_, AppState>,
    case_id: String,
) -> Result<Vec<EvidenceDto>, CommandError> {
    use kryvora_db::repo::{EvidenceRepository, SqliteEvidenceRepository};
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;
    let uuid = parse_uuid(&case_id).ok_or_else(|| CommandError {
        kind: "invalid_input".into(),
        message: format!("bad case id: {case_id}"),
    })?;
    let cid = kryvora_core::CaseId::from_uuid(uuid);
    let repo = SqliteEvidenceRepository::new(&conn);
    let rows = repo.list_by_case(&cid)?;
    Ok(rows
        .into_iter()
        .map(|r| EvidenceDto {
            id: r.id.to_string(),
            case_id: r.case_id.to_string(),
            source_path: r.source_path,
            size_bytes: r.size_bytes,
            hash_digest: r.hash_digest,
            integrity_state: r.integrity_state,
        })
        .collect())
}

#[derive(Debug, Serialize)]
pub struct JobDto {
    pub id: String,
    pub job_type: String,
    pub state: String,
    pub progress: f64,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[tauri::command]
pub fn list_jobs(state: State<'_, AppState>) -> Result<Vec<JobDto>, CommandError> {
    use kryvora_db::repo::{JobRepository, SqliteJobRepository};
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;
    let repo = SqliteJobRepository::new(&conn);
    // We expose jobs by state; for a dashboard we list the non-terminal
    // states first, then terminal. Simplest: iterate states.
    let mut all = Vec::new();
    for s in [
        "running",
        "queued",
        "created",
        "verifying",
        "succeeded",
        "failed",
        "cancelled",
        "partial",
        "inconclusive",
    ] {
        let rows = repo.list_by_state(s, 100)?;
        for r in rows {
            all.push(JobDto {
                id: r.id.to_string(),
                job_type: r.job_type,
                state: r.state,
                progress: r.progress,
                created_at: r.created_at,
                started_at: r.started_at,
                completed_at: r.completed_at,
            });
        }
    }
    Ok(all)
}

#[derive(Debug, Serialize)]
pub struct AuditEventDto {
    pub id: String,
    pub sequence: u64,
    pub event_type: String,
    pub actor: Option<String>,
    pub object_id: Option<String>,
    pub details: String,
    pub created_at: String,
    pub previous_hash: String,
    pub current_hash: String,
}

#[tauri::command]
pub fn list_audit_events(state: State<'_, AppState>) -> Result<Vec<AuditEventDto>, CommandError> {
    use kryvora_db::repo::{AuditEventRepository, SqliteAuditEventRepository};
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;
    let repo = SqliteAuditEventRepository::new(&conn);
    let rows = repo.list_range(0, 500)?;
    Ok(rows
        .into_iter()
        .map(|r| AuditEventDto {
            id: r.id.to_string(),
            sequence: r.sequence,
            event_type: r.event_type,
            actor: r.actor,
            object_id: r.object_id,
            details: r.details,
            created_at: r.created_at,
            previous_hash: r.previous_hash,
            current_hash: r.current_hash,
        })
        .collect())
}

#[derive(Debug, Serialize)]
pub struct ReportDto {
    pub id: String,
    pub case_id: Option<String>,
    pub kind: String,
    pub path: String,
    pub sha256: String,
    pub byte_size: u64,
    pub created_at: String,
    pub integrity_status: String,
}

#[tauri::command]
pub fn list_reports(state: State<'_, AppState>) -> Result<Vec<ReportDto>, CommandError> {
    use kryvora_db::repo::{ReportRepository, SqliteReportRepository};
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;
    let repo = SqliteReportRepository::new(&conn);
    // List for every case; the repository exposes per-case listing.
    // Simplest: query all rows via the case-less list_by_case path is
    // not available; we iterate over cases.
    use kryvora_db::repo::{CaseRepository, SqliteCaseRepository};
    let cases = SqliteCaseRepository::new(&conn).list(500, 0)?;
    let mut out = Vec::new();
    for c in cases {
        let rows = repo.list_by_case(&c.id)?;
        for r in rows {
            let integrity_status = verify_report_file(
                &state.reports_dir(),
                &r.path,
                &r.sha256,
                r.byte_size,
            );
            out.push(ReportDto {
                id: r.id.to_string(),
                case_id: r.case_id.map(|c| c.to_string()),
                kind: r.kind.as_str().to_string(),
                path: r.path,
                sha256: r.sha256,
                byte_size: r.byte_size,
                created_at: r.created_at,
                integrity_status,
            });
        }
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
pub struct RecoveryResultDto {
    pub id: String,
    pub evidence_id: String,
    pub source_offset: u64,
    pub source_length: u64,
    pub detected_type: String,
    pub category: String,
    pub validation_state: String,
    pub confidence: String,
    pub artifact_sha256: String,
    pub artifact_path: Option<String>,
}

#[tauri::command]
pub fn list_recovery_results(
    state: State<'_, AppState>,
    evidence_id: String,
) -> Result<Vec<RecoveryResultDto>, CommandError> {
    use kryvora_db::repo::{RecoveryResultRepository, SqliteRecoveryResultRepository};
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;
    let uuid = parse_uuid(&evidence_id).ok_or_else(|| CommandError {
        kind: "invalid_input".into(),
        message: format!("bad evidence id: {evidence_id}"),
    })?;
    let eid = kryvora_core::EvidenceId::from_uuid(uuid);
    let repo = SqliteRecoveryResultRepository::new(&conn);
    let rows = repo.list_by_evidence(&eid)?;
    Ok(rows
        .into_iter()
        .map(|r| RecoveryResultDto {
            id: r.id.to_string(),
            evidence_id: r.evidence_id.to_string(),
            source_offset: r.source_offset,
            source_length: r.source_length,
            detected_type: r.detected_type,
            category: r.category,
            validation_state: r.validation_state,
            confidence: r.confidence_level,
            artifact_path: verified_recovered_artifact_path(
                &state.recovered_dir(),
                r.artifact_path.as_deref(),
                &r.artifact_sha256,
                r.source_length,
            ),
            artifact_sha256: r.artifact_sha256,
        })
        .collect())
}

// ---------- Mutating commands ----------

#[derive(Debug, Deserialize)]
pub struct CreateCaseInput {
    pub title: String,
    pub examiner: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateCaseOutput {
    pub case_id: String,
}

#[tauri::command]
pub fn create_case(
    state: State<'_, AppState>,
    input: CreateCaseInput,
) -> Result<CreateCaseOutput, CommandError> {
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;
    create_case_inner(&conn, input)
}

fn create_case_inner(
    conn: &rusqlite::Connection,
    input: CreateCaseInput,
) -> Result<CreateCaseOutput, CommandError> {
    use kryvora_db::repo::{CaseRepository, NewCase, SqliteCaseRepository};

    if input.title.len() > 256 || input.examiner.as_ref().is_some_and(|value| value.len() > 256) {
        return Err(CommandError {
            kind: "invalid_input".into(),
            message: "case title and examiner must each be at most 256 UTF-8 bytes".into(),
        });
    }
    conn.execute_batch("SAVEPOINT kryvora_case_create")?;
    let result = (|| -> Result<CreateCaseOutput, CommandError> {
        let id = SqliteCaseRepository::new(conn).insert(&NewCase {
            title: input.title.clone(),
            examiner: input.examiner.clone(),
            notes: None,
        })?;
        let case_id = id.to_string();
        kryvora_audit::append(
            conn,
            &kryvora_audit::EventDraft {
                event_type: kryvora_audit::EventType::CaseCreated,
                actor: input.examiner,
                object_id: Some(case_id.clone()),
                job_id: None,
                details: serde_json::json!({ "title": input.title }),
            },
        )?;
        Ok(CreateCaseOutput { case_id })
    })();
    finish_savepoint(conn, "kryvora_case_create", result)
}

#[derive(Debug, Deserialize)]
pub struct RegisterEvidenceInput {
    pub case_id: String,
    pub path: String,
    pub notes: Option<String>,
    pub actor: Option<String>,
}

#[tauri::command]
pub fn register_evidence(
    state: State<'_, AppState>,
    input: RegisterEvidenceInput,
) -> Result<EvidenceDto, CommandError> {
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;
    register_evidence_inner(&conn, input)
}

fn register_evidence_inner(
    conn: &rusqlite::Connection,
    input: RegisterEvidenceInput,
) -> Result<EvidenceDto, CommandError> {
    use kryvora_db::repo::{CaseRepository, SqliteCaseRepository};
    use kryvora_evidence::{register_evidence as register, RegistrationRequest, SourceType};
    use kryvora_integrity::calculate_hash;
    use std::io::BufReader;

    if input.path.len() > 32_768
        || input.notes.as_ref().is_some_and(|value| value.len() > 4_096)
        || input.actor.as_ref().is_some_and(|value| value.len() > 256)
    {
        return Err(CommandError {
            kind: "invalid_input".into(),
            message: "evidence path, notes, or actor exceeds the supported input limit".into(),
        });
    }
    let case_uuid = parse_uuid(&input.case_id).ok_or_else(|| CommandError {
        kind: "invalid_input".into(),
        message: format!("bad case id: {}", input.case_id),
    })?;
    let case_id = kryvora_core::CaseId::from_uuid(case_uuid);
    let (source_path, file) = open_evidence_source(&input.path)?;
    if SqliteCaseRepository::new(conn).get(&case_id)?.is_none() {
        return Err(CommandError {
            kind: "not_found".into(),
            message: format!("case {case_id}"),
        });
    }

    let hash = calculate_hash(BufReader::new(file))?;
    let size_bytes = hash.input_size();
    let digest = hash.digest_hex().to_string();
    let canonical_path = source_path.to_string_lossy().into_owned();
    conn.execute_batch("SAVEPOINT kryvora_evidence_register")?;
    let result = (|| -> Result<EvidenceDto, CommandError> {
    let evidence_id = register(
        conn,
        &RegistrationRequest {
            case_id,
            source_type: SourceType::File,
            source_path: canonical_path.clone(),
            size_bytes,
            hash,
            read_only: true,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            acquisition_metadata: None,
            notes: input.notes,
        },
    )?;
    kryvora_provenance::ensure_evidence_root(conn, &case_id, &evidence_id)?;
    kryvora_audit::append(
        conn,
        &kryvora_audit::EventDraft {
            event_type: kryvora_audit::EventType::EvidenceRegistered,
            actor: input.actor,
            object_id: Some(evidence_id.to_string()),
            job_id: None,
            details: serde_json::json!({
                "case_id": case_id.to_string(),
                "source_path": canonical_path,
                "hash_algorithm": "sha256",
                "hash_digest": digest,
                "size_bytes": size_bytes,
            }),
        },
    )?;

    Ok(EvidenceDto {
        id: evidence_id.to_string(),
        case_id: case_id.to_string(),
        source_path: canonical_path,
        size_bytes,
        hash_digest: digest,
        integrity_state: "verified".into(),
    })
    })();
    finish_savepoint(conn, "kryvora_evidence_register", result)
}

#[derive(Debug, Deserialize)]
pub struct VerifyEvidenceInput {
    pub evidence_id: String,
    pub actor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VerifyEvidenceOutput {
    pub evidence_id: String,
    pub state: String,
    pub expected_hash: String,
    pub actual_hash: String,
    pub expected_size_bytes: u64,
    pub actual_size_bytes: u64,
}

#[tauri::command]
pub fn verify_evidence(
    state: State<'_, AppState>,
    input: VerifyEvidenceInput,
) -> Result<VerifyEvidenceOutput, CommandError> {
    use kryvora_evidence::{get_evidence, verify_evidence as compare_evidence};
    use kryvora_integrity::calculate_hash;
    use std::io::BufReader;

    let evidence_uuid = parse_uuid(&input.evidence_id).ok_or_else(|| CommandError {
        kind: "invalid_input".into(),
        message: format!("bad evidence id: {}", input.evidence_id),
    })?;
    let evidence_id = kryvora_core::EvidenceId::from_uuid(evidence_uuid);
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;

    let evidence = get_evidence(&conn, &evidence_id)?;
    let (_source_path, file) = open_evidence_source(&evidence.source_path)?;
    let actual = calculate_hash(BufReader::new(file))?;
    let state = compare_evidence(&conn, &evidence_id, &actual)?;
    let state_name = format!("{state:?}").to_lowercase();

    kryvora_audit::append(
        &conn,
        &kryvora_audit::EventDraft {
            event_type: kryvora_audit::EventType::IntegrityVerified,
            actor: input.actor,
            object_id: Some(evidence_id.to_string()),
            job_id: None,
            details: serde_json::json!({
                "state": state_name,
                "expected_hash": evidence.hash.digest_hex(),
                "actual_hash": actual.digest_hex(),
                "expected_size_bytes": evidence.size_bytes,
                "actual_size_bytes": actual.input_size(),
            }),
        },
    )?;

    Ok(VerifyEvidenceOutput {
        evidence_id: evidence_id.to_string(),
        state: state_name,
        expected_hash: evidence.hash.digest_hex().to_string(),
        actual_hash: actual.digest_hex().to_string(),
        expected_size_bytes: evidence.size_bytes,
        actual_size_bytes: actual.input_size(),
    })
}

fn open_evidence_source(path: &str) -> Result<(std::path::PathBuf, std::fs::File), CommandError> {
    use std::path::Path;

    if path.trim().is_empty() {
        return Err(CommandError {
            kind: "invalid_input".into(),
            message: "source path must not be empty".into(),
        });
    }

    let canonical_path = Path::new(path).canonicalize()?;
    let file = std::fs::File::open(&canonical_path)?;
    if !file.metadata()?.is_file() {
        return Err(CommandError {
            kind: "invalid_input".into(),
            message: "evidence source must be a regular file".into(),
        });
    }
    Ok((canonical_path, file))
}

#[derive(Debug, Deserialize)]
pub struct HashFileInput {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct HashFileOutput {
    pub algorithm: String,
    pub digest: String,
    pub size_bytes: u64,
}

#[tauri::command]
pub fn hash_file(input: HashFileInput) -> Result<HashFileOutput, CommandError> {
    use std::fs::File;
    use std::io::BufReader;
    let file = File::open(&input.path)?;
    let reader = BufReader::new(file);
    let hash = kryvora_integrity::calculate_hash(reader)?;
    Ok(HashFileOutput {
        algorithm: hash.algorithm().as_str().to_string(),
        digest: hash.digest_hex().to_string(),
        size_bytes: hash.input_size(),
    })
}

#[derive(Debug, Deserialize)]
pub struct SanitizeInput {
    pub target: String,
    pub confirm: bool,
    pub plan_id: Option<String>,
    pub typed_confirmation: Option<String>,
    pub case_id: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SanitizationOperationDto {
    pub id: String,
    pub case_id: Option<String>,
    pub target_path: String,
    pub target_kind: String,
    pub method: String,
    pub target_identity_verified: bool,
    pub confirmation_kind: String,
    pub confirmation_validated: bool,
    pub state: String,
    pub outcome: Option<String>,
    pub result_json: Option<String>,
    pub error_message: Option<String>,
    pub actor: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[tauri::command]
pub fn list_sanitization_operations(
    state: State<'_, AppState>,
) -> Result<Vec<SanitizationOperationDto>, CommandError> {
    use kryvora_db::repo::{
        SanitizationOperationRepository, SqliteSanitizationOperationRepository,
    };
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;
    let rows = SqliteSanitizationOperationRepository::new(&conn).list_recent(200)?;
    Ok(rows.into_iter().map(sanitization_operation_dto).collect())
}

#[derive(Debug)]
struct SanitizeExecution {
    conn: rusqlite::Connection,
    plan: crate::state::PendingSanitizationPlan,
}

fn begin_sanitization(
    state: &AppState,
    input: &SanitizeInput,
    expected_kind: kryvora_storage::TargetKind,
) -> Result<SanitizeExecution, CommandError> {
    use kryvora_policy::{evaluate, OperationKind, OperationRequest, PolicyDecision};
    use kryvora_storage::TargetKind;
    use std::path::Path;

    if input.target.trim().is_empty() {
        return Err(CommandError {
            kind: "invalid_input".into(),
            message: "sanitization target path must not be empty".into(),
        });
    }

    let plan = match input.plan_id.as_deref() {
        Some(id) => state.take_sanitization_plan(id)?,
        None => None,
    };
    let Some(plan) = plan else {
        let message = "target inspection plan is missing, expired, or already used";
        record_unplanned_sanitization_refusal(state, input, expected_kind, message)?;
        return Err(CommandError {
            kind: "policy_violation".into(),
            message: message.into(),
        });
    };

    let refuse = |reason: &str| -> Result<CommandError, CommandError> {
        record_sanitization_refusal(state, &plan, reason)?;
        Ok(CommandError {
            kind: "policy_violation".into(),
            message: reason.to_string(),
        })
    };

    if plan.target_kind != expected_kind {
        return Err(refuse("sanitization plan target kind does not match command")?);
    }
    let supplied_case_id = match parse_optional_case_id(input.case_id.as_deref()) {
        Ok(case_id) => case_id,
        Err(error) => return Err(refuse(&error.message)?),
    };
    if supplied_case_id != plan.case_id || input.actor != plan.actor {
        return Err(refuse("case or actor context changed after target inspection")?);
    }
    let supplied_canonical = match Path::new(&input.target).canonicalize() {
        Ok(path) => path,
        Err(error) => {
            let reason = format!("could not revalidate sanitization target: {error}");
            return Err(refuse(&reason)?);
        }
    };
    let identity_matches = match plan.identity.matches_path(&supplied_canonical) {
        Ok(matches) => matches,
        Err(error) => return Err(refuse(&error.to_string())?),
    };
    if supplied_canonical != plan.target_path || !identity_matches {
        return Err(refuse("sanitization target changed after inspection")?);
    }

    let profile = match inspect_sanitize_profile(&supplied_canonical, expected_kind) {
        Ok(profile) => profile,
        Err(error) => return Err(refuse(&error.message)?),
    };
    let operation_kind = match expected_kind {
        TargetKind::File => OperationKind::SanitizeFile,
        TargetKind::Directory => OperationKind::SanitizeDirectory,
        _ => return Err(refuse("only file and directory sanitization is supported")?),
    };
    let request = OperationRequest::new(operation_kind, profile.path.clone())?;
    let initial = evaluate(&profile, &request)?;
    let assessment = match initial {
        PolicyDecision::Permitted { assessment } => assessment,
        PolicyDecision::Refused { reason, .. } => return Err(refuse(&reason)?),
    };
    if let Err(reason) = validate_sanitization_confirmation(
        &assessment,
        input.confirm,
        input.typed_confirmation.as_deref(),
    ) {
        return Err(refuse(&reason)?);
    }
    let confirmed_request = request.confirmed();
    match evaluate(&profile, &confirmed_request)? {
        PolicyDecision::Permitted { .. } => {}
        PolicyDecision::Refused { reason, .. } => return Err(refuse(&reason)?),
    }

    // Recheck identity immediately before the persisted running transition.
    let identity_matches = match plan.identity.matches_path(&supplied_canonical) {
        Ok(matches) => matches,
        Err(error) => return Err(refuse(&error.to_string())?),
    };
    if !identity_matches {
        return Err(refuse("sanitization target changed immediately before execution")?);
    }

    let mut conn = kryvora_db::open(state.current_db())?;
    kryvora_db::apply_migrations(&mut conn)?;
    conn.execute_batch("SAVEPOINT kryvora_sanitize_start")?;
    let started = (|| -> Result<(), CommandError> {
        use kryvora_db::repo::{
            SanitizationOperationRepository, SqliteSanitizationOperationRepository,
        };
        SqliteSanitizationOperationRepository::new(&conn).transition(
            &plan.operation_id,
            kryvora_core::SanitizationOperationState::Running,
            None,
            None,
            None,
        )?;
        kryvora_audit::append(
            &conn,
            &kryvora_audit::EventDraft {
                event_type: kryvora_audit::EventType::SanitizationStarted,
                actor: plan.actor.clone(),
                object_id: Some(plan.operation_id.to_string()),
                job_id: None,
                details: serde_json::json!({
                    "operation_id": plan.operation_id.to_string(),
                    "case_id": plan.case_id.map(|id| id.to_string()),
                    "target_path": plan.target_path,
                    "target_kind": plan.target_kind.as_str(),
                    "target_identity_verified": true,
                    "confirmation_validated": true,
                }),
            },
        )?;
        Ok(())
    })();
    match started {
        Ok(()) => conn.execute_batch("RELEASE SAVEPOINT kryvora_sanitize_start")?,
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT kryvora_sanitize_start; RELEASE SAVEPOINT kryvora_sanitize_start",
            );
            return Err(error);
        }
    }
    Ok(SanitizeExecution { conn, plan })
}

fn validate_sanitization_confirmation(
    assessment: &kryvora_policy::SafetyAssessment,
    confirmed: bool,
    typed_confirmation: Option<&str>,
) -> Result<(), String> {
    if !confirmed {
        return Err("explicit confirmation is required".into());
    }
    for confirmation in &assessment.confirmations {
        match confirmation {
            kryvora_policy::Confirmation::Basic { .. } => {}
            kryvora_policy::Confirmation::TypedPhrase { phrase, .. } => {
                if typed_confirmation != Some(phrase.as_str()) {
                    return Err(format!("typed confirmation must exactly match {phrase}"));
                }
            }
            kryvora_policy::Confirmation::DeviceIdentity { .. } => {
                return Err("device sanitization is not available through this command".into());
            }
        }
    }
    Ok(())
}

fn record_unplanned_sanitization_refusal(
    state: &AppState,
    input: &SanitizeInput,
    target_kind: kryvora_storage::TargetKind,
    reason: &str,
) -> Result<(), CommandError> {
    use kryvora_db::repo::{
        NewSanitizationOperation, SanitizationOperationRepository,
        SqliteSanitizationOperationRepository,
    };
    let mut conn = kryvora_db::open(state.current_db())?;
    kryvora_db::apply_migrations(&mut conn)?;
    let (case_id, case_parse_error) = match parse_optional_case_id(input.case_id.as_deref()) {
        Ok(case_id) => (case_id, None),
        Err(error) => (None, Some(error.message)),
    };
    let confirmation_kind = if target_kind == kryvora_storage::TargetKind::Directory {
        "typed_phrase"
    } else {
        "basic"
    };
    let repo = SqliteSanitizationOperationRepository::new(&conn);
    let operation_id = repo.insert(&NewSanitizationOperation {
        case_id,
        target_path: input.target.clone(),
        target_kind: target_kind.as_str().to_string(),
        method: "random_overwrite".into(),
        target_identity_verified: false,
        confirmation_kind: confirmation_kind.into(),
        confirmation_validated: false,
        actor: input.actor.clone(),
    })?;
    repo.transition(
        &operation_id,
        kryvora_core::SanitizationOperationState::Refused,
        None,
        None,
        Some(reason),
    )?;
    let reason = case_parse_error
        .map(|parse_error| format!("{reason}; case context rejected: {parse_error}"))
        .unwrap_or_else(|| reason.to_string());
    append_sanitization_refusal(
        &conn,
        operation_id.to_string(),
        case_id.map(|id| id.to_string()),
        input.target.clone(),
        input.actor.clone(),
        &reason,
    )
}

fn record_sanitization_refusal(
    state: &AppState,
    plan: &crate::state::PendingSanitizationPlan,
    reason: &str,
) -> Result<(), CommandError> {
    use kryvora_db::repo::{
        SanitizationOperationRepository, SqliteSanitizationOperationRepository,
    };
    let mut conn = kryvora_db::open(state.current_db())?;
    kryvora_db::apply_migrations(&mut conn)?;
    SqliteSanitizationOperationRepository::new(&conn).transition(
        &plan.operation_id,
        kryvora_core::SanitizationOperationState::Refused,
        None,
        None,
        Some(reason),
    )?;
    append_sanitization_refusal(
        &conn,
        plan.operation_id.to_string(),
        plan.case_id.map(|id| id.to_string()),
        plan.target_path.display().to_string(),
        plan.actor.clone(),
        reason,
    )
}

fn append_sanitization_refusal(
    conn: &rusqlite::Connection,
    operation_id: String,
    case_id: Option<String>,
    target_path: String,
    actor: Option<String>,
    reason: &str,
) -> Result<(), CommandError> {
    kryvora_audit::append(
        conn,
        &kryvora_audit::EventDraft {
            event_type: kryvora_audit::EventType::SanitizationRefused,
            actor,
            object_id: Some(operation_id.clone()),
            job_id: None,
            details: serde_json::json!({
                "operation_id": operation_id,
                "case_id": case_id,
                "target_path": target_path,
                "reason": reason,
            }),
        },
    )?;
    Ok(())
}

fn record_sanitization_completion(
    execution: &SanitizeExecution,
    outcome: &str,
    result_json: &str,
    reason: Option<&str>,
) -> Result<(), CommandError> {
    use kryvora_db::repo::{
        SanitizationOperationRepository, SqliteSanitizationOperationRepository,
    };
    execution.conn.execute_batch("SAVEPOINT kryvora_sanitize_finish")?;
    let result = (|| -> Result<(), CommandError> {
        SqliteSanitizationOperationRepository::new(&execution.conn).transition(
            &execution.plan.operation_id,
            kryvora_core::SanitizationOperationState::Completed,
            Some(outcome),
            Some(result_json),
            reason,
        )?;
        kryvora_audit::append(
            &execution.conn,
            &kryvora_audit::EventDraft {
                event_type: kryvora_audit::EventType::SanitizationOutcomeRecorded,
                actor: execution.plan.actor.clone(),
                object_id: Some(execution.plan.operation_id.to_string()),
                job_id: None,
                details: serde_json::json!({
                    "operation_id": execution.plan.operation_id.to_string(),
                    "case_id": execution.plan.case_id.map(|id| id.to_string()),
                    "target_path": execution.plan.target_path,
                    "outcome": outcome,
                    "reason": reason,
                }),
            },
        )?;
        Ok(())
    })();
    match result {
        Ok(()) => execution.conn.execute_batch("RELEASE SAVEPOINT kryvora_sanitize_finish")?,
        Err(error) => {
            let _ = execution.conn.execute_batch(
                "ROLLBACK TO SAVEPOINT kryvora_sanitize_finish; RELEASE SAVEPOINT kryvora_sanitize_finish",
            );
            return Err(error);
        }
    }
    Ok(())
}

fn record_sanitization_failure(
    execution: &SanitizeExecution,
    error_message: &str,
) -> Result<(), CommandError> {
    use kryvora_db::repo::{
        SanitizationOperationRepository, SqliteSanitizationOperationRepository,
    };
    execution.conn.execute_batch("SAVEPOINT kryvora_sanitize_failure")?;
    let result = (|| -> Result<(), CommandError> {
        SqliteSanitizationOperationRepository::new(&execution.conn).transition(
            &execution.plan.operation_id,
            kryvora_core::SanitizationOperationState::Failed,
            Some("failed"),
            None,
            Some(error_message),
        )?;
        kryvora_audit::append(
            &execution.conn,
            &kryvora_audit::EventDraft {
                event_type: kryvora_audit::EventType::SanitizationOutcomeRecorded,
                actor: execution.plan.actor.clone(),
                object_id: Some(execution.plan.operation_id.to_string()),
                job_id: None,
                details: serde_json::json!({
                    "operation_id": execution.plan.operation_id.to_string(),
                    "case_id": execution.plan.case_id.map(|id| id.to_string()),
                    "target_path": execution.plan.target_path,
                    "outcome": "failed",
                    "error": error_message,
                }),
            },
        )?;
        Ok(())
    })();
    match result {
        Ok(()) => execution.conn.execute_batch("RELEASE SAVEPOINT kryvora_sanitize_failure")?,
        Err(error) => {
            let _ = execution.conn.execute_batch(
                "ROLLBACK TO SAVEPOINT kryvora_sanitize_failure; RELEASE SAVEPOINT kryvora_sanitize_failure",
            );
            return Err(error);
        }
    }
    Ok(())
}

fn sanitization_operation_dto(
    row: kryvora_db::repo::SanitizationOperationRow,
) -> SanitizationOperationDto {
    SanitizationOperationDto {
        id: row.id.to_string(),
        case_id: row.case_id.map(|id| id.to_string()),
        target_path: row.target_path,
        target_kind: row.target_kind,
        method: row.method,
        target_identity_verified: row.target_identity_verified,
        confirmation_kind: row.confirmation_kind,
        confirmation_validated: row.confirmation_validated,
        state: row.state.as_str().to_string(),
        outcome: row.outcome,
        result_json: row.result_json,
        error_message: row.error_message,
        actor: row.actor,
        created_at: row.created_at,
        started_at: row.started_at,
        completed_at: row.completed_at,
    }
}

#[derive(Debug, Serialize)]
pub struct SanitizeFileOutput {
    pub operation_id: String,
    pub outcome: String,
    pub bytes_overwritten: u64,
    pub elapsed_secs: f64,
    pub reason: Option<String>,
}

#[tauri::command]
pub fn sanitize_file(
    state: State<'_, AppState>,
    input: SanitizeInput,
) -> Result<SanitizeFileOutput, CommandError> {
    let execution = begin_sanitization(&state, &input, kryvora_storage::TargetKind::File)?;
    let options = kryvora_sanitize::SanitizeOptions {
        buffer_size: 1024 * 1024,
        unlink_after: true,
        actor: execution.plan.actor.clone(),
        operation_id: Some(execution.plan.operation_id.to_string()),
        case_id: execution.plan.case_id.map(|id| id.to_string()),
        target_identity_verified: true,
        ..kryvora_sanitize::SanitizeOptions::default()
    };

    let result = match kryvora_sanitize::sanitize_file_bound(
        &execution.conn,
        &execution.plan.target_path,
        &options,
        Some(&execution.plan.identity),
    ) {
        Ok(result) => result,
        Err(error) => {
            record_sanitization_failure(&execution, &error.to_string())?;
            return Err(error.into());
        }
    };
    record_sanitization_completion(
        &execution,
        result.outcome.as_str(),
        &serde_json::to_string(&result).map_err(|error| CommandError {
            kind: "internal_error".into(),
            message: format!("could not serialize sanitization result: {error}"),
        })?,
        result.reason.as_deref(),
    )?;
    Ok(SanitizeFileOutput {
        operation_id: execution.plan.operation_id.to_string(),
        outcome: result.outcome.as_str().to_string(),
        bytes_overwritten: result.bytes_overwritten,
        elapsed_secs: result.elapsed.as_secs_f64(),
        reason: result.reason,
    })
}

#[derive(Debug, Serialize)]
pub struct SanitizeFolderOutput {
    pub operation_id: String,
    pub outcome: String,
    pub files_discovered: u64,
    pub files_processed: u64,
    pub files_removed: u64,
    pub files_failed: u64,
    pub total_original_bytes: u64,
    pub total_bytes_written: u64,
    pub elapsed_secs: f64,
    pub reason: Option<String>,
    pub failures: Vec<FileFailureDto>,
}

#[derive(Debug, Serialize)]
pub struct FileFailureDto {
    pub path: String,
    pub reason: String,
}

#[tauri::command]
pub fn sanitize_folder(
    state: State<'_, AppState>,
    input: SanitizeInput,
) -> Result<SanitizeFolderOutput, CommandError> {
    let execution = begin_sanitization(&state, &input, kryvora_storage::TargetKind::Directory)?;
    let options = kryvora_sanitize::SanitizeOptions {
        buffer_size: 1024 * 1024,
        unlink_after: true,
        actor: execution.plan.actor.clone(),
        operation_id: Some(execution.plan.operation_id.to_string()),
        case_id: execution.plan.case_id.map(|id| id.to_string()),
        target_identity_verified: true,
        ..kryvora_sanitize::SanitizeOptions::default()
    };

    let report = match kryvora_sanitize::sanitize_directory_bound(
        &execution.conn,
        &execution.plan.target_path,
        &options,
        Some(&execution.plan.identity),
    ) {
        Ok(report) => report,
        Err(error) => {
            record_sanitization_failure(&execution, &error.to_string())?;
            return Err(error.into());
        }
    };
    record_sanitization_completion(
        &execution,
        report.outcome.as_str(),
        &serde_json::to_string(&report).map_err(|error| CommandError {
            kind: "internal_error".into(),
            message: format!("could not serialize sanitization result: {error}"),
        })?,
        report.reason.as_deref(),
    )?;
    Ok(SanitizeFolderOutput {
        operation_id: execution.plan.operation_id.to_string(),
        outcome: report.outcome.as_str().to_string(),
        files_discovered: report.files_discovered,
        files_processed: report.files_processed,
        files_removed: report.files_removed,
        files_failed: report.files_failed,
        total_original_bytes: report.total_original_bytes,
        total_bytes_written: report.total_bytes_written,
        elapsed_secs: report.elapsed.as_secs_f64(),
        reason: report.reason,
        failures: report
            .failures
            .into_iter()
            .map(|f| FileFailureDto {
                path: f.path,
                reason: f.reason,
            })
            .collect(),
    })
}

#[derive(Debug, Deserialize)]
pub struct CarveInput {
    pub case_id: String,
    pub evidence_id: String,
    pub examiner: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CarveOutput {
    pub case_id: String,
    pub evidence_id: String,
    pub source_sha256: String,
    pub carving_job_id: Option<String>,
    pub recovery_job_id: Option<String>,
    pub bytes_scanned: u64,
    pub scan_limit_reached: bool,
    pub candidate_limit_reached: bool,
    pub headers_dropped_by_limit: u64,
    pub candidates_found: u64,
    pub candidates_validated: u64,
    pub candidates_rejected: u64,
    pub artifacts: Vec<ArtifactSummaryDto>,
}

#[derive(Debug, Serialize)]
pub struct ArtifactSummaryDto {
    pub id: String,
    pub detected_type: String,
    pub source_offset: u64,
    pub source_length: u64,
    pub validation_state: String,
    pub confidence: String,
    pub artifact_sha256: String,
    pub artifact_path: Option<String>,
}

#[tauri::command]
pub fn carve_source(
    state: State<'_, AppState>,
    input: CarveInput,
) -> Result<CarveOutput, CommandError> {
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;
    carve_source_inner(&conn, &state.recovered_dir(), input)
}

fn carve_source_inner(
    conn: &rusqlite::Connection,
    recovered_dir: &std::path::Path,
    input: CarveInput,
) -> Result<CarveOutput, CommandError> {
    use kryvora_carving::{run_carving_job_for_evidence, ScanOptions};
    use kryvora_db::repo::{EvidenceRepository, SqliteEvidenceRepository};
    use kryvora_integrity::calculate_hash;
    use kryvora_provenance::{persist_recovery_result, ChainInputs};
    use kryvora_recovery::run_recovery_job_for_case;
    use std::io::{BufReader, Seek, SeekFrom};

    let case_uuid = parse_uuid(&input.case_id).ok_or_else(|| CommandError {
        kind: "invalid_input".into(),
        message: format!("bad case id: {}", input.case_id),
    })?;
    let case_id = kryvora_core::CaseId::from_uuid(case_uuid);
    let evidence_uuid = parse_uuid(&input.evidence_id).ok_or_else(|| CommandError {
        kind: "invalid_input".into(),
        message: format!("bad evidence id: {}", input.evidence_id),
    })?;
    let evidence_id = kryvora_core::EvidenceId::from_uuid(evidence_uuid);
    let evidence = SqliteEvidenceRepository::new(conn)
        .get(&evidence_id)?
        .ok_or_else(|| CommandError {
            kind: "not_found".into(),
            message: format!("evidence {evidence_id}"),
        })?;
    if evidence.case_id != case_id {
        return Err(CommandError {
            kind: "case_mismatch".into(),
            message: "selected evidence does not belong to the selected case".into(),
        });
    }
    if evidence.hash_algorithm != "sha256" {
        return Err(CommandError {
            kind: "integrity_unavailable".into(),
            message: format!("unsupported registered hash algorithm: {}", evidence.hash_algorithm),
        });
    }
    let (canonical_source, mut source_file) = open_evidence_source(&evidence.source_path)?;

    let hash = calculate_hash(&mut source_file)?;
    let size_bytes = hash.input_size();
    if size_bytes != evidence.size_bytes || hash.digest_hex() != evidence.hash_digest {
        kryvora_audit::append(
            conn,
            &kryvora_audit::EventDraft {
                event_type: kryvora_audit::EventType::IntegrityVerified,
                actor: input.examiner.clone(),
                object_id: Some(evidence_id.to_string()),
                job_id: None,
                details: serde_json::json!({
                    "case_id": case_id.to_string(),
                    "state": "changed",
                    "expected_size_bytes": evidence.size_bytes,
                    "actual_size_bytes": size_bytes,
                    "expected_sha256": evidence.hash_digest,
                    "actual_sha256": hash.digest_hex(),
                    "stage": "pre_scan",
                }),
            },
        )?;
        return Err(CommandError {
            kind: "integrity_mismatch".into(),
            message: "registered evidence changed before analysis; scan was not started".into(),
        });
    }
    if canonical_source.to_string_lossy() != evidence.source_path {
        return Err(CommandError {
            kind: "evidence_identity_changed".into(),
            message: "registered evidence path no longer resolves to the same canonical path".into(),
        });
    }

    source_file.seek(SeekFrom::Start(0))?;
    let carve_reader = BufReader::new(source_file.try_clone()?);
    let carve_report = run_carving_job_for_evidence(
        conn,
        carve_reader,
        ScanOptions {
            source_size_bytes: Some(size_bytes),
            ..ScanOptions::default()
        },
        input.examiner.clone(),
        Some(case_id),
        Some(evidence_id),
    )?
    .ok_or_else(|| CommandError {
        kind: "job_cancelled".into(),
        message: "carve job cancelled".into(),
    })?;

    source_file.seek(SeekFrom::Start(0))?;
    let bytes_scanned = carve_report.bytes_scanned;
    let scan_limit_reached = carve_report.scan_limit_reached;
    let candidate_limit_reached = carve_report.candidate_limit_reached;
    let headers_dropped_by_limit = carve_report.headers_dropped_by_limit;
    let candidates = carve_report.candidates;
    for candidate in &candidates {
        kryvora_audit::append(
            conn,
            &kryvora_audit::EventDraft {
                event_type: kryvora_audit::EventType::CandidateDetected,
                actor: input.examiner.clone(),
                object_id: Some(format!("CANDIDATE-{}-{}", candidate.offset, candidate.length)),
                job_id: carve_report.job_id,
                details: serde_json::json!({
                    "case_id": case_id.to_string(),
                    "evidence_id": evidence_id.to_string(),
                    "format": candidate.format,
                    "offset": candidate.offset,
                    "detected_length": candidate.length,
                    "signature_header_preview": candidate.header_preview,
                    "termination_reason": "footer_matched",
                }),
            },
        )?;
    }

    let recovery_report = run_recovery_job_for_case(
        conn,
        source_file.try_clone()?,
        candidates,
        Some(evidence_id),
        Some(case_id),
        input.examiner.clone(),
    )?
    .ok_or_else(|| CommandError {
        kind: "job_cancelled".into(),
        message: "recovery job cancelled".into(),
    })?;

    source_file.seek(SeekFrom::Start(0))?;
    let post_scan_hash = calculate_hash(&mut source_file)?;
    if post_scan_hash.input_size() != size_bytes || post_scan_hash.digest_hex() != hash.digest_hex()
    {
        return Err(CommandError {
            kind: "integrity_mismatch".into(),
            message: "evidence source changed during carving or recovery; results were discarded"
                .into(),
        });
    }

    let mut artifact_paths = Vec::with_capacity(recovery_report.results.len());
    for result in &recovery_report.results {
        match write_recovered_artifact(recovered_dir, &mut source_file, result) {
            Ok(path) => artifact_paths.push(path),
            Err(error) => {
                for path in artifact_paths.iter().flatten() {
                    let _ = std::fs::remove_file(path);
                }
                return Err(error);
            }
        }
    }

    source_file.seek(SeekFrom::Start(0))?;
    let final_source_hash = calculate_hash(&mut source_file)?;
    if final_source_hash.input_size() != size_bytes
        || final_source_hash.digest_hex() != hash.digest_hex()
    {
        for path in artifact_paths.iter().flatten() {
            let _ = std::fs::remove_file(path);
        }
        return Err(CommandError {
            kind: "integrity_mismatch".into(),
            message: "evidence changed while recovered outputs were being written".into(),
        });
    }

    let mut artifacts = Vec::new();
    for (result, artifact_path) in recovery_report.results.iter().zip(artifact_paths) {
        let candidate_object_id = format!(
            "CANDIDATE-{}-{}",
            result.source_offset, result.source_length
        );
        let inputs = ChainInputs {
            case_id,
            evidence_id,
            job_id: recovery_report.job_id,
            candidate_object_id,
            actor_note: Some("kryvora-app carve".into()),
            artifact_path: artifact_path.clone(),
        };
        if let Err(error) = persist_recovery_result(conn, &inputs, result) {
            if let Some(path) = artifact_path.as_deref() {
                let _ = std::fs::remove_file(path);
            }
            return Err(error.into());
        }
        artifacts.push(ArtifactSummaryDto {
            id: result.id.to_string(),
            detected_type: result.detected_type.clone(),
            source_offset: result.source_offset,
            source_length: result.source_length,
            validation_state: format!("{:?}", result.validation_state).to_lowercase(),
            confidence: format!("{:?}", result.confidence.level).to_lowercase(),
            artifact_sha256: result.artifact_sha256.clone(),
            artifact_path,
        });
    }

    Ok(CarveOutput {
        case_id: case_id.to_string(),
        evidence_id: evidence_id.to_string(),
        source_sha256: hash.digest_hex().to_string(),
        carving_job_id: carve_report.job_id.map(|id| id.to_string()),
        recovery_job_id: recovery_report.job_id.map(|id| id.to_string()),
        bytes_scanned,
        scan_limit_reached,
        candidate_limit_reached,
        headers_dropped_by_limit,
        candidates_found: recovery_report.candidates_considered,
        candidates_validated: recovery_report.candidates_validated,
        candidates_rejected: recovery_report.candidates_rejected,
        artifacts,
    })
}

fn write_recovered_artifact(
    recovered_dir: &std::path::Path,
    source: &mut std::fs::File,
    result: &kryvora_recovery::RecoveryResult,
) -> Result<Option<String>, CommandError> {
    use std::fs::{File, OpenOptions};
    use std::io::{BufReader, Read, Seek, SeekFrom};

    if result.validation_state != kryvora_core::ValidationState::Valid {
        return Ok(None);
    }
    let extension = match result.detected_type.as_str() {
        "jpeg" => "jpg",
        "png" => "png",
        "pdf" => "pdf",
        _ => return Ok(None),
    };

    std::fs::create_dir_all(recovered_dir)?;
    let metadata = std::fs::symlink_metadata(recovered_dir)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(CommandError {
            kind: "unsafe_target".into(),
            message: "recovered-artifact directory must be a real directory".into(),
        });
    }
    let canonical_dir = recovered_dir.canonicalize()?;
    let output_path = canonical_dir.join(format!("{}.{}", result.id, extension));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output_path)?;

    let write_result = (|| -> Result<(), CommandError> {
        source.seek(SeekFrom::Start(result.source_offset))?;
        let mut bounded_source = (&mut *source).take(result.source_length);
        let written = std::io::copy(&mut bounded_source, &mut output)?;
        if written != result.source_length {
            return Err(CommandError {
                kind: "candidate_read_failed".into(),
                message: "source ended before the validated candidate range was copied".into(),
            });
        }
        output.sync_all()?;
        drop(output);

        let mut recovered_file = File::open(&output_path)?;
        let output_hash = kryvora_integrity::calculate_hash(BufReader::new(&mut recovered_file))?;
        if output_hash.input_size() != result.source_length
            || output_hash.digest_hex() != result.artifact_sha256
        {
            return Err(CommandError {
                kind: "artifact_integrity_mismatch".into(),
                message: "written artifact does not match the validated candidate digest".into(),
            });
        }
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&output_path);
        return Err(error);
    }
    Ok(Some(output_path.display().to_string()))
}

fn verified_recovered_artifact_path(
    recovered_dir: &std::path::Path,
    artifact_path: Option<&str>,
    expected_sha256: &str,
    expected_size: u64,
) -> Option<String> {
    let artifact_path = std::path::Path::new(artifact_path?);
    let canonical_dir = recovered_dir.canonicalize().ok()?;
    let metadata = std::fs::symlink_metadata(artifact_path).ok()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return None;
    }
    let canonical_path = artifact_path.canonicalize().ok()?;
    if canonical_path.parent()? != canonical_dir {
        return None;
    }
    let mut file = std::fs::File::open(&canonical_path).ok()?;
    let hash = kryvora_integrity::calculate_hash(std::io::BufReader::new(&mut file)).ok()?;
    if hash.input_size() != expected_size || hash.digest_hex() != expected_sha256 {
        return None;
    }
    Some(canonical_path.display().to_string())
}

#[derive(Debug, Deserialize)]
pub struct GenerateReportInput {
    pub case_id: String,
    pub out_path: String,
}

#[derive(Debug, Serialize)]
pub struct GenerateReportOutput {
    pub report_id: String,
    pub path: String,
    pub byte_size: u64,
    pub sha256: String,
}

#[tauri::command]
pub fn generate_report(
    state: State<'_, AppState>,
    input: GenerateReportInput,
) -> Result<GenerateReportOutput, CommandError> {
    let db = state.current_db();
    let mut conn = kryvora_db::open(db)?;
    kryvora_db::apply_migrations(&mut conn)?;

    let uuid = parse_uuid(&input.case_id).ok_or_else(|| CommandError {
        kind: "invalid_input".into(),
        message: format!("bad case id: {}", input.case_id),
    })?;
    let case_id = kryvora_core::CaseId::from_uuid(uuid);
    let report_name = validate_report_name(&input.out_path)?;
    let reports_dir = state.reports_dir();
    std::fs::create_dir_all(&reports_dir)?;
    let report_dir_metadata = std::fs::symlink_metadata(&reports_dir)?;
    if !report_dir_metadata.is_dir() || report_dir_metadata.file_type().is_symlink() {
        return Err(CommandError {
            kind: "unsafe_target".into(),
            message: "report directory must be a real directory, not a symlink".into(),
        });
    }
    let report_path = reports_dir.join(report_name);

    let report = assemble_persisted_case_report(&conn, &state.recovered_dir(), &case_id)?;
    let generated = kryvora_report::generate_persisted_case_report(&report_path, &report)?;
    let report_id = persist_generated_case_report(&conn, case_id, &generated)?;

    Ok(GenerateReportOutput {
        report_id: report_id.to_string(),
        path: generated.path.display().to_string(),
        byte_size: generated.byte_size,
        sha256: generated.sha256,
    })
}

fn verify_report_file(
    reports_dir: &std::path::Path,
    path: &str,
    expected_sha256: &str,
    expected_size: u64,
) -> String {
    let Ok(root) = reports_dir.canonicalize() else {
        return "failed".into();
    };
    let candidate = std::path::Path::new(path);
    let metadata = match std::fs::symlink_metadata(candidate) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return "missing".into(),
        Err(_) => return "failed".into(),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return "failed".into();
    }
    let Ok(canonical) = candidate.canonicalize() else {
        return "failed".into();
    };
    if canonical.parent() != Some(root.as_path()) {
        return "failed".into();
    }
    let Ok(mut file) = std::fs::File::open(canonical) else {
        return "failed".into();
    };
    let Ok(hash) = kryvora_integrity::calculate_hash(std::io::BufReader::new(&mut file)) else {
        return "failed".into();
    };
    if hash.input_size() == expected_size && hash.digest_hex() == expected_sha256 {
        "intact".into()
    } else {
        "changed".into()
    }
}

fn assemble_persisted_case_report(
    conn: &rusqlite::Connection,
    recovered_dir: &std::path::Path,
    case_id: &kryvora_core::CaseId,
) -> Result<kryvora_report::PersistedCaseReport, CommandError> {
    use kryvora_db::repo::{
        CaseRepository, EvidenceRepository, JobRepository, RecoveryResultRepository,
        ProvenanceRepository, SqliteCaseRepository, SqliteEvidenceRepository,
        SqliteJobRepository, SqliteProvenanceRepository, SqliteRecoveryResultRepository,
    };
    use kryvora_report::{
        PersistedArtifact, PersistedAuditStatus, PersistedEvidence, PersistedJob,
    };

    let case = SqliteCaseRepository::new(conn)
        .get(case_id)?
        .ok_or_else(|| CommandError {
            kind: "not_found".into(),
            message: format!("case {case_id}"),
        })?;
    let evidence_rows = SqliteEvidenceRepository::new(conn).list_by_case(case_id)?;
    let mut evidence = Vec::with_capacity(evidence_rows.len());
    let mut artifacts = Vec::new();
    let recovery_repo = SqliteRecoveryResultRepository::new(conn);

    for row in evidence_rows {
        let (integrity_status, integrity_details) = match open_evidence_source(&row.source_path) {
            Ok((_, mut source)) => match kryvora_integrity::calculate_hash(&mut source) {
                Ok(actual)
                    if actual.input_size() == row.size_bytes
                        && actual.digest_hex() == row.hash_digest =>
                {
                    ("Intact".to_string(), "Current SHA-256 and size match registration.".to_string())
                }
                Ok(actual) => (
                    "Changed".to_string(),
                    format!(
                        "Registered size/hash differ from current size={} SHA-256={}",
                        actual.input_size(),
                        actual.digest_hex()
                    ),
                ),
                Err(error) => ("Failed".to_string(), error.to_string()),
            },
            Err(error) => ("Failed".to_string(), error.message),
        };
        evidence.push(PersistedEvidence {
            id: row.id.to_string(),
            source_path: row.source_path,
            size_bytes: row.size_bytes,
            sha256: row.hash_digest,
            integrity_status,
            integrity_details,
            registered_at: row.created_at,
        });

        for result in recovery_repo.list_by_evidence(&row.id)? {
            let validation_status = result.validation_state.clone();
            let verified_path = if validation_status == "valid" {
                verified_recovered_artifact_path(
                    recovered_dir,
                    result.artifact_path.as_deref(),
                    &result.artifact_sha256,
                    result.source_length,
                )
            } else {
                None
            };
            let job_link_valid = match result.job_id {
                Some(job_id) => SqliteJobRepository::new(conn)
                    .get(&job_id)?
                    .is_some_and(|job| {
                        job.case_id == Some(*case_id) && job.evidence_id == Some(row.id)
                    }),
                None => false,
            };
            let job_link_status = if job_link_valid {
                "valid"
            } else {
                "missing or mismatched"
            };
            let output_status = match (validation_status.as_str(), verified_path.as_deref()) {
                ("valid", Some(_)) => "present; SHA-256 verified",
                ("valid", None) => "missing, altered, or outside app-owned storage",
                ("partial", _) => "not created for partial validation",
                ("invalid", _) => "not created for invalid candidate",
                ("inconclusive", _) => "not created for inconclusive candidate",
                _ => "not created; validation status unavailable",
            };
            let provenance_repo = SqliteProvenanceRepository::new(conn);
            let (provenance, provenance_link_valid) = match result.provenance_id {
                Some(provenance_id) => match kryvora_provenance::ProvenanceGraph::new(conn)
                    .path_to_root(&provenance_id)
                {
                    Ok(path) => {
                        let root = path.elements.last();
                        let leaf = path.elements.first();
                        let node_links_valid = path.elements.iter().try_fold(true, |valid, element| {
                            Ok::<_, kryvora_core::Error>(
                                valid
                                    && provenance_repo
                                        .get(&element.id)?
                                        .is_some_and(|node| node.case_id == *case_id),
                            )
                        })?;
                        let valid = node_links_valid
                            && leaf.is_some_and(|node| {
                                node.kind == kryvora_provenance::NodeKind::Artifact
                                    && node.object_id == result.id.to_string()
                            })
                            && root.is_some_and(|node| {
                                node.kind == kryvora_provenance::NodeKind::Evidence
                                    && node.object_id == result.evidence_id.to_string()
                            });
                        (
                            path.elements
                                .into_iter()
                                .map(|element| format!("{:?}:{}", element.kind, element.object_id))
                                .collect(),
                            valid,
                        )
                    }
                    Err(error) => (vec![format!("provenance unavailable: {error}")], false),
                },
                None => (vec!["provenance ID unavailable".into()], false),
            };
            let provenance_link_status = if provenance_link_valid {
                "valid"
            } else {
                "missing or mismatched"
            };
            let linkage_valid = job_link_valid && provenance_link_valid;
            let linkage_status = format!(
                "job: {job_link_status}; provenance: {provenance_link_status}"
            );
            let recovery_status = match (
                validation_status.as_str(),
                verified_path.is_some(),
                linkage_valid,
            ) {
                ("valid", true, true) => "recovered file exists, hash matches, and links are valid",
                ("valid", _, false) => "linkage missing or mismatched; recovery is not asserted",
                ("valid", false, true) => "valid range recorded; recovered file unavailable",
                ("partial", _, _) => "partial; not successful recovery",
                ("invalid", _, _) => "rejected",
                ("inconclusive", _, _) => "inconclusive; not successful recovery",
                _ => "unknown; not successful recovery",
            };
            artifacts.push(PersistedArtifact {
                id: result.id.to_string(),
                evidence_id: result.evidence_id.to_string(),
                job_id: result.job_id.map(|id| id.to_string()),
                format: result.detected_type,
                offset: result.source_offset,
                length: result.source_length,
                validation_status,
                recovery_status: recovery_status.into(),
                linkage_status,
                confidence: result.confidence_level,
                confidence_rationale: result.confidence_reasons,
                sha256: result.artifact_sha256,
                output_path: verified_path,
                output_status: output_status.into(),
                validation_facts: result.validation_facts,
                provenance,
                created_at: result.created_at,
            });
        }
    }

    let jobs = SqliteJobRepository::new(conn)
        .list_by_case(case_id)?
        .into_iter()
        .map(|job| PersistedJob {
            id: job.id.to_string(),
            evidence_id: job.evidence_id.map(|id| id.to_string()),
            kind: job.job_type,
            state: job.state,
            progress: format!("{:.6}", job.progress),
            configuration: job.configuration,
            error: job.error_message,
            created_at: job.created_at,
            started_at: job.started_at,
            completed_at: job.completed_at,
        })
        .collect();

    let audit_chain = match kryvora_audit::verify_chain(conn) {
        Ok(kryvora_audit::ChainStatus::Empty) => PersistedAuditStatus {
            state: "empty".into(),
            verified_length: 0,
            first_bad_sequence: None,
            reason: None,
        },
        Ok(kryvora_audit::ChainStatus::Intact { length }) => PersistedAuditStatus {
            state: "intact".into(),
            verified_length: length,
            first_bad_sequence: None,
            reason: None,
        },
        Ok(kryvora_audit::ChainStatus::Broken {
            first_bad_sequence,
            reason,
        }) => PersistedAuditStatus {
            state: "broken".into(),
            verified_length: first_bad_sequence,
            first_bad_sequence: Some(first_bad_sequence),
            reason: Some(format!("{reason:?}")),
        },
        Err(error) => PersistedAuditStatus {
            state: "verification_error".into(),
            verified_length: 0,
            first_bad_sequence: None,
            reason: Some(error.to_string()),
        },
    };

    let mut limitations = vec![
        "The local audit hash chain has no external signature or independent anchor.".into(),
        "Audit-chain status is checked before this report is appended to the chain.".into(),
        "Signature carving supports contiguous JPEG, PNG, and PDF candidates only.".into(),
        "Filesystem-aware deleted-file recovery and fragmented-file reconstruction are unsupported.".into(),
        "A detected signature is not proof of recovery; partial and inconclusive candidates are not successful recoveries.".into(),
        "This report makes no claim of legal admissibility or forensic certification.".into(),
    ];
    if evidence.is_empty() {
        limitations.push("No evidence records are linked to this case.".into());
    }
    if audit_chain.state != "intact" {
        limitations.push(format!(
            "Audit-chain verification status is {}.",
            audit_chain.state
        ));
    }

    Ok(kryvora_report::PersistedCaseReport {
        case_id: case.id.to_string(),
        case_title: case.title,
        examiner: case.examiner,
        case_created_at: case.created_at,
        evidence,
        jobs,
        artifacts,
        audit_chain,
        limitations,
    })
}

fn persist_generated_case_report(
    conn: &rusqlite::Connection,
    case_id: kryvora_core::CaseId,
    generated: &kryvora_report::GeneratedReport,
) -> Result<kryvora_core::ReportId, CommandError> {
    use kryvora_db::repo::{ReportKind, ReportRepository, SqliteReportRepository};

    conn.execute_batch("SAVEPOINT kryvora_report_metadata")?;
    let result = (|| -> Result<kryvora_core::ReportId, CommandError> {
        let report_id = SqliteReportRepository::new(conn).insert(&kryvora_db::repo::NewReport {
            case_id: Some(case_id),
            job_id: None,
            kind: ReportKind::Recovery,
            path: generated.path.display().to_string(),
            sha256: generated.sha256.clone(),
            byte_size: generated.byte_size,
        })?;
        kryvora_audit::append(
            conn,
            &kryvora_audit::EventDraft {
                event_type: kryvora_audit::EventType::ReportGenerated,
                actor: None,
                object_id: Some(report_id.to_string()),
                job_id: None,
                details: serde_json::json!({
                    "case_id": case_id.to_string(),
                    "path": generated.path.display().to_string(),
                    "sha256": generated.sha256,
                    "byte_size": generated.byte_size,
                }),
            },
        )?;
        Ok(report_id)
    })();
    match result {
        Ok(report_id) => {
            conn.execute_batch("RELEASE SAVEPOINT kryvora_report_metadata")?;
            Ok(report_id)
        }
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT kryvora_report_metadata; RELEASE SAVEPOINT kryvora_report_metadata",
            );
            let _ = std::fs::remove_file(&generated.path);
            Err(error)
        }
    }
}

fn validate_report_name(name: &str) -> Result<&str, CommandError> {
    let valid = !name.is_empty()
        && name.len() <= 120
        && name.to_ascii_lowercase().ends_with(".html")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
    let base = name.get(..name.len().saturating_sub(5));
    let reserved = base.is_some_and(|base| {
        matches!(
            base.to_ascii_uppercase().as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        )
    });
    if valid && !reserved {
        Ok(name)
    } else {
        Err(CommandError {
            kind: "invalid_input".into(),
            message: "report name must be a simple .html filename without path components".into(),
        })
    }
}

// ---------- helpers ----------

fn parse_uuid(s: &str) -> Option<uuid::Uuid> {
    if s.len() > 64 {
        return None;
    }
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

fn finish_savepoint<T>(
    conn: &rusqlite::Connection,
    name: &str,
    result: Result<T, CommandError>,
) -> Result<T, CommandError> {
    match result {
        Ok(value) => {
            conn.execute_batch(&format!("RELEASE SAVEPOINT {name}"))?;
            Ok(value)
        }
        Err(error) => {
            let _ = conn.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {name}; RELEASE SAVEPOINT {name}"
            ));
            Err(error)
        }
    }
}

fn parse_optional_case_id(value: Option<&str>) -> Result<Option<kryvora_core::CaseId>, CommandError> {
    match value.filter(|value| !value.trim().is_empty()) {
        None => Ok(None),
        Some(value) => parse_uuid(value)
            .map(kryvora_core::CaseId::from_uuid)
            .map(Some)
            .ok_or_else(|| CommandError {
                kind: "invalid_input".into(),
                message: "case id is invalid".into(),
            }),
    }
}

#[cfg(test)]
mod report_path_tests {
    use super::validate_report_name;

    #[test]
    fn only_safe_html_filenames_are_accepted() {
        assert_eq!(
            validate_report_name("case-report.html").unwrap(),
            "case-report.html"
        );
        for invalid in [
            "",
            "../outside.html",
            "C:\\outside.html",
            "report.html:stream",
            "CON.html",
            "report.txt",
        ] {
            assert!(validate_report_name(invalid).is_err(), "{invalid}");
        }
    }
}

#[cfg(test)]
mod sanitization_confirmation_tests {
    use super::validate_sanitization_confirmation;
    use kryvora_policy::{Confirmation, SafetyAssessment};

    fn assessment(confirmation: Confirmation) -> SafetyAssessment {
        SafetyAssessment {
            allowed: true,
            destructive: true,
            requires_elevation: false,
            warnings: Vec::new(),
            confirmations: vec![confirmation],
        }
    }

    #[test]
    fn file_sanitization_requires_explicit_boolean_confirmation() {
        let plan = assessment(Confirmation::Basic {
            prompt: "confirm file".into(),
        });
        assert!(validate_sanitization_confirmation(&plan, false, None).is_err());
        assert!(validate_sanitization_confirmation(&plan, true, None).is_ok());
    }

    #[test]
    fn directory_sanitization_requires_exact_backend_phrase() {
        let plan = assessment(Confirmation::TypedPhrase {
            prompt: "type ERASE".into(),
            phrase: "ERASE".into(),
        });
        assert!(validate_sanitization_confirmation(&plan, true, Some("SANITIZE")).is_err());
        assert!(validate_sanitization_confirmation(&plan, true, Some("erase")).is_err());
        assert!(validate_sanitization_confirmation(&plan, true, Some("ERASE")).is_ok());
    }
}

#[cfg(test)]
mod sanitization_command_tests {
    use super::*;
    use kryvora_audit::ChainStatus;
    use kryvora_core::SanitizationOperationState;
    use kryvora_db::repo::{
        NewSanitizationOperation, SanitizationOperationRepository,
        SqliteSanitizationOperationRepository,
    };
    use kryvora_storage::{TargetIdentity, TargetKind};
    use rusqlite::Connection;
    use std::path::{Path, PathBuf};

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("kryvora-sanitize-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        root
    }

    fn open_test_db(path: &Path) -> Connection {
        let mut connection = kryvora_db::open(path).unwrap();
        kryvora_db::apply_migrations(&mut connection).unwrap();
        connection
    }

    fn create_plan(
        state: &AppState,
        db_path: &Path,
        target_path: &Path,
        kind: TargetKind,
        confirmation_kind: &str,
    ) -> String {
        let profile = inspect_sanitize_profile(target_path, kind).unwrap();
        let identity = TargetIdentity::capture(&profile.path, kind).unwrap();
        let connection = open_test_db(db_path);
        let repo = SqliteSanitizationOperationRepository::new(&connection);
        let operation_id = repo
            .insert(&NewSanitizationOperation {
                case_id: None,
                target_path: profile.path.clone(),
                target_kind: kind.as_str().into(),
                method: "random_overwrite".into(),
                target_identity_verified: false,
                confirmation_kind: confirmation_kind.into(),
                confirmation_validated: false,
                actor: None,
            })
            .unwrap();
        kryvora_audit::append(
            &connection,
            &kryvora_audit::EventDraft {
                event_type: kryvora_audit::EventType::SanitizationPlanned,
                actor: None,
                object_id: Some(operation_id.to_string()),
                job_id: None,
                details: serde_json::json!({ "target_path": profile.path }),
            },
        )
        .unwrap();
        state
            .issue_sanitization_plan(
                PathBuf::from(profile.path),
                kind,
                identity,
                operation_id,
                None,
                None,
            )
            .unwrap()
    }

    #[test]
    fn missing_plan_is_refused_and_audited_without_writing_target() {
        let root = temp_root();
        let target = root.join("target.bin");
        std::fs::write(&target, b"preserve me").unwrap();
        let db_path = root.join("kryvora.db");
        let state = AppState::new(db_path.clone());
        let before = std::fs::read(&target).unwrap();
        let input = SanitizeInput {
            target: target.display().to_string(),
            confirm: true,
            plan_id: None,
            typed_confirmation: None,
            case_id: None,
            actor: None,
        };

        let error = begin_sanitization(&state, &input, TargetKind::File).unwrap_err();
        assert_eq!(error.kind, "policy_violation");
        assert_eq!(std::fs::read(&target).unwrap(), before);

        let connection = open_test_db(&db_path);
        let rows = SqliteSanitizationOperationRepository::new(&connection)
            .list_recent(10)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, SanitizationOperationState::Refused);
        assert_eq!(
            kryvora_audit::verify_chain(&connection).unwrap(),
            ChainStatus::Intact { length: 1 }
        );
        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wrong_directory_phrase_is_refused_and_audited_without_writing() {
        let root = temp_root();
        let target = root.join("nested");
        std::fs::create_dir(&target).unwrap();
        let file = target.join("target.bin");
        std::fs::write(&file, b"preserve me").unwrap();
        let db_path = root.join("kryvora.db");
        let state = AppState::new(db_path.clone());
        let plan_id = create_plan(&state, &db_path, &target, TargetKind::Directory, "typed_phrase");
        let before = std::fs::read(&file).unwrap();
        let input = SanitizeInput {
            target: target.display().to_string(),
            confirm: true,
            plan_id: Some(plan_id),
            typed_confirmation: Some("SANITIZE".into()),
            case_id: None,
            actor: None,
        };

        let error = begin_sanitization(&state, &input, TargetKind::Directory).unwrap_err();
        assert_eq!(error.kind, "policy_violation");
        assert_eq!(std::fs::read(&file).unwrap(), before);

        let connection = open_test_db(&db_path);
        let rows = SqliteSanitizationOperationRepository::new(&connection)
            .list_recent(10)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, SanitizationOperationState::Refused);
        assert!(!rows[0].confirmation_validated);
        assert_eq!(
            kryvora_audit::verify_chain(&connection).unwrap(),
            ChainStatus::Intact { length: 2 }
        );
        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn alternate_path_cannot_reuse_an_inspected_plan() {
        let root = temp_root();
        let original = root.join("original.bin");
        let alternate = root.join("alternate.bin");
        std::fs::write(&original, b"original").unwrap();
        std::fs::write(&alternate, b"alternate").unwrap();
        let db_path = root.join("kryvora.db");
        let state = AppState::new(db_path.clone());
        let plan_id = create_plan(&state, &db_path, &original, TargetKind::File, "basic");
        let before = std::fs::read(&alternate).unwrap();
        let input = SanitizeInput {
            target: alternate.display().to_string(),
            confirm: true,
            plan_id: Some(plan_id),
            typed_confirmation: None,
            case_id: None,
            actor: None,
        };

        let error = begin_sanitization(&state, &input, TargetKind::File).unwrap_err();
        assert_eq!(error.kind, "policy_violation");
        assert_eq!(std::fs::read(&alternate).unwrap(), before);
        let rows = SqliteSanitizationOperationRepository::new(&open_test_db(&db_path))
            .list_recent(10)
            .unwrap();
        assert_eq!(rows[0].state, SanitizationOperationState::Refused);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod carving_command_tests {
    use super::{
        assemble_persisted_case_report, carve_source_inner, persist_generated_case_report,
        verify_report_file, write_recovered_artifact, CarveInput,
    };
    use kryvora_db::repo::{
        CaseRepository, EvidenceRepository, JobRepository, NewCase, NewEvidence, NewJob,
        SqliteCaseRepository, SqliteEvidenceRepository, SqliteJobRepository,
    };
    use rusqlite::Connection;
    use std::path::Path;

    fn temp_root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "kryvora-carve-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        root
    }

    fn open_db(path: &Path) -> Connection {
        let mut connection = kryvora_db::open(path).unwrap();
        kryvora_db::apply_migrations(&mut connection).unwrap();
        connection
    }

    fn register_source(
        connection: &Connection,
        source_path: &Path,
    ) -> (kryvora_core::CaseId, kryvora_core::EvidenceId) {
        use std::io::BufReader;

        let case_id = SqliteCaseRepository::new(connection)
            .insert(&NewCase {
                title: "Synthetic analysis case".into(),
                examiner: Some("test examiner".into()),
                notes: None,
            })
            .unwrap();
        let source_file = std::fs::File::open(source_path).unwrap();
        let hash = kryvora_integrity::calculate_hash(BufReader::new(source_file)).unwrap();
        let evidence_id = SqliteEvidenceRepository::new(connection)
            .insert(&NewEvidence {
                case_id,
                source_type: "file".into(),
                source_path: source_path.canonicalize().unwrap().display().to_string(),
                size_bytes: hash.input_size(),
                hash_algorithm: "sha256".into(),
                hash_digest: hash.digest_hex().into(),
                integrity_state: "verified".into(),
                read_only: true,
                tool_version: "test".into(),
                acquisition_metadata: None,
                notes: None,
            })
            .unwrap();
        (case_id, evidence_id)
    }

    fn synthetic_png() -> Vec<u8> {
        let mut bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&[
            0, 0, 0, 10, 0, 0, 0, 10, 8, 6, 0, 0, 0,
        ]);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(b"IEND");
        bytes.extend_from_slice(&[0xAE, 0x42, 0x60, 0x82]);
        bytes
    }

    #[test]
    fn carving_reuses_registered_case_and_evidence_without_modifying_source() {
        let root = temp_root();
        let database = root.join("kryvora.db");
        let source = root.join("evidence.bin");
        let original = b"synthetic evidence without supported signatures";
        std::fs::write(&source, original).unwrap();
        let connection = open_db(&database);
        let (case_id, evidence_id) = register_source(&connection, &source);

        let recovered_dir = root.join("recovered");
        let output = carve_source_inner(
            &connection,
            &recovered_dir,
            CarveInput {
                case_id: case_id.to_string(),
                evidence_id: evidence_id.to_string(),
                examiner: Some("test examiner".into()),
            },
        )
        .unwrap();

        assert_eq!(output.case_id, case_id.to_string());
        assert_eq!(output.evidence_id, evidence_id.to_string());
        assert_eq!(std::fs::read(&source).unwrap(), original);
        let case_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM cases", [], |row| row.get(0))
            .unwrap();
        let evidence_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM evidence", [], |row| row.get(0))
            .unwrap();
        assert_eq!(case_count, 1);
        assert_eq!(evidence_count, 1);
        let jobs = SqliteJobRepository::new(&connection)
            .list_by_state("succeeded", 10)
            .unwrap();
        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().all(|job| job.case_id == Some(case_id)));
        assert!(jobs.iter().all(|job| job.evidence_id == Some(evidence_id)));
        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn carving_rejects_wrong_case_and_changed_registered_source() {
        let root = temp_root();
        let database = root.join("kryvora.db");
        let source = root.join("evidence.bin");
        std::fs::write(&source, b"original evidence bytes").unwrap();
        let connection = open_db(&database);
        let (case_id, evidence_id) = register_source(&connection, &source);
        let other_case_id = SqliteCaseRepository::new(&connection)
            .insert(&NewCase {
                title: "Other case".into(),
                examiner: None,
                notes: None,
            })
            .unwrap();

        let recovered_dir = root.join("recovered");
        let mismatch = carve_source_inner(
            &connection,
            &recovered_dir,
            CarveInput {
                case_id: other_case_id.to_string(),
                evidence_id: evidence_id.to_string(),
                examiner: None,
            },
        )
        .unwrap_err();
        assert_eq!(mismatch.kind, "case_mismatch");

        std::fs::write(&source, b"changed evidence bytes").unwrap();
        let changed_bytes = std::fs::read(&source).unwrap();
        let changed = carve_source_inner(
            &connection,
            &recovered_dir,
            CarveInput {
                case_id: case_id.to_string(),
                evidence_id: evidence_id.to_string(),
                examiner: None,
            },
        )
        .unwrap_err();
        assert_eq!(changed.kind, "integrity_mismatch");
        assert_eq!(std::fs::read(&source).unwrap(), changed_bytes);

        let job_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(job_count, 0);
        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn valid_candidate_is_written_and_persisted_without_modifying_evidence() {
        use kryvora_db::repo::{RecoveryResultRepository, SqliteRecoveryResultRepository};

        let root = temp_root();
        let database = root.join("kryvora.db");
        let source = root.join("evidence.bin");
        let original = synthetic_png();
        std::fs::write(&source, &original).unwrap();
        let connection = open_db(&database);
        let (case_id, evidence_id) = register_source(&connection, &source);
        let recovered_dir = root.join("recovered");

        let output = carve_source_inner(
            &connection,
            &recovered_dir,
            CarveInput {
                case_id: case_id.to_string(),
                evidence_id: evidence_id.to_string(),
                examiner: Some("test examiner".into()),
            },
        )
        .unwrap();

        assert_eq!(output.candidates_validated, 1);
        assert_eq!(output.artifacts.len(), 1);
        let artifact_path = output.artifacts[0]
            .artifact_path
            .as_ref()
            .expect("fully validated PNG should have an output file");
        assert_eq!(std::fs::read(artifact_path).unwrap(), original);
        assert_eq!(std::fs::read(&source).unwrap(), original);

        let rows = SqliteRecoveryResultRepository::new(&connection)
            .list_by_evidence(&evidence_id)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].artifact_path.as_deref(), Some(artifact_path.as_str()));
        assert_eq!(rows[0].artifact_sha256, output.artifacts[0].artifact_sha256);
        assert_eq!(rows[0].validation_state, "valid");
        assert!(rows[0].job_id.is_some());
        assert!(rows[0].provenance_id.is_some());

        let persisted = assemble_persisted_case_report(&connection, &recovered_dir, &case_id)
            .unwrap();
        assert_eq!(persisted.case_id, case_id.to_string());
        assert_eq!(persisted.evidence[0].integrity_status, "Intact");
        assert!(persisted.artifacts[0].output_path.is_some());
        assert!(persisted.artifacts[0]
            .recovery_status
            .contains("hash matches"), "{}; {:?}", persisted.artifacts[0].linkage_status, persisted.artifacts[0].provenance);

        std::fs::write(artifact_path, b"altered recovered output").unwrap();
        let altered = assemble_persisted_case_report(&connection, &recovered_dir, &case_id)
            .unwrap();
        assert!(altered.artifacts[0].output_path.is_none());
        assert!(altered.artifacts[0].output_status.contains("altered"));

        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_isolate_cases_and_disclose_missing_evidence_and_partial_results() {
        use kryvora_db::repo::{
            NewRecoveryResult, RecoveryResultRepository, SqliteRecoveryResultRepository,
        };

        let root = temp_root();
        let connection = open_db(&root.join("kryvora.db"));
        let source = root.join("evidence.bin");
        std::fs::write(&source, b"registered source bytes").unwrap();
        let (case_id, evidence_id) = register_source(&connection, &source);
        let other_case = SqliteCaseRepository::new(&connection)
            .insert(&NewCase {
                title: "No evidence case".into(),
                examiner: None,
                notes: None,
            })
            .unwrap();
        let other_case_job = SqliteJobRepository::new(&connection)
            .insert(&NewJob {
                case_id: Some(other_case),
                evidence_id: None,
                job_type: "recover".into(),
                configuration: None,
            })
            .unwrap();

        let unrelated = assemble_persisted_case_report(
            &connection,
            &root.join("recovered"),
            &other_case,
        )
        .unwrap();
        assert!(unrelated.evidence.is_empty());
        assert!(unrelated.artifacts.is_empty());
        assert!(unrelated
            .limitations
            .iter()
            .any(|item| item.contains("No evidence records")));

        SqliteRecoveryResultRepository::new(&connection)
            .insert(&NewRecoveryResult {
                id: None,
                evidence_id,
                job_id: Some(other_case_job),
                provenance_id: None,
                source_offset: 0,
                source_length: 10,
                detected_type: "png".into(),
                category: "image".into(),
                validation_state: "partial".into(),
                confidence_level: "uncertain".into(),
                confidence_reasons: r#"{"level":"uncertain","reasons":[]}"#.into(),
                recovery_method: "signature_carving".into(),
                reconstruction_state: "contiguous".into(),
                artifact_sha256: "a".repeat(64),
                artifact_path: None,
                validation_facts: r#"{"check":"partial"}"#.into(),
            })
            .unwrap();

        let linked = assemble_persisted_case_report(
            &connection,
            &root.join("recovered"),
            &case_id,
        )
        .unwrap();
        assert_eq!(linked.artifacts.len(), 1);
        assert_eq!(linked.artifacts[0].validation_status, "partial");
        assert!(linked.artifacts[0]
            .recovery_status
            .contains("not successful recovery"));
        assert!(linked.artifacts[0]
            .linkage_status
            .contains("job: missing or mismatched"));
        assert!(linked.artifacts[0].output_path.is_none());

        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_expose_audit_tampering_and_persist_report_digest() {
        use kryvora_db::repo::{ReportRepository, SqliteReportRepository};

        let root = temp_root();
        let database = root.join("kryvora.db");
        let connection = open_db(&database);
        let case_id = SqliteCaseRepository::new(&connection)
            .insert(&NewCase {
                title: "Audit report case".into(),
                examiner: None,
                notes: None,
            })
            .unwrap();
        kryvora_audit::append(
            &connection,
            &kryvora_audit::EventDraft {
                event_type: kryvora_audit::EventType::CaseCreated,
                actor: None,
                object_id: Some(case_id.to_string()),
                job_id: None,
                details: serde_json::json!({"case_id": case_id.to_string()}),
            },
        )
        .unwrap();

        let initial = assemble_persisted_case_report(
            &connection,
            &root.join("recovered"),
            &case_id,
        )
        .unwrap();
        assert_eq!(initial.audit_chain.state, "intact");
        let report_path = root.join("reports").join("case.html");
        std::fs::create_dir(report_path.parent().unwrap()).unwrap();
        let generated = kryvora_report::generate_persisted_case_report(&report_path, &initial)
            .unwrap();
        let report_id =
            persist_generated_case_report(&connection, case_id, &generated).unwrap();
        let stored = SqliteReportRepository::new(&connection)
            .get(&report_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.sha256, generated.sha256);
        assert_eq!(stored.byte_size, generated.byte_size);
        assert_eq!(kryvora_audit::verify_chain(&connection).unwrap(), kryvora_audit::ChainStatus::Intact { length: 2 });

        connection
            .execute(
                "UPDATE audit_events SET current_hash = ?1 WHERE sequence = 0",
                rusqlite::params!["0".repeat(64)],
            )
            .unwrap();
        let tampered = assemble_persisted_case_report(
            &connection,
            &root.join("recovered"),
            &case_id,
        )
        .unwrap();
        assert_eq!(tampered.audit_chain.state, "broken");
        assert_eq!(tampered.audit_chain.first_bad_sequence, Some(0));
        assert!(tampered
            .limitations
            .iter()
            .any(|item| item.contains("verification status is broken")));
        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovered_artifact_creation_never_overwrites_an_existing_path() {
        let root = temp_root();
        let recovered_dir = root.join("recovered");
        std::fs::create_dir(&recovered_dir).unwrap();
        let bytes = synthetic_png();
        let hash = kryvora_integrity::calculate_hash(std::io::Cursor::new(&bytes)).unwrap();
        let result = kryvora_recovery::RecoveryResult {
            id: kryvora_core::RecoveryResultId::new(),
            evidence_id: None,
            source_offset: 0,
            source_length: bytes.len() as u64,
            detected_type: "png".into(),
            category: kryvora_recovery::ArtifactCategory::Image,
            validation_state: kryvora_core::ValidationState::Valid,
            confidence: kryvora_recovery::ConfidenceAssessment::all_satisfied(),
            recovery_method: kryvora_recovery::RecoveryMethod::SignatureCarving,
            reconstruction_state: kryvora_core::ReconstructionState::Contiguous,
            artifact_sha256: hash.digest_hex().into(),
            created_at: time::OffsetDateTime::now_utc(),
            job_id: None,
            provenance_id: None,
            report_id: None,
            validation_facts: serde_json::json!({}),
        };
        let existing_path = recovered_dir.join(format!("{}.png", result.id));
        std::fs::write(&existing_path, b"keep existing file").unwrap();
        let before = std::fs::read(&existing_path).unwrap();

        let error = write_recovered_artifact(
            &recovered_dir,
            &mut std::fs::File::open({
                let source_path = root.join("source.bin");
                std::fs::write(&source_path, &bytes).unwrap();
                source_path
            })
            .unwrap(),
            &result,
        )
        .unwrap_err();

        assert_eq!(error.kind, "io_error");
        assert_eq!(std::fs::read(existing_path).unwrap(), before);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_output_hash_verification_removes_the_created_file() {
        let root = temp_root();
        let recovered_dir = root.join("recovered");
        let bytes = synthetic_png();
        let source_path = root.join("source.bin");
        std::fs::write(&source_path, &bytes).unwrap();
        let result = kryvora_recovery::RecoveryResult {
            id: kryvora_core::RecoveryResultId::new(),
            evidence_id: None,
            source_offset: 0,
            source_length: bytes.len() as u64,
            detected_type: "png".into(),
            category: kryvora_recovery::ArtifactCategory::Image,
            validation_state: kryvora_core::ValidationState::Valid,
            confidence: kryvora_recovery::ConfidenceAssessment::all_satisfied(),
            recovery_method: kryvora_recovery::RecoveryMethod::SignatureCarving,
            reconstruction_state: kryvora_core::ReconstructionState::Contiguous,
            artifact_sha256: "0".repeat(64),
            created_at: time::OffsetDateTime::now_utc(),
            job_id: None,
            provenance_id: None,
            report_id: None,
            validation_facts: serde_json::json!({}),
        };
        let expected_path = recovered_dir.join(format!("{}.png", result.id));

        let error = write_recovered_artifact(
            &recovered_dir,
            &mut std::fs::File::open(source_path).unwrap(),
            &result,
        )
        .unwrap_err();

        assert_eq!(error.kind, "artifact_integrity_mismatch");
        assert!(!expected_path.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn report_file_verification_distinguishes_intact_changed_missing_and_external() {
        let root = temp_root();
        let reports_dir = root.join("reports");
        std::fs::create_dir(&reports_dir).unwrap();
        let report_path = reports_dir.join("report.html");
        let original = b"persisted report contents";
        std::fs::write(&report_path, original).unwrap();
        let hash = kryvora_integrity::calculate_hash(std::io::Cursor::new(original)).unwrap();
        let digest = hash.digest_hex().to_string();

        assert_eq!(
            verify_report_file(&reports_dir, &report_path.display().to_string(), &digest, original.len() as u64),
            "intact"
        );
        std::fs::write(&report_path, b"altered report").unwrap();
        assert_eq!(
            verify_report_file(&reports_dir, &report_path.display().to_string(), &digest, original.len() as u64),
            "changed"
        );
        std::fs::remove_file(&report_path).unwrap();
        assert_eq!(
            verify_report_file(&reports_dir, &report_path.display().to_string(), &digest, original.len() as u64),
            "missing"
        );
        let external_path = root.join("outside.html");
        std::fs::write(&external_path, original).unwrap();
        assert_eq!(
            verify_report_file(&reports_dir, &external_path.display().to_string(), &digest, original.len() as u64),
            "failed"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod command_boundary_tests {
    use super::{create_case_inner, parse_uuid, register_evidence_inner, CreateCaseInput, RegisterEvidenceInput};
    use kryvora_db::repo::{CaseRepository, NewCase, SqliteCaseRepository};
    use rusqlite::Connection;
    use std::path::{Path, PathBuf};

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("kryvora-boundary-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        root
    }

    fn open_db(path: &Path) -> Connection {
        let mut connection = kryvora_db::open(path).unwrap();
        kryvora_db::apply_migrations(&mut connection).unwrap();
        connection
    }

    #[test]
    fn case_row_rolls_back_when_audit_append_fails() {
        let root = temp_root();
        let connection = open_db(&root.join("db.sqlite"));
        connection.execute_batch(
            "CREATE TRIGGER block_case_audit BEFORE INSERT ON audit_events \
             WHEN NEW.event_type = 'case_created' BEGIN \
             SELECT RAISE(ABORT, 'injected audit failure'); END;",
        ).unwrap();

        let error = create_case_inner(
            &connection,
            CreateCaseInput { title: "atomic case".into(), examiner: None },
        ).unwrap_err();
        assert!(error.message.contains("injected audit failure"), "{error:?}");
        let count: i64 = connection.query_row("SELECT COUNT(*) FROM cases", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0);
        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn evidence_and_provenance_roll_back_when_audit_append_fails() {
        let root = temp_root();
        let connection = open_db(&root.join("db.sqlite"));
        let case_id = SqliteCaseRepository::new(&connection).insert(&NewCase {
            title: "evidence case".into(), examiner: None, notes: None,
        }).unwrap();
        connection.execute_batch(
            "CREATE TRIGGER block_evidence_audit BEFORE INSERT ON audit_events \
             WHEN NEW.event_type = 'evidence_registered' BEGIN \
             SELECT RAISE(ABORT, 'injected audit failure'); END;",
        ).unwrap();
        let source = root.join("evidence.bin");
        let original = b"read-only test evidence";
        std::fs::write(&source, original).unwrap();

        let error = register_evidence_inner(
            &connection,
            RegisterEvidenceInput {
                case_id: case_id.to_string(),
                path: source.display().to_string(),
                notes: None,
                actor: None,
            },
        ).unwrap_err();
        assert!(error.message.contains("injected audit failure"), "{error:?}");
        let evidence_count: i64 = connection.query_row("SELECT COUNT(*) FROM evidence", [], |row| row.get(0)).unwrap();
        let provenance_count: i64 = connection.query_row("SELECT COUNT(*) FROM provenance_nodes", [], |row| row.get(0)).unwrap();
        assert_eq!(evidence_count, 0);
        assert_eq!(provenance_count, 0);
        assert_eq!(std::fs::read(&source).unwrap(), original);
        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn oversized_case_text_and_identifier_inputs_are_rejected() {
        let root = temp_root();
        let connection = open_db(&root.join("db.sqlite"));
        let error = create_case_inner(
            &connection,
            CreateCaseInput { title: "x".repeat(257), examiner: None },
        ).unwrap_err();
        assert_eq!(error.kind, "invalid_input");
        assert!(parse_uuid(&"a".repeat(65)).is_none());
        let case_id = SqliteCaseRepository::new(&connection).insert(&NewCase {
            title: "bounded evidence inputs".into(), examiner: None, notes: None,
        }).unwrap();
        for input in [
            RegisterEvidenceInput {
                case_id: case_id.to_string(),
                path: "unused".into(),
                notes: Some("n".repeat(4_097)),
                actor: None,
            },
            RegisterEvidenceInput {
                case_id: case_id.to_string(),
                path: "p".repeat(32_769),
                notes: None,
                actor: None,
            },
        ] {
            let error = register_evidence_inner(&connection, input).unwrap_err();
            assert_eq!(error.kind, "invalid_input");
        }
        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }
}
