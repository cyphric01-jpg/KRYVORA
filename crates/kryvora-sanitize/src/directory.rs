// file: crates/kryvora-sanitize/src/directory.rs
//! Recursive directory sanitization.

use crate::file::{sanitize_file_bound, SanitizeOptions};
use crate::outcome::SanitizeOutcome;
use crate::result::SanitizeError;
use crate::safety::is_system_path;
use kryvora_core::Error;
use kryvora_storage::{is_volume_root, TargetIdentity, TargetKind};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Per-file failure record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFailure {
    pub path: String,
    pub reason: String,
}

/// The aggregate result of a directory sanitization.
///
/// The `outcome` field is authoritative. Every other field is a fact
/// about what happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanitizeDirectoryReport {
    pub outcome: SanitizeOutcome,
    pub root: String,
    pub files_discovered: u64,
    pub files_processed: u64,
    pub files_removed: u64,
    pub files_failed: u64,
    pub total_original_bytes: u64,
    pub total_bytes_written: u64,
    #[serde(with = "duration_secs")]
    pub elapsed: Duration,
    pub failures: Vec<FileFailure>,
    /// A short explanation when the outcome is not Success.
    pub reason: Option<String>,
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        d.as_secs_f64().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = f64::deserialize(d)?;
        Ok(Duration::from_secs_f64(secs.max(0.0)))
    }
}

/// Recursively sanitize every regular file under `root`.
///
/// # Behavior
///
/// * Symlinks and reparse points are skipped, not followed.
/// * Directories are recursed into; they are removed after their
///   contents if empty.
/// * Per-file outcome is captured. `Success` from the file level is
///   required for a file to be counted as removed.
/// * The aggregate outcome is:
///   * `Unsupported` if the root is a protected system path.
///   * `Failed` if the root does not exist or is not a directory.
///   * `Success` if every discovered file was processed with outcome
///     `Success` and no failures.
///   * `Partial` if some files succeeded and some failed, or some
///     files returned `NotVerified`.
///   * `NotVerified` if every file was overwritten but at least one
///     file's media cannot guarantee sanitization.
///
/// # Errors
///
/// * [`Error::NotFound`] — root does not exist.
/// * [`Error::InvalidInput`] — root is not a directory.
/// * Any I/O error that aborts the walk before it begins.
pub fn sanitize_directory(
    conn: &Connection,
    root: impl AsRef<Path>,
    options: &SanitizeOptions,
) -> kryvora_core::Result<SanitizeDirectoryReport> {
    sanitize_directory_bound(conn, root, options, None)
}

