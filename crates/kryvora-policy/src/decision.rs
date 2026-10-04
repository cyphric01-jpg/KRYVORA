//! Policy decisions, safety assessments, and warnings.

use crate::operation::{OperationKind, OperationRequest};
use kryvora_storage::{TargetKind, TargetProfile, TargetWarning};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from policy evaluation above and beyond
/// [`kryvora_core::Error`].
#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("policy evaluation is inconsistent: {0}")]
    Inconsistent(String),
}

/// A warning that must be shown to the user before execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Warning {
    RemovableMedia { display: String },
    SystemVolume { display: String },
    ReadOnlyTarget { display: String },
    UnknownTargetKind { reason: String },
    DeviceSanitizationIsIrreversible { display: String },
    DirectorySanitizationIsRecursive { display: String },
}

/// A confirmation the user must give before execution proceeds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Confirmation {
    /// A simple yes/no prompt: "Sanitize this file?"
    Basic { prompt: String },
    /// A destructive-action prompt requiring a typed phrase.
    TypedPhrase { prompt: String, phrase: String },
    /// A prompt requiring the user to confirm a specific device identity.
    DeviceIdentity {
        prompt: String,
        expected_identity: String,
    },
}

/// The assessment of a target's safety for a given operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyAssessment {
    pub allowed: bool,
    pub destructive: bool,
    pub requires_elevation: bool,
    pub warnings: Vec<Warning>,
    pub confirmations: Vec<Confirmation>,
}

/// The final policy decision for an operation on a target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PolicyDecision {
    /// The operation is permitted. If `assessment.confirmations` is
    /// non-empty, the caller must obtain each one and re-evaluate with
    /// `user_confirmed = true`.
    Permitted { assessment: SafetyAssessment },
    /// The operation is refused. `reason` is user-facing.
    Refused {
        reason: String,
        assessment: SafetyAssessment,
    },
}

impl PolicyDecision {
    #[must_use]
    pub fn is_permitted(&self) -> bool {
        matches!(self, Self::Permitted { .. })
    }

    #[must_use]
    pub fn assessment(&self) -> &SafetyAssessment {
        match self {
            Self::Permitted { assessment } => assessment,
            Self::Refused { assessment, .. } => assessment,
        }
    }
}

/// Evaluate a request against a target profile.
///
/// Pure. Deterministic. Never returns an error for a well-formed
/// request; refusals are `PolicyDecision::Refused`, not `Err`.
///
/// # Errors
///
/// Returns [`kryvora_core::Error::Internal`] only in the case of an
/// internal inconsistency that should be impossible given the current
/// operation set. A future operation kind that requires policy logic
/// not yet implemented returns `PolicyError::Inconsistent`.
pub fn evaluate(
    profile: &TargetProfile,
    request: &OperationRequest,
) -> kryvora_core::Result<PolicyDecision> {
    let destructive = request.kind.is_destructive();
    let requires_elevation = request.kind.typically_requires_elevation();

    let warnings = collect_warnings(profile, request.kind);
    let confirmations = collect_confirmations(profile, request.kind)?;

    // Capture the emptiness of the confirmation list *before* moving it
    // into the assessment. Whether confirmations are required is a
    // property of the operation, not of the vector's storage location.
    let has_confirmations = !confirmations.is_empty();

    let assessment = SafetyAssessment {
        allowed: true,
        destructive,
        requires_elevation,
        warnings,
        confirmations,
    };

    // Non-destructive operations are always permitted.
    if !destructive {
        return Ok(PolicyDecision::Permitted { assessment });
    }

    // Destructive operations: check the target supports them.
    if let Some(refusal_reason) = refuse_reason(profile, request.kind) {
        return Ok(PolicyDecision::Refused {
            reason: refusal_reason,
            assessment,
        });
    }

    // Destructive operation with confirmations: permitted only if all
    // confirmations have been acknowledged. The decision is still
    // Permitted; the caller must obtain the confirmations and
    // re-evaluate. Returning Refused here would conflate "you must
    // confirm" with "you may not".
    if has_confirmations && !request.user_confirmed {
        return Ok(PolicyDecision::Permitted { assessment });
    }

    Ok(PolicyDecision::Permitted { assessment })
}

