// file: crates/kryvora-storage/src/device.rs
//! Inspection of block devices and filesystems.

use crate::target::{FilesystemInfo, TargetKind, TargetProfile, TargetWarning};
use kryvora_core::{Error, Result};
use serde::{Deserialize, Serialize};
use sysinfo::Disks;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors specific to device inspection.
#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("device not found: {0}")]
    NotFound(String),
    #[error("device inspection not supported on this platform: {0}")]
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceSafetyDecision {
    AllowedForPlanning,
    Blocked,
    Unsupported,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSafetyAssessment {
    pub decision: DeviceSafetyDecision,
    pub method: String,
    pub scope: String,
    pub reason: Option<String>,
    pub warnings: Vec<String>,
    pub requires_elevation: bool,
    pub target_is_mounted: bool,
    pub system_volume_risk: bool,
    pub device_identity_available: bool,
    pub platform_supported: bool,
    pub execution_disabled: bool,
}

/// Assessment used by the drive-eraser foundation before any destructive
/// execution is ever considered. It is intentionally conservative. An
/// inconclusive or blocked assessment is never treated as approval.
pub fn assess_device_target(
    profile: &TargetProfile,
    method: &str,
    scope: &str,
) -> DeviceSafetyAssessment {
    let mut warnings = Vec::new();
    let system_volume_risk = profile
        .warnings
        .iter()
        .any(|warning| matches!(warning, TargetWarning::SystemVolume));
    if system_volume_risk {
        warnings.push("device is a system or boot volume".into());
    }
    if profile.read_only {
        warnings.push("device is read-only or not writable".into());
    }
    if profile
        .filesystem
        .as_ref()
        .is_some_and(|fs| !fs.mount_point.trim().is_empty())
    {
        warnings.push("device is mounted and may be in use".into());
    }
    if profile.removable == Some(true) {
        warnings.push("device is removable media".into());
    }

    let target_is_mounted = profile.filesystem.as_ref().is_some();
    let device_identity_available = profile.device_identity.as_deref().is_some_and(|v| !v.trim().is_empty());
    let supported_method = matches!(method, "overwrite" | "random_overwrite" | "sanitize_overwrite");
    let platform_supported = cfg!(windows) || cfg!(unix);

    if profile.kind != TargetKind::BlockDevice && profile.kind != TargetKind::Filesystem {
        return DeviceSafetyAssessment {
            decision: DeviceSafetyDecision::Blocked,
            method: method.to_string(),
            scope: scope.to_string(),
            reason: Some(format!("target kind {} is not a block device or mounted filesystem", profile.kind.as_str())),
            warnings,
            requires_elevation: true,
            target_is_mounted,
            system_volume_risk,
            device_identity_available,
            platform_supported,
            execution_disabled: true,
        };
    }

    if system_volume_risk {
        return DeviceSafetyAssessment {
            decision: DeviceSafetyDecision::Blocked,
            method: method.to_string(),
            scope: scope.to_string(),
            reason: Some("device is a system or boot volume; drive erasure is blocked".into()),
            warnings,
            requires_elevation: true,
            target_is_mounted,
            system_volume_risk,
            device_identity_available,
            platform_supported,
            execution_disabled: true,
        };
    }

    if target_is_mounted {
        return DeviceSafetyAssessment {
            decision: DeviceSafetyDecision::Blocked,
            method: method.to_string(),
            scope: scope.to_string(),
            reason: Some("device is mounted and therefore treated as in use; drive erasure remains disabled".into()),
            warnings,
            requires_elevation: true,
            target_is_mounted,
            system_volume_risk,
            device_identity_available,
            platform_supported,
            execution_disabled: true,
        };
    }

    if !platform_supported {
        return DeviceSafetyAssessment {
            decision: DeviceSafetyDecision::Unsupported,
            method: method.to_string(),
            scope: scope.to_string(),
            reason: Some("device sanitization is unsupported on this platform".into()),
            warnings,
            requires_elevation: false,
            target_is_mounted,
            system_volume_risk,
            device_identity_available,
            platform_supported,
            execution_disabled: true,
        };
    }

    if !supported_method {
        return DeviceSafetyAssessment {
            decision: DeviceSafetyDecision::Unsupported,
            method: method.to_string(),
            scope: scope.to_string(),
            reason: Some(format!("requested method {} is not supported for physical-device sanitization", method)),
            warnings,
            requires_elevation: true,
            target_is_mounted,
            system_volume_risk,
            device_identity_available,
            platform_supported,
            execution_disabled: true,
        };
    }

    if !device_identity_available {
        return DeviceSafetyAssessment {
            decision: DeviceSafetyDecision::Inconclusive,
            method: method.to_string(),
            scope: scope.to_string(),
            reason: Some("device identity is unavailable; a stable identity is required before any planning".into()),
            warnings,
            requires_elevation: true,
            target_is_mounted,
            system_volume_risk,
            device_identity_available,
            platform_supported,
            execution_disabled: true,
        };
    }

    DeviceSafetyAssessment {
        decision: DeviceSafetyDecision::AllowedForPlanning,
        method: method.to_string(),
        scope: scope.to_string(),
        reason: Some("inspection-only planning is currently enabled; physical drive erasure is intentionally disabled until platform validation is complete".into()),
        warnings,
        requires_elevation: true,
        target_is_mounted,
        system_volume_risk,
        device_identity_available,
        platform_supported,
        execution_disabled: true,
    }
}

/// List every block device / filesystem the platform reports.
#[must_use]
pub fn list_devices() -> Vec<TargetProfile> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .map(|d| {
            let mount = d.mount_point().display().to_string();
            let display = if mount.is_empty() {
                d.name().to_string_lossy().into_owned()
            } else {
                mount.clone()
            };

            let fs_type = d.file_system().to_string_lossy().into_owned();
            let total = d.total_space();

            let mut warnings = Vec::new();
            if d.is_removable() {
                warnings.push(TargetWarning::RemovableMedia);
            }
            if is_system_volume_mount(&mount) {
                warnings.push(TargetWarning::SystemVolume);
            }

            TargetProfile {
                kind: TargetKind::BlockDevice,
                path: d.name().to_string_lossy().into_owned(),
                display,
                size_bytes: Some(total),
                read_only: d.is_read_only(),
                removable: Some(d.is_removable()),
                device_identity: None,
                filesystem: Some(FilesystemInfo {
                    fs_type,
                    mount_point: mount,
                    writable: Some(!d.is_read_only()),
                }),
                warnings,
            }
        })
        .collect()
}

