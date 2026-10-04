// file: crates/kryvora-sanitize/src/media.rs
//! Detect the storage media type of a path.
//!
//! Used to decide whether file overwrite can be reported as a
//! verified sanitization. On rotational media (magnetic HDDs), a
//! single cryptographic-random overwrite is widely accepted to defeat
//! recovery. On non-rotational media (SSDs, NVMe, some USB sticks), it
//! is not, because the firmware may redirect the writes to fresh
//! blocks and leave the original data in place.

use std::path::Path;
use sysinfo::{DiskKind, Disks};

/// The storage media type, as far as it can be determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaType {
    /// Rotational magnetic storage.
    Magnetic,
    /// Solid-state storage (SSD, NVMe, most modern USB sticks).
    SolidState,
    /// Removable media whose rotational status is unknown.
    RemovableUnknown,
    /// The media type could not be determined.
    Unknown,
}

impl MediaType {
    /// Whether file overwrite can be reported as a verified
    /// sanitization on this media.
    #[must_use]
    pub const fn overwrite_is_sufficient(self) -> bool {
        matches!(self, Self::Magnetic)
    }

    /// A short human-readable label for audit and reporting.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Magnetic => "magnetic",
            Self::SolidState => "solid_state",
            Self::RemovableUnknown => "removable_unknown",
            Self::Unknown => "unknown",
        }
    }
}

/// Detect the media type for the filesystem that contains `path`.
///
/// Matches `path` against the mount points reported by `sysinfo`. If
/// no mount point matches, or if the platform does not report
/// rotational status, [`MediaType::Unknown`] is returned.
///
/// The caller must treat `Unknown` conservatively: it does not confirm
/// that overwrite is sufficient.
#[must_use]
pub fn detect_media_type(path: &Path) -> MediaType {
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    let disks = Disks::new_with_refreshed_list();

    let mut best: Option<(&sysinfo::Disk, usize)> = None;
    for d in disks.iter() {
        let mount = d.mount_point();
        if canon.starts_with(mount) {
            let len = mount.as_os_str().len();
            if best.map(|(_, l)| len > l).unwrap_or(true) {
                best = Some((d, len));
            }
        }
    }

    match best {
        None => MediaType::Unknown,
        Some((d, _)) => {
            if d.is_removable() && matches!(d.kind(), DiskKind::Unknown(_)) {
                MediaType::RemovableUnknown
            } else {
                match d.kind() {
                    DiskKind::HDD => MediaType::Magnetic,
                    DiskKind::SSD => MediaType::SolidState,
                    DiskKind::Unknown(_) => MediaType::Unknown,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnetic_permits_overwrite_success() {
        assert!(MediaType::Magnetic.overwrite_is_sufficient());
    }

    #[test]
    fn solid_state_does_not_permit_overwrite_success() {
        assert!(!MediaType::SolidState.overwrite_is_sufficient());
    }

    #[test]
    fn unknown_does_not_permit_overwrite_success() {
        assert!(!MediaType::Unknown.overwrite_is_sufficient());
    }

    #[test]
    fn detect_on_nonexistent_path_returns_a_variant() {
        let t = detect_media_type(Path::new("C:/does/not/exist/anywhere"));
        let _ = t;
    }

    #[test]
    fn as_str_is_stable() {
        assert_eq!(MediaType::Magnetic.as_str(), "magnetic");
        assert_eq!(MediaType::SolidState.as_str(), "solid_state");
    }
}