fn refuse_reason(profile: &TargetProfile, kind: OperationKind) -> Option<String> {
    if kind.is_destructive() && profile.read_only {
        return Some(format!(
            "operation {} refuses a read-only target: {}",
            kind.as_str(),
            profile.display
        ));
    }

    match (kind, profile.kind) {
        (OperationKind::SanitizeFile, TargetKind::File) => None,
        (OperationKind::SanitizeFile, other) => Some(format!(
            "operation sanitize_file requires a file target, got {}",
            other.as_str()
        )),
        (OperationKind::SanitizeDirectory, TargetKind::Directory) => None,
        (OperationKind::SanitizeDirectory, other) => Some(format!(
            "operation sanitize_directory requires a directory target, got {}",
            other.as_str()
        )),
        (OperationKind::SanitizeDevice, TargetKind::BlockDevice) => None,
        (OperationKind::SanitizeDevice, TargetKind::Filesystem) => None,
        (OperationKind::SanitizeDevice, other) => Some(format!(
            "operation sanitize_device requires a block device or filesystem, got {}",
            other.as_str()
        )),
        _ => None,
    }
}

fn collect_warnings(profile: &TargetProfile, kind: OperationKind) -> Vec<Warning> {
    let mut out = Vec::new();

    for w in &profile.warnings {
        match w {
            TargetWarning::RemovableMedia => out.push(Warning::RemovableMedia {
                display: profile.display.clone(),
            }),
            TargetWarning::SystemVolume => out.push(Warning::SystemVolume {
                display: profile.display.clone(),
            }),
            TargetWarning::ReadOnly => out.push(Warning::ReadOnlyTarget {
                display: profile.display.clone(),
            }),
            TargetWarning::SpecialFile => {}
            TargetWarning::UnknownKind { reason } => out.push(Warning::UnknownTargetKind {
                reason: reason.clone(),
            }),
        }
    }

    if matches!(kind, OperationKind::SanitizeDevice) {
        out.push(Warning::DeviceSanitizationIsIrreversible {
            display: profile.display.clone(),
        });
    }
    if matches!(kind, OperationKind::SanitizeDirectory) {
        out.push(Warning::DirectorySanitizationIsRecursive {
            display: profile.display.clone(),
        });
    }

    out
}

