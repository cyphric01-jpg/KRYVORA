// file: crates/kryvora-storage/src/target.rs
//! The [`TargetProfile`] value type and its supporting enums.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// What kind of thing a target is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    /// A regular file on a filesystem.
    File,
    /// A directory on a filesystem.
    Directory,
    /// A filesystem root or a mount point.
    Filesystem,
    /// A block device (physical disk, partition, loop device).
    BlockDevice,
    /// The target could not be classified on this platform. Explicit.
    Unknown,
}

impl TargetKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::Filesystem => "filesystem",
            Self::BlockDevice => "block_device",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for TargetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Filesystem information, when it can be determined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemInfo {
    /// Filesystem type, e.g. `"NTFS"`, `"ext4"`, `"APFS"`. Best-effort.
    pub fs_type: String,
    /// Mount point or drive root, e.g. `"C:\\"` or `"/mnt/data"`.
    pub mount_point: String,
    /// Whether the mount is writable. `None` means unknown.
    pub writable: Option<bool>,
}

/// A factual description of a target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetProfile {
    pub kind: TargetKind,
    pub path: String,
    pub display: String,
    pub size_bytes: Option<u64>,
    pub read_only: bool,
    pub removable: Option<bool>,
    pub device_identity: Option<String>,
    pub filesystem: Option<FilesystemInfo>,
    pub warnings: Vec<TargetWarning>,
}

/// Non-fatal observations from inspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetWarning {
    /// The target is on removable media.
    RemovableMedia,
    /// The target is a system or boot volume.
    SystemVolume,
    /// The target cannot be written to.
    ReadOnly,
    /// The target is not a regular file or directory.
    SpecialFile,
    /// The target's kind could not be classified.
    UnknownKind { reason: String },
}

/// Errors from inspection above and beyond [`kryvora_core::Error`].
#[derive(Debug, Error)]
pub enum InspectionError {
    #[error("target does not exist: {0}")]
    NotFound(String),

    #[error("permission denied inspecting {0}")]
    PermissionDenied(String),

    #[error("unsupported on this platform: {0}")]
    Unsupported(String),

    #[error("io error while inspecting {0}: {1}")]
    Io(String, String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_kind_strings() {
        assert_eq!(TargetKind::File.as_str(), "file");
        assert_eq!(TargetKind::BlockDevice.as_str(), "block_device");
        assert_eq!(TargetKind::Unknown.to_string(), "unknown");
    }

    #[test]
    fn target_kind_serde_is_snake_case() {
        assert_eq!(
            serde_json::to_string(&TargetKind::BlockDevice).unwrap(),
            "\"block_device\""
        );
    }

    #[test]
    fn warning_serde_is_tagged() {
        let w = TargetWarning::UnknownKind {
            reason: "test".into(),
        };
        let s = serde_json::to_string(&w).unwrap();
        assert!(s.contains("\"kind\":\"unknown_kind\""), "{s}");
    }

    #[test]
    fn profile_roundtrips_through_json() {
        let p = TargetProfile {
            kind: TargetKind::File,
            path: "/tmp/x".into(),
            display: "x".into(),
            size_bytes: Some(42),
            read_only: false,
            removable: Some(false),
            device_identity: None,
            filesystem: None,
            warnings: vec![TargetWarning::SpecialFile],
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: TargetProfile = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}
