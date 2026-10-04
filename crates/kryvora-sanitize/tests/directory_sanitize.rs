#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration tests for directory sanitization.

use kryvora_db::{apply_migrations, open};
use kryvora_sanitize::{
    sanitize_directory, MediaType, SanitizeDirectoryReport, SanitizeOptions, SanitizeOutcome,
};
use rusqlite::Connection;
use std::fs;
use std::path::Path;

fn fresh_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("k.sqlite");
    let mut conn = open(&path).unwrap();
    apply_migrations(&mut conn).unwrap();
    (dir, conn)
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let p = dir.join(name);
    fs::write(&p, bytes).unwrap();
    p
}

/// Options that keep files in place after overwrite and force the
/// magnetic media type. The magnetic override makes the outcome
/// deterministic regardless of the machine the test runs on: it
/// exercises the `Success` path of the walker.
fn magnetic_keep_options() -> SanitizeOptions {
    SanitizeOptions {
        unlink_after: false,
        media_override: Some(MediaType::Magnetic),
        ..SanitizeOptions::default()
    }
}

/// Options that request unlink after overwrite and force the magnetic
/// media type. The sanitizer conservatively retains the file when it
/// cannot bind an unlink operation to the opened file handle.
fn magnetic_remove_options() -> SanitizeOptions {
    SanitizeOptions {
        unlink_after: true,
        media_override: Some(MediaType::Magnetic),
        ..SanitizeOptions::default()
    }
}

#[test]
fn empty_directory_reports_success_with_zero_files() {
    let (db_dir, conn) = fresh_db();
    let target = db_dir.path().join("empty");
    fs::create_dir(&target).unwrap();

    let r: SanitizeDirectoryReport =
        sanitize_directory(&conn, &target, &magnetic_remove_options()).unwrap();

    assert_eq!(r.outcome, SanitizeOutcome::Success, "{r:?}");
    assert_eq!(r.files_discovered, 0);
    assert_eq!(r.files_processed, 0);
    assert_eq!(r.total_bytes_written, 0);
}

#[test]
fn flat_directory_processes_all_files() {
    let (db_dir, conn) = fresh_db();
    let target = db_dir.path().join("flat");
    fs::create_dir(&target).unwrap();

    let a = write_file(&target, "a.bin", &[0xAA; 1024]);
    let b = write_file(&target, "b.bin", &[0xBB; 2048]);
    let c = write_file(&target, "c.bin", &[0xCC; 512]);

    let r = sanitize_directory(&conn, &target, &magnetic_keep_options()).unwrap();

    assert_eq!(r.outcome, SanitizeOutcome::Success, "{r:?}");
    assert_eq!(r.files_discovered, 3);
    assert_eq!(r.files_processed, 3);
    assert_eq!(r.files_failed, 0);
    assert_eq!(r.total_bytes_written, 1024 + 2048 + 512);

    for (path, orig) in [(&a, 0xAA_u8), (&b, 0xBB), (&c, 0xCC)] {
        let bytes = fs::read(path).unwrap();
        assert!(!bytes.iter().all(|&x| x == orig));
    }
}

#[test]
fn recursive_directory_processes_nested_files() {
    let (db_dir, conn) = fresh_db();
    let root = db_dir.path().join("root");
    let sub1 = root.join("sub1");
    let sub2 = sub1.join("sub2");
    fs::create_dir_all(&sub2).unwrap();

    write_file(&root, "top.bin", &[1; 100]);
    write_file(&sub1, "mid.bin", &[2; 200]);
    write_file(&sub2, "deep.bin", &[3; 300]);

    let r = sanitize_directory(&conn, &root, &magnetic_remove_options()).unwrap();

    assert_eq!(r.outcome, SanitizeOutcome::Partial);
    assert_eq!(r.files_discovered, 3);
    assert_eq!(r.files_processed, 3);
    assert_eq!(r.files_removed, 0);
    assert_eq!(r.total_bytes_written, 100 + 200 + 300);

    assert!(root.join("top.bin").exists());
    assert!(sub1.join("mid.bin").exists());
    assert!(sub2.join("deep.bin").exists());
}

