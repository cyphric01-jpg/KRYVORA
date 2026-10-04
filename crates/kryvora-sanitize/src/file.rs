// file: crates/kryvora-sanitize/src/file.rs
//! Single-file overwrite and verification.

use crate::media::{detect_media_type, MediaType};
use crate::result::{SanitizationResult, SanitizeError};
use crate::safety::is_system_path;
use kryvora_audit::{append, EventDraft, EventType};
use kryvora_core::Error;
use kryvora_integrity::calculate_hash;
use kryvora_storage::{TargetIdentity, TargetKind};
use rusqlite::Connection;
use serde_json::json;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Instant;

/// Options for a single-file sanitization.
#[derive(Debug, Clone)]
pub struct SanitizeOptions {
    /// Size of the random-data buffer used per write. Defaults to 1 MiB.
    pub buffer_size: usize,
    /// Whether to unlink the file after a verified overwrite.
    pub unlink_after: bool,
    /// Actor string for audit events.
    pub actor: Option<String>,
    /// Persisted application operation that owns this destructive attempt.
    pub operation_id: Option<String>,
    /// Associated case when the caller has a selected case context.
    pub case_id: Option<String>,
    /// True only when the caller captured and revalidated a target identity.
    pub target_identity_verified: bool,
    /// Test-only override for the detected media type. In production,
    /// leave this as `None`. When `Some`, `sanitize_file` uses the
    /// supplied media type instead of calling `detect_media_type`.
    ///
    /// Exposed so that tests can deterministically exercise the
    /// magnetic-media `Success` path and the solid-state
    /// `NotVerified` path without depending on the machine the tests
    /// run on.
    pub media_override: Option<MediaType>,
}

impl Default for SanitizeOptions {
    fn default() -> Self {
        Self {
            buffer_size: 1024 * 1024,
            unlink_after: true,
            actor: None,
            operation_id: None,
            case_id: None,
            target_identity_verified: false,
            media_override: None,
        }
    }
}

/// Sanitize a single regular file.
///
/// See the module documentation for the honesty contract.
///
/// # Errors
///
/// * [`Error::PermissionDenied`] — path is a protected system path or
///   is read-only.
/// * [`Error::InvalidInput`] — path is not a regular file.
/// * [`Error::NotFound`] — path does not exist.
/// * [`Error::Io`] — any I/O failure that aborts the operation.
pub fn sanitize_file(
    conn: &Connection,
    path: impl AsRef<Path>,
    options: &SanitizeOptions,
) -> kryvora_core::Result<SanitizationResult> {
    sanitize_file_bound(conn, path, options, None)
}

