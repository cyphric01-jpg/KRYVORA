#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration tests for file sanitization.

use kryvora_audit::{count, verify_chain, ChainStatus};
use kryvora_core::Error;
use kryvora_db::{apply_migrations, open};
use kryvora_sanitize::{
    sanitize_file, sanitize_file_bound, SanitizationResult, SanitizeOptions, SanitizeOutcome,
};
use kryvora_storage::{TargetIdentity, TargetKind};
use rusqlite::Connection;
use std::fs;

fn fresh_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("k.sqlite");
    let mut conn = open(&path).unwrap();
    apply_migrations(&mut conn).unwrap();
    (dir, conn)
}

fn write_file(dir: &std::path::Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let p = dir.join(name);
    fs::write(&p, bytes).unwrap();
    p
}

#[test]
fn sanitize_reports_a_real_outcome_and_counts_real_bytes() {
    let (db_dir, conn) = fresh_db();
    let payload = b"secret payload that must be erased";
    let target = write_file(db_dir.path(), "target.bin", payload);

    let result: SanitizationResult =
        sanitize_file(&conn, &target, &SanitizeOptions::default()).unwrap();

    assert_eq!(result.bytes_overwritten, payload.len() as u64);

    assert_eq!(result.outcome, SanitizeOutcome::Partial);
    assert!(target.exists());
    assert_ne!(fs::read(&target).unwrap(), payload);
    assert!(count(&conn).unwrap() >= 1);
    assert!(matches!(
        verify_chain(&conn).unwrap(),
        ChainStatus::Intact { .. }
    ));
}

#[test]
fn sanitize_without_unlink_leaves_overwritten_content() {
    let (db_dir, conn) = fresh_db();
    let original = b"original content";
    let target = write_file(db_dir.path(), "target.bin", original);

    let options = SanitizeOptions {
        unlink_after: false,
        ..SanitizeOptions::default()
    };
    let result = sanitize_file(&conn, &target, &options).unwrap();

    assert!(target.exists());
    let new_content = fs::read(&target).unwrap();
    assert_eq!(new_content.len(), original.len());
    assert_ne!(new_content, original);
    assert_eq!(result.bytes_overwritten, original.len() as u64);
}

#[test]
fn sanitize_rejects_missing_file() {
    let (db_dir, conn) = fresh_db();
    let missing = db_dir.path().join("nope.bin");

    let err = sanitize_file(&conn, &missing, &SanitizeOptions::default()).unwrap_err();
    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

#[test]
fn sanitize_rejects_directory() {
    let (db_dir, conn) = fresh_db();

    let err = sanitize_file(&conn, db_dir.path(), &SanitizeOptions::default()).unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[test]
fn sanitize_empty_file_reports_failed() {
    let (db_dir, conn) = fresh_db();
    let target = write_file(db_dir.path(), "empty.bin", b"");

    let result = sanitize_file(&conn, &target, &SanitizeOptions::default()).unwrap();
    assert_eq!(result.outcome, SanitizeOutcome::Failed);
    assert_eq!(result.bytes_overwritten, 0);
    assert!(target.exists());
}

#[test]
fn sanitize_large_file_streams_in_chunks() {
    let (db_dir, conn) = fresh_db();
    let data = vec![0xAB_u8; 5 * 1024 * 1024];
    let target = write_file(db_dir.path(), "big.bin", &data);

    let options = SanitizeOptions {
        buffer_size: 64 * 1024,
        ..SanitizeOptions::default()
    };
    let result = sanitize_file(&conn, &target, &options).unwrap();

    assert_eq!(result.bytes_overwritten, data.len() as u64);
    assert!(target.exists());
    assert_eq!(result.outcome, SanitizeOutcome::Partial);
}

#[test]
fn sanitize_digest_changes_after_overwrite() {
    let (db_dir, conn) = fresh_db();
    let original = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let target = write_file(db_dir.path(), "t.bin", original);

    let options = SanitizeOptions {
        unlink_after: false,
        ..SanitizeOptions::default()
    };
    let _ = sanitize_file(&conn, &target, &options).unwrap();

    let new = fs::read(&target).unwrap();
    assert_eq!(new.len(), original.len());
    assert_ne!(new.as_slice(), original.as_slice());
}

#[test]
fn sanitize_refuses_a_target_replaced_after_inspection() {
    let (db_dir, conn) = fresh_db();
    let target = write_file(db_dir.path(), "target.bin", b"original target");
    let replacement = write_file(db_dir.path(), "replacement.bin", b"new target");
    let identity = TargetIdentity::capture(&target, TargetKind::File).unwrap();

    fs::remove_file(&target).unwrap();
    fs::rename(&replacement, &target).unwrap();
    let before = fs::read(&target).unwrap();
    let error = sanitize_file_bound(
        &conn,
        &target,
        &SanitizeOptions::default(),
        Some(&identity),
    )
    .unwrap_err();

    assert!(matches!(error, Error::UnsafeTarget(_)), "{error:?}");
    assert_eq!(fs::read(&target).unwrap(), before);
    assert_eq!(count(&conn).unwrap(), 0);
}

#[test]
fn sanitize_rejects_read_only_target_without_writing() {
    let (db_dir, conn) = fresh_db();
    let original = b"preserve this read-only file";
    let target = write_file(db_dir.path(), "readonly.bin", original);
    let mut permissions = fs::metadata(&target).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&target, permissions).unwrap();

    let error = sanitize_file(&conn, &target, &SanitizeOptions::default()).unwrap_err();

    assert!(matches!(error, Error::PermissionDenied(_)), "{error:?}");
    assert_eq!(fs::read(&target).unwrap(), original);
    assert_eq!(count(&conn).unwrap(), 0);
}
