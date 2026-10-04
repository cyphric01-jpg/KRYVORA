#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration tests for the `kryvora` binary.
//!
//! These tests invoke the compiled binary as a subprocess. They rely on
//! `env!("CARGO_BIN_EXE_kryvora")`, which Cargo sets to the path of the
//! binary under test.

use std::path::Path;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_kryvora")
}

fn write_source(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn run_creates_database_and_reports_intact_chain() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("kryvora.db");
    let source = write_source(dir.path(), "sample.bin", b"forensic payload");

    let out = Command::new(bin())
        .arg("run")
        .arg("--db")
        .arg(&db)
        .arg("--case-title")
        .arg("Case Alpha")
        .arg("--examiner")
        .arg("tester")
        .arg("--source")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "run failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("=== KRYVORA v0.1 ==="));
    assert!(stdout.contains("case:         CASE-"));
    assert!(stdout.contains("evidence:     EVID-"));
    assert!(stdout.contains("integrity:    Verified"));
    assert!(stdout.contains("chain:        Intact (length 4)"));

    // The database file must exist.
    assert!(db.is_file());
}

#[test]
fn verify_reports_intact_after_run() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("kryvora.db");
    let source = write_source(dir.path(), "sample.bin", b"payload");

    // First run the slice.
    let run_out = Command::new(bin())
        .arg("run")
        .arg("--db")
        .arg(&db)
        .arg("--case-title")
        .arg("t")
        .arg("--source")
        .arg(&source)
        .output()
        .unwrap();
    assert!(run_out.status.success());

    // Then verify the chain.
    let out = Command::new(bin())
        .arg("verify")
        .arg("--db")
        .arg(&db)
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("=== KRYVORA verify ==="));
    assert!(stdout.contains("events:       4"));
    assert!(stdout.contains("chain:        Intact (length 4)"));
    assert!(!stdout.contains("CHAIN INTEGRITY FAILURE"));
}

#[test]
fn verify_on_missing_database_fails() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("does-not-exist.db");

    let out = Command::new(bin())
        .arg("verify")
        .arg("--db")
        .arg(&db)
        .output()
        .unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("database does not exist"), "{stderr}");
}

#[test]
fn run_with_bad_source_fails() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("kryvora.db");
    let missing = dir.path().join("nope.bin");

    let out = Command::new(bin())
        .arg("run")
        .arg("--db")
        .arg(&db)
        .arg("--case-title")
        .arg("t")
        .arg("--source")
        .arg(&missing)
        .output()
        .unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not a regular file"), "{stderr}");
}

#[test]
fn sanitize_file_sanitizes_only_the_requested_file() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("kryvora.db");
    let target = write_source(dir.path(), "target.bin", b"disposable payload");
    let sibling = write_source(dir.path(), "sibling.bin", b"leave this file untouched");

    let out = Command::new(bin())
        .arg("sanitize-file")
        .arg("--db")
        .arg(&db)
        .arg("--target")
        .arg(&target)
        .arg("--yes")
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "sanitize-file failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("=== KRYVORA sanitize-file ==="), "{stdout}");
    assert!(stdout.contains("outcome:          partial"), "{stdout}");
    assert!(!stdout.contains("files discovered:"), "{stdout}");
    assert!(target.exists(), "safe unlink is deliberately not attempted");
    assert_ne!(
        std::fs::read(&target).unwrap(),
        b"disposable payload",
        "the selected file should be overwritten"
    );
    assert_eq!(
        std::fs::read(&sibling).unwrap(),
        b"leave this file untouched",
        "a neighboring file must not be sanitized"
    );
}
