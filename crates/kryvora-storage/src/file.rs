// file: crates/kryvora-storage/src/file.rs
//! Inspection of regular files and directories.

use crate::target::{TargetKind, TargetProfile, TargetWarning};
use kryvora_core::{Error, Result};
use same_file::Handle;
use std::fs::{File, Metadata};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors specific to file and directory inspection.
#[derive(Debug, Error)]
pub enum FileInspectionError {
    #[error("not a regular file: {0}")]
    NotRegularFile(String),
    #[error("not a directory: {0}")]
    NotDirectory(String),
}

/// Inspect a path that is expected to be a regular file.
///
/// # Errors
///
/// * [`Error::NotFound`] — path does not exist.
/// * [`Error::InvalidInput`] — path exists but is not a regular file.
/// * [`Error::PermissionDenied`] — metadata cannot be read.
/// * [`Error::Io`] — any other I/O failure.
pub fn inspect_file(path: impl AsRef<Path>) -> Result<TargetProfile> {
    let path = path.as_ref();
    let meta = std::fs::symlink_metadata(path).map_err(map_meta_err)?;

    if !meta.file_type().is_file() {
        return Err(Error::InvalidInput(
            FileInspectionError::NotRegularFile(path.display().to_string()).to_string(),
        ));
    }

    let mut warnings = Vec::new();
    if meta.permissions().readonly() {
        warnings.push(TargetWarning::ReadOnly);
    }

    Ok(TargetProfile {
        kind: TargetKind::File,
        path: path.display().to_string(),
        display: file_name_display(path),
        size_bytes: Some(meta.len()),
        read_only: meta.permissions().readonly(),
        removable: None,
        device_identity: None,
        filesystem: None,
        warnings,
    })
}

/// Inspect a path that is expected to be a directory.
///
/// # Errors
///
/// Same as [`inspect_file`], with `NotDirectory` in place of
/// `NotRegularFile`.
pub fn inspect_directory(path: impl AsRef<Path>) -> Result<TargetProfile> {
    let path = path.as_ref();
    let meta = std::fs::symlink_metadata(path).map_err(map_meta_err)?;

    if !meta.file_type().is_dir() {
        return Err(Error::InvalidInput(
            FileInspectionError::NotDirectory(path.display().to_string()).to_string(),
        ));
    }

    let mut warnings = Vec::new();
    if meta.permissions().readonly() {
        warnings.push(TargetWarning::ReadOnly);
    }

    Ok(TargetProfile {
        kind: TargetKind::Directory,
        path: path.display().to_string(),
        display: file_name_display(path),
        size_bytes: None,
        read_only: meta.permissions().readonly(),
        removable: None,
        device_identity: None,
        filesystem: None,
        warnings,
    })
}

/// An opaque identity captured during target inspection.
///
/// The open handle pins the inspected object while the user confirms. It is
/// intentionally not serializable; callers keep it in backend state and
/// return a separate one-use plan ID to the UI.
#[derive(Debug)]
pub struct TargetIdentity {
    kind: TargetKind,
    canonical_path: PathBuf,
    handle: Handle,
}

impl TargetIdentity {
    /// Capture a regular file or directory identity and its canonical path.
    pub fn capture(path: impl AsRef<Path>, kind: TargetKind) -> Result<Self> {
        let path = path.as_ref();
        let metadata = std::fs::symlink_metadata(path).map_err(map_meta_err)?;
        validate_kind(&metadata, kind, path)?;
        reject_reparse_point(&metadata, path)?;
        let canonical_path = path.canonicalize().map_err(map_meta_err)?;
        let handle = Handle::from_path(&canonical_path).map_err(map_meta_err)?;
        Ok(Self {
            kind,
            canonical_path,
            handle,
        })
    }

    /// Check that a path still resolves to the confirmed object and location.
    pub fn matches_path(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = path.as_ref();
        let metadata = std::fs::symlink_metadata(path).map_err(map_meta_err)?;
        validate_kind(&metadata, self.kind, path)?;
        reject_reparse_point(&metadata, path)?;
        let canonical_path = path.canonicalize().map_err(map_meta_err)?;
        if canonical_path != self.canonical_path {
            return Ok(false);
        }
        let current = Handle::from_path(path).map_err(map_meta_err)?;
        Ok(current == self.handle)
    }