fn collect_confirmations(
    profile: &TargetProfile,
    kind: OperationKind,
) -> kryvora_core::Result<Vec<Confirmation>> {
    let mut out = Vec::new();

    match kind {
        OperationKind::SanitizeFile => out.push(Confirmation::Basic {
            prompt: format!("Sanitize file {}?", profile.display),
        }),
        OperationKind::SanitizeDirectory => out.push(Confirmation::TypedPhrase {
            prompt: format!(
                "Type ERASE to confirm recursive sanitization of {}",
                profile.display
            ),
            phrase: "ERASE".to_string(),
        }),
        OperationKind::SanitizeDevice => {
            let identity = profile
                .device_identity
                .clone()
                .unwrap_or_else(|| profile.display.clone());
            out.push(Confirmation::DeviceIdentity {
                prompt: format!(
                    "Confirm sanitization of device {} by typing its identity",
                    profile.display
                ),
                expected_identity: identity,
            });
        }
        _ => {}
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::OperationKind;

    fn file_profile() -> TargetProfile {
        TargetProfile {
            kind: TargetKind::File,
            path: "/tmp/x".into(),
            display: "x".into(),
            size_bytes: Some(10),
            read_only: false,
            removable: None,
            device_identity: None,
            filesystem: None,
            warnings: vec![],
        }
    }

    fn dir_profile() -> TargetProfile {
        TargetProfile {
            kind: TargetKind::Directory,
            path: "/tmp/d".into(),
            display: "d".into(),
            size_bytes: None,
            read_only: false,
            removable: None,
            device_identity: None,
            filesystem: None,
            warnings: vec![],
        }
    }

    fn device_profile() -> TargetProfile {
        TargetProfile {
            kind: TargetKind::BlockDevice,
            path: "/dev/sdz".into(),
            display: "sdz".into(),
            size_bytes: Some(1_000_000_000),
            read_only: false,
            removable: Some(false),
            device_identity: Some("DISK-SERIAL-ABC".into()),
            filesystem: None,
            warnings: vec![],
        }
    }

    fn req(kind: OperationKind) -> OperationRequest {
        OperationRequest::new(kind, "test").unwrap()
    }

    #[test]
    fn inspect_is_permitted_without_confirmation() {
        let d = evaluate(&file_profile(), &req(OperationKind::Inspect)).unwrap();
        assert!(d.is_permitted());
        assert!(d.assessment().confirmations.is_empty());
        assert!(!d.assessment().destructive);
    }

    #[test]
    fn hash_is_permitted_without_confirmation() {
        let d = evaluate(&file_profile(), &req(OperationKind::Hash)).unwrap();
        assert!(d.is_permitted());
        assert!(d.assessment().confirmations.is_empty());
    }

    #[test]
    fn sanitize_file_on_file_requires_basic_confirmation() {
        let d = evaluate(&file_profile(), &req(OperationKind::SanitizeFile)).unwrap();
        assert!(d.is_permitted());
        let a = d.assessment();
        assert!(a.destructive);
        assert_eq!(a.confirmations.len(), 1);
        assert!(matches!(a.confirmations[0], Confirmation::Basic { .. }));
    }

    #[test]
    fn sanitize_file_on_directory_is_refused() {
        let d = evaluate(&dir_profile(), &req(OperationKind::SanitizeFile)).unwrap();
        match d {
            PolicyDecision::Refused { reason, .. } => {
                assert!(reason.contains("requires a file target"), "{reason}");
            }
            other => panic!("expected Refused, got {other:?}"),
        }
    }

    #[test]
    fn destructive_file_operation_refuses_read_only_target() {
        let mut profile = file_profile();
        profile.read_only = true;
        profile.warnings.push(TargetWarning::ReadOnly);

        let decision = evaluate(&profile, &req(OperationKind::SanitizeFile)).unwrap();
        assert!(!decision.is_permitted());
    }

    #[test]
    fn destructive_directory_operation_refuses_read_only_target() {
        let mut profile = dir_profile();
        profile.read_only = true;
        profile.warnings.push(TargetWarning::ReadOnly);

        let decision = evaluate(&profile, &req(OperationKind::SanitizeDirectory)).unwrap();
        assert!(!decision.is_permitted());
    }

    #[test]
    fn sanitize_directory_requires_typed_phrase() {
        let d = evaluate(&dir_profile(), &req(OperationKind::SanitizeDirectory)).unwrap();
        assert!(d.is_permitted());
        let a = d.assessment();
        assert_eq!(a.confirmations.len(), 1);
        match &a.confirmations[0] {
            Confirmation::TypedPhrase { phrase, .. } => assert_eq!(phrase, "ERASE"),
            other => panic!("expected TypedPhrase, got {other:?}"),
        }
    }

    #[test]
    fn sanitize_device_requires_identity_confirmation() {
        let d = evaluate(&device_profile(), &req(OperationKind::SanitizeDevice)).unwrap();
        assert!(d.is_permitted());
        let a = d.assessment();
        match &a.confirmations[0] {
            Confirmation::DeviceIdentity {
                expected_identity, ..
            } => assert_eq!(expected_identity, "DISK-SERIAL-ABC"),
            other => panic!("expected DeviceIdentity, got {other:?}"),
        }
    }

    #[test]
    fn sanitize_device_on_file_is_refused() {
        let d = evaluate(&file_profile(), &req(OperationKind::SanitizeDevice)).unwrap();
        assert!(!d.is_permitted());
    }

    #[test]
    fn readonly_target_emits_warning() {
        let mut p = file_profile();
        p.read_only = true;
        p.warnings = vec![TargetWarning::ReadOnly];

        let d = evaluate(&p, &req(OperationKind::SanitizeFile)).unwrap();
        let warnings = &d.assessment().warnings;
        assert!(warnings
            .iter()
            .any(|w| matches!(w, Warning::ReadOnlyTarget { .. })));
    }

    #[test]
    fn removable_media_emits_warning() {
        let mut p = file_profile();
        p.removable = Some(true);
        p.warnings = vec![TargetWarning::RemovableMedia];

        let d = evaluate(&p, &req(OperationKind::SanitizeFile)).unwrap();
        let warnings = &d.assessment().warnings;
        assert!(warnings
            .iter()
            .any(|w| matches!(w, Warning::RemovableMedia { .. })));
    }

    #[test]
    fn device_sanitization_always_emits_irreversibility_warning() {
        let d = evaluate(&device_profile(), &req(OperationKind::SanitizeDevice)).unwrap();
        let warnings = &d.assessment().warnings;
        assert!(warnings
            .iter()
            .any(|w| matches!(w, Warning::DeviceSanitizationIsIrreversible { .. })));
    }
}
