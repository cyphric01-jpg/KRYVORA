// file: frontend/src/api.ts
// KRYVORA frontend API. Every function calls a real Tauri command.

import { invoke } from "@tauri-apps/api/core";

export interface Case {
  id: string;
  title: string;
  examiner: string | null;
  created_at: string;
}

export interface Evidence {
  id: string;
  case_id: string;
  source_path: string;
  size_bytes: number;
  hash_digest: string;
  integrity_state: string;
}

export interface EvidenceVerification {
  evidence_id: string;
  state: string;
  expected_hash: string;
  actual_hash: string;
  expected_size_bytes: number;
  actual_size_bytes: number;
}

export interface Job {
  id: string;
  job_type: string;
  state: string;
  progress: number;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
}

export interface AuditEvent {
  id: string;
  sequence: number;
  event_type: string;
  actor: string | null;
  object_id: string | null;
  details: string;
  created_at: string;
  previous_hash: string;
  current_hash: string;
}

export interface Report {
  id: string;
  case_id: string | null;
  kind: string;
  path: string;
  sha256: string;
  byte_size: number;
  created_at: string;
  integrity_status: "intact" | "changed" | "missing" | "failed";
}

export interface ChainStatus {
  state: "empty" | "intact" | "broken";
  length: number;
  first_bad_sequence: number | null;
  reason: string | null;
}

export type TargetKind = "file" | "directory" | "filesystem" | "block_device" | "unknown";

export interface TargetWarning {
  kind: string;
  display?: string;
  reason?: string;
}

export interface TargetProfile {
  kind: TargetKind;
  path: string;
  display: string;
  size_bytes: number | null;
  read_only: boolean;
  removable: boolean | null;
  device_identity: string | null;
  filesystem: {
    fs_type: string;
    mount_point: string;
    writable: boolean | null;
  } | null;
  warnings: TargetWarning[];
}

export interface SafetyAssessment {
  allowed: boolean;
  destructive: boolean;
  requires_elevation: boolean;
  warnings: Array<Record<string, unknown>>;
  confirmations: Array<Record<string, unknown>>;
}

export interface SanitizeTargetPreview {
  operation_id: string;
  plan_id: string;
  profile: TargetProfile;
  assessment: SafetyAssessment;
  confirmation_phrase: string | null;
}

export type DeviceSafetyDecision =
  | "allowed_for_planning"
  | "blocked"
  | "unsupported"
  | "inconclusive";

export interface DeviceSafetyAssessment {
  decision: DeviceSafetyDecision;
  method: string;
  scope: string;
  reason: string | null;
  warnings: string[];
  requires_elevation: boolean;
  target_is_mounted: boolean;
  system_volume_risk: boolean;
  device_identity_available: boolean;
  platform_supported: boolean;
  execution_disabled: boolean;
}

export interface DriveInspectionReport {
  profile: TargetProfile;
  assessment: DeviceSafetyAssessment;
  can_plan: boolean;
  execution_disabled: boolean;
}

export interface DrivePlan {
  plan_id: string;
  device_path: string;
  device_identity: string;
  method: string;
  scope: string;
  assessment: DeviceSafetyAssessment;
  expires_at: string;
  execution_disabled: boolean;
}

export interface HashFileOutput {
  algorithm: string;
  digest: string;
  size_bytes: number;
}

export type SanitizeOutcome =
  | "success"
  | "partial"
  | "failed"
  | "not_verified"
  | "unsupported";

export interface SanitizeFileResult {
  operation_id: string;
  outcome: SanitizeOutcome;
  bytes_overwritten: number;
  elapsed_secs: number;
  reason: string | null;
}

export interface FileFailure {
  path: string;
  reason: string;
}

export interface SanitizeFolderResult {
  operation_id: string;
  outcome: SanitizeOutcome;
  files_discovered: number;
  files_processed: number;
  files_removed: number;
  files_failed: number;
  total_original_bytes: number;
  total_bytes_written: number;
  elapsed_secs: number;
  reason: string | null;
  failures: FileFailure[];
}

export interface SanitizationOperation {
  id: string;
  case_id: string | null;
  target_path: string;
  target_kind: string;
  method: string;
  target_identity_verified: boolean;
  confirmation_kind: string;
  confirmation_validated: boolean;
  state: string;
  outcome: SanitizeOutcome | null;
  result_json: string | null;
  error_message: string | null;
  actor: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
}

export interface ArtifactSummary {
  id: string;
  detected_type: string;
  source_offset: number;
  source_length: number;
  validation_state: string;
  confidence: string;
  artifact_sha256: string;
  artifact_path: string | null;
}

