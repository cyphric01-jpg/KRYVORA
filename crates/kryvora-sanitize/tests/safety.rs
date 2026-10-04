#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration tests for system-path protection.

use kryvora_sanitize::{is_system_path, SystemPathReason};
use std::path::PathBuf;

#[test]
fn temporary_paths_are_not_protected() {
    let dir = tempfile::tempdir().unwrap();
    assert!(is_system_path(dir.path()).is_none());
}

#[test]
fn relative_paths_are_not_protected() {
    assert!(is_system_path(PathBuf::from("some/relative/path").as_path()).is_none());
}

#[test]
fn windows_system_root_is_protected_when_env_is_set() {
    // Only meaningful on Windows.
    let Ok(root) = std::env::var("SystemRoot") else {
        return;
    };
    let probe = PathBuf::from(&root).join("System32");
    if !probe.exists() {
        return;
    }
    match is_system_path(&probe) {
        Some(SystemPathReason::WindowsSystemRoot { .. }) => {}
        other => panic!("expected WindowsSystemRoot, got {other:?}"),
    }
}

#[test]
fn program_files_is_protected_when_env_is_set() {
    let Ok(root) = std::env::var("ProgramFiles") else {
        return;
    };
    let probe = PathBuf::from(&root).join("Common Files");
    if !probe.exists() {
        return;
    }
    match is_system_path(&probe) {
        Some(SystemPathReason::ProgramFiles { .. }) => {}
        other => panic!("expected ProgramFiles, got {other:?}"),
    }
}

#[test]
fn reason_code_is_stable() {
    let r = SystemPathReason::WindowsSystemRoot { root: "x".into() };
    assert_eq!(r.code(), "system_root");
    let r = SystemPathReason::ProgramFiles { root: "x".into() };
    assert_eq!(r.code(), "program_files");
}