/// Sanitize a file only if the opened object still matches the identity
/// captured during the user's target inspection.
pub fn sanitize_file_bound(
    conn: &Connection,
    path: impl AsRef<Path>,
    options: &SanitizeOptions,
    expected_identity: Option<&TargetIdentity>,
) -> kryvora_core::Result<SanitizationResult> {
    let path = path.as_ref();
    let start = Instant::now();

    // 1. System-path protection.
    if let Some(reason) = is_system_path(path) {
        let result = SanitizationResult::unsupported(reason.message());
        append_outcome(conn, path, &result, options)?;
        return Ok(result);
    }

    if let Some(identity) = expected_identity {
        if identity.kind() != TargetKind::File || !identity.matches_path(path)? {
            return Err(Error::UnsafeTarget(
                "sanitization target changed after inspection; inspect it again".into(),
            ));
        }
    }

    // 2. Preconditions.
    let meta = std::fs::symlink_metadata(path).map_err(map_io)?;
    if !meta.file_type().is_file() {
        return Err(Error::InvalidInput(
            SanitizeError::NotAFile(path.display().to_string()).to_string(),
        ));
    }
    if meta.permissions().readonly() {
        return Err(Error::PermissionDenied(
            SanitizeError::ReadOnly(path.display().to_string()).to_string(),
        ));
    }

    let mut file = open_target(path).map_err(map_io)?;
    let opened_meta = file.metadata().map_err(map_io)?;
    let path_handle = same_file::Handle::from_path(path).map_err(map_io)?;
    let opened_handle =
        same_file::Handle::from_file(file.try_clone().map_err(map_io)?).map_err(map_io)?;
    if !opened_meta.is_file() || path_handle != opened_handle {
        return Err(Error::InvalidInput(
            "target changed while opening; refusing to sanitize".into(),
        ));
    }
    if let Some(identity) = expected_identity {
        if !identity.matches_file(&file)? {
            return Err(Error::UnsafeTarget(
                "opened file does not match the inspected target identity".into(),
            ));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if opened_meta.file_attributes() & 0x400 != 0 {
            return Err(Error::UnsafeTarget(
                "reparse-point targets cannot be sanitized safely".into(),
            ));
        }
    }

    // Media detection. Test override takes precedence.
    let media = options
        .media_override
        .unwrap_or_else(|| detect_media_type(path));

    // 3. Empty file.
    if meta.len() == 0 {
        let result = SanitizationResult::failed(
            start.elapsed(),
            "target file is empty; no data to overwrite".into(),
        );
        append_outcome(conn, path, &result, options)?;
        return Ok(result);
    }

    // 4. Original digest.
    let original_hash = match hash_open_file(&mut file) {
        Ok(h) => h,
        Err(e) => {
            let result = SanitizationResult::failed(
                start.elapsed(),
                format!("could not hash original: {e}"),
            );
            append_outcome(conn, path, &result, options)?;
            return Ok(result);
        }
    };

    // 5. Overwrite pass.
    let buffer_size = if options.buffer_size == 0 {
        1024 * 1024
    } else {
        options.buffer_size
    };
    let total = meta.len();
    let mut bytes_written: u64 = 0;

    file.seek(SeekFrom::Start(0)).map_err(map_io)?;
    let mut buffer = vec![0u8; buffer_size];

    while bytes_written < total {
        let remaining = total - bytes_written;
        let to_write = remaining.min(buffer_size as u64) as usize;
        getrandom::getrandom(&mut buffer[..to_write]).map_err(|e| {
            Error::Internal(SanitizeError::RandomUnavailable(e.to_string()).to_string())
        })?;

        match file.write_all(&buffer[..to_write]) {
            Ok(()) => {
                bytes_written += to_write as u64;
            }
            Err(e) => {
                let result = SanitizationResult::partial(
                    bytes_written,
                    start.elapsed(),
                    format!("write failed after {bytes_written} bytes: {e}"),
                );
                append_outcome(conn, path, &result, options)?;
                return Ok(result);
            }
        }
    }

    if let Err(e) = file.sync_all() {
        let result = SanitizationResult::partial(
            bytes_written,
            start.elapsed(),
            format!("sync failed: {e}"),
        );
        append_outcome(conn, path, &result, options)?;
        return Ok(result);
    }

    // 6. Verification.
    let new_hash = match hash_open_file(&mut file) {
        Ok(h) => h,
        Err(e) => {
            let result = SanitizationResult::partial(
                bytes_written,
                start.elapsed(),
                format!("could not re-hash after overwrite: {e}"),
            );
            append_outcome(conn, path, &result, options)?;
            return Ok(result);
        }
    };

    if new_hash.digest_matches(&original_hash) {
        let result = SanitizationResult::failed(
            start.elapsed(),
            "post-overwrite digest matches original; overwrite was ineffective".into(),
        );
        append_outcome(conn, path, &result, options)?;
        return Ok(result);
    }

    // 7. Unlink.
    if options.unlink_after {
        let result = SanitizationResult::partial(
            bytes_written,
            start.elapsed(),
            "overwrite verified; path-based unlink is disabled because it cannot be \
             safely bound to the opened file on this platform"
                .into(),
        );
        append_outcome(conn, path, &result, options)?;
        return Ok(result);
    }

    // 8. Final outcome depends on media.
    let result = if media.overwrite_is_sufficient() {
        SanitizationResult::success(bytes_written, start.elapsed())
    } else {
        SanitizationResult::not_verified(
            bytes_written,
            start.elapsed(),
            format!(
                "file overwrite on {} media does not guarantee the original \
                 storage blocks are unrecoverable",
                media.as_str()
            ),
        )
    };

    append_outcome(conn, path, &result, options)?;
    Ok(result)
}

fn open_target(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }

    options.open(path)
}

fn hash_open_file(file: &mut File) -> kryvora_core::Result<kryvora_integrity::Hash> {
    file.seek(SeekFrom::Start(0)).map_err(map_io)?;
    let hash = calculate_hash(&mut *file)?;
    file.seek(SeekFrom::Start(0)).map_err(map_io)?;
    Ok(hash)
}

fn append_outcome(
    conn: &Connection,
    path: &Path,
    result: &SanitizationResult,
    options: &SanitizeOptions,
) -> kryvora_core::Result<()> {
    append(
        conn,
        &EventDraft {
            event_type: EventType::SanitizationOutcomeRecorded,
            actor: options.actor.clone(),
            object_id: options
                .operation_id
                .clone()
                .or_else(|| Some(path.display().to_string())),
            job_id: None,
            details: json!({
                "operation_id": options.operation_id,
                "case_id": options.case_id,
                "target_path": path.display().to_string(),
                "target_identity_verified": options.target_identity_verified,
                "outcome": result.outcome.as_str(),
                "bytes_overwritten": result.bytes_overwritten,
                "elapsed_secs": result.elapsed.as_secs_f64(),
                "reason": result.reason,
            }),
        },
    )?;
    Ok(())
}

fn map_io(e: std::io::Error) -> Error {
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
    fn file_handles_distinguish_different_targets() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        std::fs::write(&first, b"one").unwrap();
        std::fs::write(&second, b"a different file").unwrap();

        let first_path_handle = same_file::Handle::from_path(&first).unwrap();
        let same_file_handle = same_file::Handle::from_file(File::open(&first).unwrap()).unwrap();
        let second_file_handle =
            same_file::Handle::from_file(File::open(&second).unwrap()).unwrap();

        assert_eq!(first_path_handle, same_file_handle);
        assert_ne!(first_path_handle, second_file_handle);
    }

    #[test]
    fn options_default_is_sane() {
        let o = SanitizeOptions::default();
        assert_eq!(o.buffer_size, 1024 * 1024);
        assert!(o.unlink_after);
        assert!(o.actor.is_none());
        assert!(o.media_override.is_none());
    }
}