export interface CarveResult {
  case_id: string;
  evidence_id: string;
  source_sha256: string;
  carving_job_id: string | null;
  recovery_job_id: string | null;
  bytes_scanned: number;
  scan_limit_reached: boolean;
  candidate_limit_reached: boolean;
  headers_dropped_by_limit: number;
  candidates_found: number;
  candidates_validated: number;
  candidates_rejected: number;
  artifacts: ArtifactSummary[];
}

export interface RecoveryResultDto {
  id: string;
  evidence_id: string;
  source_offset: number;
  source_length: number;
  detected_type: string;
  category: string;
  validation_state: string;
  confidence: string;
  artifact_sha256: string;
  artifact_path: string | null;
}

export interface GenerateReportResult {
  report_id: string;
  path: string;
  byte_size: number;
  sha256: string;
}

export const api = {
  verifyChain: (): Promise<ChainStatus> => invoke("verify_chain"),
  listDevices: (): Promise<TargetProfile[]> => invoke("list_devices"),
  inspectDriveTarget: (
    path: string,
    method: string,
    scope: string,
    caseId: string | null,
    actor: string | null,
  ): Promise<DriveInspectionReport> =>
    invoke("inspect_drive_target", {
      input: { path, method, scope, case_id: caseId, actor },
    }),
  planDriveSanitization: (
    path: string,
    method: string,
    scope: string,
    caseId: string | null,
    actor: string | null,
  ): Promise<DrivePlan> =>
    invoke("plan_drive_sanitization", {
      input: { path, method, scope, case_id: caseId, actor },
    }),
  inspectSanitizeTarget: (
    target: string,
    kind: "file" | "directory",
    case_id: string | null,
    actor: string | null,
  ): Promise<SanitizeTargetPreview> =>
    invoke("inspect_sanitize_target", { input: { target, kind, case_id, actor } }),
  cancelSanitizationPlan: (planId: string): Promise<boolean> =>
    invoke("cancel_sanitization_plan", { planId }),
  listSanitizationOperations: (): Promise<SanitizationOperation[]> =>
    invoke("list_sanitization_operations"),
  listCases: (): Promise<Case[]> => invoke("list_cases"),
  createCase: (title: string, examiner: string | null): Promise<{ case_id: string }> =>
    invoke("create_case", { input: { title, examiner } }),
  listEvidenceForCase: (case_id: string): Promise<Evidence[]> =>
    invoke("list_evidence_for_case", { caseId: case_id }),
  registerEvidence: (
    case_id: string,
    path: string,
    notes: string | null,
    actor: string | null,
  ): Promise<Evidence> =>
    invoke("register_evidence", {
      input: { case_id, path, notes, actor },
    }),
  verifyEvidence: (
    evidence_id: string,
    actor: string | null,
  ): Promise<EvidenceVerification> =>
    invoke("verify_evidence", { input: { evidence_id, actor } }),
  listJobs: (): Promise<Job[]> => invoke("list_jobs"),
  listAuditEvents: (): Promise<AuditEvent[]> => invoke("list_audit_events"),
  listReports: (): Promise<Report[]> => invoke("list_reports"),
  listRecoveryResults: (evidence_id: string): Promise<RecoveryResultDto[]> =>
    invoke("list_recovery_results", { evidenceId: evidence_id }),
  hashFile: (path: string): Promise<HashFileOutput> =>
    invoke("hash_file", { input: { path } }),
  sanitizeFile: (
    target: string,
    planId: string,
    confirm: boolean,
    typedConfirmation: string | null,
    caseId: string | null,
    actor: string | null,
  ): Promise<SanitizeFileResult> =>
    invoke("sanitize_file", {
      input: {
        target,
        plan_id: planId,
        confirm,
        typed_confirmation: typedConfirmation,
        case_id: caseId,
        actor,
      },
    }),
  sanitizeFolder: (
    target: string,
    planId: string,
    confirm: boolean,
    typedConfirmation: string | null,
    caseId: string | null,
    actor: string | null,
  ): Promise<SanitizeFolderResult> =>
    invoke("sanitize_folder", {
      input: {
        target,
        plan_id: planId,
        confirm,
        typed_confirmation: typedConfirmation,
        case_id: caseId,
        actor,
      },
    }),
  carveSource: (
    caseId: string,
    evidenceId: string,
    examiner: string | null,
  ): Promise<CarveResult> =>
    invoke("carve_source", {
      input: { case_id: caseId, evidence_id: evidenceId, examiner },
    }),
  generateReport: (
    caseId: string,
    outPath: string,
  ): Promise<GenerateReportResult> =>
    invoke("generate_report", { input: { case_id: caseId, out_path: outPath } }),
};