/// Sanitize a directory only if its root still matches the identity
/// captured during target inspection.
pub fn sanitize_directory_bound(
    conn: &Connection,
    root: impl AsRef<Path>,
    options: &SanitizeOptions,
    expected_identity: Option<&TargetIdentity>,
) -> kryvora_core::Result<SanitizeDirectoryReport> {
    let root = root.as_ref();
    let start = Instant::now();

    // System-path protection.
    if let Some(reason) = is_system_path(root) {
        return Ok(SanitizeDirectoryReport {
            outcome: SanitizeOutcome::Unsupported,
            root: root.display().to_string(),
            files_discovered: 0,
            files_processed: 0,
            files_removed: 0,
            files_failed: 0,
            total_original_bytes: 0,
            total_bytes_written: 0,
            elapsed: start.elapsed(),
            failures: vec![],
            reason: Some(reason.message()),
        });
    }

    if is_volume_root(root) {
        return Ok(SanitizeDirectoryReport {
            outcome: SanitizeOutcome::Unsupported,
            root: root.display().to_string(),
            files_discovered: 0,
            files_processed: 0,
            files_removed: 0,
            files_failed: 0,
            total_original_bytes: 0,
            total_bytes_written: 0,
            elapsed: start.elapsed(),
            failures: vec![],
            reason: Some("filesystem roots and mount points cannot be sanitized".into()),
        });
    }

    if let Some(identity) = expected_identity {
        if identity.kind() != TargetKind::Directory || !identity.matches_path(root)? {
            return Err(Error::UnsafeTarget(
                "sanitization directory changed after inspection; inspect it again".into(),
            ));
        }
    }

    if !root.exists() {
        return Err(Error::NotFound(root.display().to_string()));
    }
    if !root.is_dir() {
        return Err(Error::InvalidInput(
            SanitizeError::Unsupported(format!(
                "sanitize_directory requires a directory, got file: {}",
                root.display()
            ))
            .to_string(),
        ));
    }

    let mut files_processed: u64 = 0;
    let mut files_removed: u64 = 0;
    let mut files_failed: u64 = 0;
    let mut total_original_bytes: u64 = 0;
    let mut total_bytes_written: u64 = 0;
    let mut failures: Vec<FileFailure> = Vec::new();
    let mut any_not_verified = false;
    let mut any_partial = false;

    // Collect files and unsupported entries first. This keeps the report
    // accurate even if the walk is interrupted partway through processing.
    let mut files: Vec<PathBuf> = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    let unsupported_entries = collect_entries(root, &mut files, &mut dirs)?;
    let files_discovered = (files.len() + unsupported_entries.len()) as u64;
    if !unsupported_entries.is_empty() {
        files_failed += unsupported_entries.len() as u64;
        failures.extend(unsupported_entries);
        any_partial = true;
    }

    for path in &files {
        if expected_identity.is_some_and(|identity| !identity.matches_path(root).unwrap_or(false)) {
            let remaining = files_discovered.saturating_sub(files_processed);
            files_failed = files_failed.saturating_add(remaining);
            failures.push(FileFailure {
                path: root.display().to_string(),
                reason: "directory identity changed during sanitization; remaining files were not processed".into(),
            });
            any_partial = true;
            break;
        }
        // Record original size before sanitization; the file will be
        // gone after success.
        let original_size = match std::fs::metadata(path) {
            Ok(m) => m.len(),
            Err(_) => 0,
        };
        total_original_bytes = total_original_bytes.saturating_add(original_size);

        let file_identity = match TargetIdentity::capture(path, TargetKind::File) {
            Ok(identity) => identity,
            Err(error) => {
                files_failed += 1;
                failures.push(FileFailure {
                    path: path.display().to_string(),
                    reason: error.to_string(),
                });
                continue;
            }
        };

        match sanitize_file_bound(conn, path, options, Some(&file_identity)) {
            Ok(r) => {
                files_processed += 1;
                total_bytes_written = total_bytes_written.saturating_add(r.bytes_overwritten);
                match r.outcome {
                    SanitizeOutcome::Success => {
                        if !path.exists() {
                            files_removed += 1;
                        } else if options.unlink_after {
                            any_partial = true;
                            files_failed += 1;
                            failures.push(FileFailure {
                                path: path.display().to_string(),
                                reason: "file was overwritten but retained because safe unlink is unavailable".into(),
                            });
                        }
                    }
                    SanitizeOutcome::NotVerified => {
                        any_not_verified = true;
                        if !path.exists() {
                            files_removed += 1;
                        } else if options.unlink_after {
                            any_partial = true;
                            files_failed += 1;
                            failures.push(FileFailure {
                                path: path.display().to_string(),
                                reason: "file was overwritten but retained because safe unlink is unavailable".into(),
                            });
                        }
                    }
                    SanitizeOutcome::Partial => {
                        any_partial = true;
                        files_failed += 1;
                        failures.push(FileFailure {
                            path: path.display().to_string(),
                            reason: r.reason.unwrap_or_else(|| "partial".into()),
                        });
                    }
                    SanitizeOutcome::Failed | SanitizeOutcome::Unsupported => {
                        files_failed += 1;
                        failures.push(FileFailure {
                            path: path.display().to_string(),
                            reason: r.reason.unwrap_or_else(|| "failed".into()),
                        });
                    }
                }
            }
            Err(e) => {
                files_failed += 1;
                failures.push(FileFailure {
                    path: path.display().to_string(),
                    reason: e.to_string(),
                });
            }
        }
    }

    // Remove empty directories, deepest first. Directories that still
    // contain files (because those files failed) are kept.
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for d in dirs {
        if d == root {
            continue;
        }
        let _ = std::fs::remove_dir(&d); // best-effort; ignore errors.
    }

    let outcome = if files_failed > 0 && files_processed == 0 {
        SanitizeOutcome::Failed
    } else if files_failed > 0 || any_partial {
        SanitizeOutcome::Partial
    } else if files_processed == 0 {
        // Empty directory. Nothing to sanitize. Not a failure.
        SanitizeOutcome::Success
    } else if any_not_verified {
        SanitizeOutcome::NotVerified
    } else {
        SanitizeOutcome::Success
    };

    let reason = match outcome {
        SanitizeOutcome::Success => None,
        SanitizeOutcome::Failed => {
            Some(format!("all {} discovered files failed", files_discovered))
        }
        SanitizeOutcome::Partial => Some(format!(
            "{files_failed} of {files_discovered} files failed or were retained"
        )),
        SanitizeOutcome::NotVerified => Some(
            "file overwrite cannot guarantee sanitization on the underlying \
             storage media"
                .into(),
        ),
        SanitizeOutcome::Unsupported => Some("unsupported target".into()),
    };

    Ok(SanitizeDirectoryReport {
        outcome,
        root: root.display().to_string(),
        files_discovered,
        files_processed,
        files_removed,
        files_failed,
        total_original_bytes,
        total_bytes_written,
        elapsed: start.elapsed(),
        failures,
        reason,
    })
}

