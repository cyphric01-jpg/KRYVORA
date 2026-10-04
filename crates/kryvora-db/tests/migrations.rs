#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration tests for the migration runner against a real on-disk
//! database file.

use kryvora_db::{apply_migrations, open, MIGRATIONS};

#[test]
fn migrations_persist_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kryvora.sqlite");

    {
        let mut conn = open(&path).unwrap();
        let applied = apply_migrations(&mut conn).unwrap();
        assert_eq!(applied.len(), MIGRATIONS.len());
    }

    {
        let mut conn = open(&path).unwrap();
        let applied = apply_migrations(&mut conn).unwrap();
        assert!(applied.is_empty(), "reopen must not re-apply");
    }
}

#[test]
fn journal_mode_is_wal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kryvora.sqlite");
    let conn = open(&path).unwrap();

    let mode: String = conn
        .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode.to_lowercase(), "wal");
}