/// Return whether `path` is a filesystem root/mount point reported by the
/// operating system. This is a conservative block for directory erasure,
/// not a drive-erasure implementation.
#[must_use]
pub fn is_volume_root(path: impl AsRef<Path>) -> bool {
    let candidate = normalized_path(path.as_ref());
    Path::new(&candidate).parent().is_none()
        || list_devices().iter().any(|device| {
            device
                .filesystem
                .as_ref()
                .is_some_and(|filesystem| normalized_path(Path::new(&filesystem.mount_point)) == candidate)
        })
}

fn is_system_volume_mount(mount: &str) -> bool {
    let mount_path = normalized_path(Path::new(mount));
    system_roots().iter().any(|root| {
        let system_path = normalized_path(root);
        if system_path == mount_path {
            return true;
        }
        let separator = if cfg!(windows) { '\\' } else { '/' };
        let prefix = if mount_path.ends_with(separator) {
            mount_path.clone()
        } else {
            format!("{mount_path}{separator}")
        };
        system_path.starts_with(&prefix)
    })
}

fn system_roots() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut roots = Vec::new();
        if let Ok(system_root) = std::env::var("SystemRoot") {
            roots.push(PathBuf::from(system_root));
        }
        if let Ok(system_drive) = std::env::var("SystemDrive") {
            roots.push(PathBuf::from(format!("{system_drive}\\")));
        }
        roots
    }
    #[cfg(unix)]
    {
        vec![PathBuf::from("/")]
    }
    #[cfg(not(any(windows, unix)))]
    {
        Vec::new()
    }
}