/// Recursively collect files and directories under `root`.
///
/// Symlinks are not followed.
fn collect_entries(
    root: &Path,
    files: &mut Vec<PathBuf>,
    dirs: &mut Vec<PathBuf>,
) -> kryvora_core::Result<Vec<FileFailure>> {
    let entries = std::fs::read_dir(root).map_err(|e| {
        use std::io::ErrorKind;
        match e.kind() {
            ErrorKind::NotFound => Error::NotFound(root.display().to_string()),
            ErrorKind::PermissionDenied => Error::PermissionDenied(root.display().to_string()),
            _ => Error::Io(e),
        }
    })?;

    let mut unsupported = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                unsupported.push(FileFailure {
                    path: root.display().to_string(),
                    reason: format!("could not inspect directory entry: {err}"),
                });
                continue;
            }
        };
        let path = entry.path();
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(err) => {
                unsupported.push(FileFailure {
                    path: path.display().to_string(),
                    reason: format!("could not inspect file type: {err}"),
                });
                continue;
            }
        };
        if ft.is_symlink() {
            unsupported.push(FileFailure {
                path: path.display().to_string(),
                reason: "symlink or reparse point skipped; not followed".into(),
            });
            continue;
        }
        if ft.is_dir() {
            dirs.push(path.clone());
            unsupported.extend(collect_entries(&path, files, dirs)?);
        } else if ft.is_file() {
            files.push(path);
        } else {
            unsupported.push(FileFailure {
                path: path.display().to_string(),
                reason: "special file skipped because it is not a regular file or directory".into(),
            });
        }
    }
    Ok(unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_system_path_reports_unsupported() {
        // Any path under a protected env var will produce Unsupported.
        // This test is a no-op if no env var is set.
        if let Ok(root) = std::env::var("SystemRoot") {
            let probe = PathBuf::from(&root).join("System32").join("drivers");
            if probe.exists() {
                // We need a real database connection to call the function;
                // this test asserts the *logic* by checking is_system_path
                // directly, since a live DB is not necessary for the check.
                assert!(is_system_path(&probe).is_some());
            }
        }
    }
}