    /// Check the object opened for execution against the inspected handle.
    pub fn matches_file(&self, file: &File) -> Result<bool> {
        let metadata = file.metadata().map_err(map_meta_err)?;
        validate_metadata_kind(&metadata, self.kind)?;
        let current = Handle::from_file(file.try_clone().map_err(map_meta_err)?)
            .map_err(map_meta_err)?;
        Ok(current == self.handle)
    }

    #[must_use]
    pub const fn kind(&self) -> TargetKind {
        self.kind
    }

    #[must_use]
    pub fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }
}

fn validate_kind(metadata: &Metadata, expected: TargetKind, path: &Path) -> Result<()> {
    validate_metadata_kind(metadata, expected).map_err(|_| {
        Error::InvalidInput(format!(
            "target kind changed or is unsupported for {}: {}",
            path.display(),
            expected.as_str()
        ))
    })
}

fn validate_metadata_kind(metadata: &Metadata, expected: TargetKind) -> Result<()> {
    let matches = match expected {
        TargetKind::File => metadata.file_type().is_file(),
        TargetKind::Directory => metadata.file_type().is_dir(),
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(Error::InvalidInput("target kind does not match".into()))
    }
}

fn reject_reparse_point(metadata: &Metadata, path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(Error::UnsafeTarget(format!(
                "reparse-point targets cannot be bound safely: {}",
                path.display()
            )));
        }
    }
    let _ = (metadata, path);
    Ok(())
}

fn file_name_display(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn map_meta_err(e: std::io::Error) -> Error {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::NotFound => Error::NotFound(e.to_string()),
        ErrorKind::PermissionDenied => Error::PermissionDenied(e.to_string()),
        _ => Error::Io(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_file_returns_profile() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.bin");
        std::fs::write(&p, b"hello").unwrap();

        let profile = inspect_file(&p).unwrap();
        assert_eq!(profile.kind, TargetKind::File);
        assert_eq!(profile.size_bytes, Some(5));
        assert_eq!(profile.display, "x.bin");
    }

    #[test]
    fn inspect_file_rejects_directory() {
        let dir = tempfile::tempdir().unwrap();
        let err = inspect_file(dir.path()).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    }

    #[test]
    fn inspect_file_rejects_missing() {
        let err = inspect_file("C:/definitely/not/here/at/all").unwrap_err();
        assert!(matches!(err, Error::NotFound(_)), "{err:?}");
    }

    #[test]
    fn inspect_directory_returns_profile() {
        let dir = tempfile::tempdir().unwrap();
        let profile = inspect_directory(dir.path()).unwrap();
        assert_eq!(profile.kind, TargetKind::Directory);
        assert_eq!(profile.size_bytes, None);
    }

    #[test]
    fn inspect_directory_rejects_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.bin");
        std::fs::write(&p, b"hello").unwrap();

        let err = inspect_directory(&p).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    }

    #[test]
    fn captured_identity_matches_the_same_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.bin");
        std::fs::write(&path, b"payload").unwrap();

        let identity = TargetIdentity::capture(&path, TargetKind::File).unwrap();
        assert!(identity.matches_path(&path).unwrap());
        let file = File::open(&path).unwrap();
        assert!(identity.matches_file(&file).unwrap());
    }

    #[test]
    fn captured_identity_rejects_a_replaced_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.bin");
        let replacement = dir.path().join("replacement.bin");
        std::fs::write(&path, b"same-size").unwrap();
        std::fs::write(&replacement, b"same-size").unwrap();
        let identity = TargetIdentity::capture(&path, TargetKind::File).unwrap();

        std::fs::remove_file(&path).unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        assert!(!identity.matches_path(&path).unwrap());
    }

    #[test]
    fn stable_identity_rejects_symbolic_links() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.bin");
        let link = dir.path().join("link.bin");
        std::fs::write(&target, b"payload").unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(windows)]
        if std::os::windows::fs::symlink_file(&target, &link).is_err() {
            return;
        }

        assert!(matches!(
            TargetIdentity::capture(&link, TargetKind::File),
            Err(Error::InvalidInput(_))
        ));
    }
}