fn normalized_path(path: &Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let value = canonical.to_string_lossy().replace('/', "\\");
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value.replace('\\', "/")
    }
}

/// Inspect a single device by its mount point or device path.
///
/// # Errors
///
/// * [`Error::NotFound`] — no enumerated device matches `id`.
pub fn inspect_device_by_id(id: &str) -> Result<TargetProfile> {
    let devices = list_devices();
    devices
        .into_iter()
        .find(|d| d.path == id || d.display == id)
        .ok_or_else(|| Error::NotFound(DeviceError::NotFound(id.to_string()).to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_devices_does_not_panic() {
        let _ = list_devices();
    }

    #[test]
    fn inspect_unknown_device_returns_not_found() {
        let err = inspect_device_by_id("no-such-device-xyz").unwrap_err();
        assert!(matches!(err, Error::NotFound(_)), "{err:?}");
    }

    #[test]
    fn filesystem_root_is_protected() {
        #[cfg(unix)]
        assert!(is_volume_root("/"));

        #[cfg(windows)]
        if let Ok(system_drive) = std::env::var("SystemDrive") {
            assert!(is_volume_root(format!("{system_drive}\\")));
        }
    }

    #[test]
    fn empty_device_list_is_valid_and_not_error() {
        let devices = Vec::<TargetProfile>::new();
        assert!(devices.is_empty());
        let assessment = assess_device_target(
            &TargetProfile {
                kind: TargetKind::BlockDevice,
                path: "\\\\.\\PhysicalDrive0".into(),
                display: "PhysicalDrive0".into(),
                size_bytes: Some(1 << 30),
                read_only: false,
                removable: Some(false),
                device_identity: Some("drive-0".into()),
                filesystem: None,
                warnings: vec![],
            },
            "random_overwrite",
            "full",
        );
        assert_eq!(assessment.decision, DeviceSafetyDecision::AllowedForPlanning);
    }

    #[test]
    fn system_volume_and_mounted_devices_are_blocked() {
        let mounted = TargetProfile {
            kind: TargetKind::BlockDevice,
            path: "/dev/sda".into(),
            display: "sda".into(),
            size_bytes: Some(1024),
            read_only: false,
            removable: Some(false),
            device_identity: Some("sda".into()),
            filesystem: Some(FilesystemInfo {
                fs_type: "ext4".into(),
                mount_point: "/".into(),
                writable: Some(true),
            }),
            warnings: vec![TargetWarning::SystemVolume],
        };
        let blocked = assess_device_target(&mounted, "random_overwrite", "full");
        assert_eq!(blocked.decision, DeviceSafetyDecision::Blocked);
        assert!(blocked.reason.unwrap().contains("system or boot volume"));
    }

    #[test]
    fn unsupported_methods_and_missing_identity_are_conservative() {
        let profile = TargetProfile {
            kind: TargetKind::BlockDevice,
            path: "/dev/sdb".into(),
            display: "sdb".into(),
            size_bytes: Some(2048),
            read_only: false,
            removable: Some(false),
            device_identity: None,
            filesystem: None,
            warnings: vec![],
        };

        let unsupported = assess_device_target(&profile, "unsupported_method", "full");
        assert_eq!(unsupported.decision, DeviceSafetyDecision::Unsupported);

        let inconclusive = assess_device_target(
            &TargetProfile {
                kind: TargetKind::BlockDevice,
                path: "/dev/sdc".into(),
                display: "sdc".into(),
                size_bytes: Some(4096),
                read_only: false,
                removable: Some(false),
                device_identity: None,
                filesystem: None,
                warnings: vec![],
            },
            "random_overwrite",
            "full",
        );
        assert_eq!(inconclusive.decision, DeviceSafetyDecision::Inconclusive);
    }
}