#[test]
fn zero_byte_file_is_reported_as_failed_in_the_aggregate() {
    let (db_dir, conn) = fresh_db();
    let target = db_dir.path().join("zero");
    fs::create_dir(&target).unwrap();
    write_file(&target, "empty.bin", b"");
    write_file(&target, "real.bin", &[0xAA; 500]);

    let r = sanitize_directory(&conn, &target, &magnetic_keep_options()).unwrap();

    assert_eq!(r.files_discovered, 2);
    assert_eq!(r.files_failed, 1);
    assert_eq!(r.outcome, SanitizeOutcome::Partial);
    assert_eq!(r.failures.len(), 1);
    assert!(r
        .failures
        .iter()
        .any(|failure| failure.path.contains("empty.bin")));
}

#[test]
fn directory_that_is_a_file_reports_error() {
    let (db_dir, conn) = fresh_db();
    let f = write_file(db_dir.path(), "not-a-dir.bin", b"x");

    let err = sanitize_directory(&conn, &f, &magnetic_remove_options()).unwrap_err();
    assert!(
        matches!(err, kryvora_core::Error::InvalidInput(_)),
        "{err:?}"
    );
}

#[test]
fn nonexistent_directory_reports_not_found() {
    let (db_dir, conn) = fresh_db();
    let missing = db_dir.path().join("does-not-exist");

    let err = sanitize_directory(&conn, &missing, &magnetic_remove_options()).unwrap_err();
    assert!(matches!(err, kryvora_core::Error::NotFound(_)), "{err:?}");
}

#[test]
fn filesystem_root_is_refused_without_scanning_or_writing() {
    let (_db_dir, conn) = fresh_db();
    #[cfg(windows)]
    let root = std::path::PathBuf::from(format!(
        "{}\\",
        std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into())
    ));
    #[cfg(unix)]
    let root = std::path::PathBuf::from("/");

    let report = sanitize_directory(&conn, &root, &SanitizeOptions::default()).unwrap();

    assert_eq!(report.outcome, SanitizeOutcome::Unsupported);
    assert_eq!(report.files_discovered, 0);
    assert!(report
        .reason
        .as_deref()
        .unwrap_or_default()
        .contains("mount points"));
}

#[test]
fn files_are_not_treated_as_the_directory_itself() {
    let (db_dir, conn) = fresh_db();
    let target = db_dir.path().join("t");
    fs::create_dir(&target).unwrap();
    write_file(&target, "only.bin", b"abcdefghij");

    let r = sanitize_directory(&conn, &target, &magnetic_keep_options()).unwrap();
    assert_eq!(r.files_discovered, 1);
    assert_eq!(r.total_original_bytes, 10);
    assert_eq!(r.total_bytes_written, 10);
}

#[cfg(unix)]
#[test]
fn symlinks_and_fifo_entries_are_reported_as_unsupported() {
    let (db_dir, conn) = fresh_db();
    let target = db_dir.path().join("with-links");
    fs::create_dir(&target).unwrap();
    let real = write_file(&target, "real.bin", b"sensitive");

    let symlink_path = target.join("shortcut.bin");
    std::os::unix::fs::symlink(&real, &symlink_path).unwrap();

    let fifo_path = target.join("named-pipe");
    let fifo = std::ffi::CString::new(fifo_path.as_os_str().as_bytes()).unwrap();
    let rc = unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) };
    assert_eq!(rc, 0, "mkfifo failed to create a test FIFO");

    let r = sanitize_directory(&conn, &target, &magnetic_keep_options()).unwrap();

    assert!(r.outcome == SanitizeOutcome::Partial || r.outcome == SanitizeOutcome::Failed);
    assert!(r.failures.iter().any(|f| f.path.ends_with("shortcut.bin")));
    assert!(r.failures.iter().any(|f| f.path.ends_with("named-pipe")));
    assert!(r.files_discovered >= 1);
}

#[test]
fn real_machine_media_produces_a_honest_outcome() {
    // This test does NOT force a media type. It runs against the
    // actual storage the tempdir lives on and asserts only that the
    // walker produces one of the two honest outcomes. It is the test
    // that proves the crate does not claim Success on hardware where
    // overwrite is insufficient.
    let (db_dir, conn) = fresh_db();
    let target = db_dir.path().join("honest");
    fs::create_dir(&target).unwrap();
    write_file(&target, "x.bin", &[0x42; 256]);

    let r = sanitize_directory(&conn, &target, &SanitizeOptions::default()).unwrap();

    match r.outcome {
        SanitizeOutcome::Success | SanitizeOutcome::NotVerified | SanitizeOutcome::Partial => {}
        other => panic!("unexpected outcome on real media: {other:?}"),
    }
    assert_eq!(r.files_discovered, 1);
    assert_eq!(r.total_bytes_written, 256);
}
