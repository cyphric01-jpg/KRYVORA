//! Versioned migrations.
//!
//! A migration is an ordered, named SQL script. Each migration is
//! applied at most once; the tracking table `_migrations` records which
//! ones have been applied and when.
//!
//! The runner is deliberately small:
//!
//! * It does not support down-migrations. Forensic databases must not
//!   silently lose data on rollback. If a migration is wrong, the fix
//!   is a new forward migration.
//! * It runs each migration inside a transaction. If any statement
//!   fails, the transaction rolls back and the version is not recorded.
//! * It records the migration's content hash, so a later edit to an
//!   already-applied file is detected and rejected.

use kryvora_core::{Error, Result};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::OffsetDateTime;

/// A single migration, embedded at compile time.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// The complete, ordered set of migrations compiled into this binary.
///
/// New migrations are appended. Never edit or reorder an entry that has
/// shipped — the content hash check will reject it on the next run.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "0001_initial",
        sql: include_str!("../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "0002_provenance",
        sql: include_str!("../migrations/0002_provenance.sql"),
    },
    Migration {
        version: 3,
        name: "0003_reports",
        sql: include_str!("../migrations/0003_reports.sql"),
    },
    Migration {
        version: 4,
        name: "0004_sanitization_operations",
        sql: include_str!("../migrations/0004_sanitization_operations.sql"),
    },
    Migration {
        version: 5,
        name: "0005_recovered_artifact_paths",
        sql: include_str!("../migrations/0005_recovered_artifact_paths.sql"),
    },
];
/// A row from the `_migrations` tracking table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    pub version: u32,
    pub name: String,
    pub content_hash: String,
    pub applied_at: String,
}

/// Errors produced by the migration runner.
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error(
        "migration {version} ({name}) was applied with content hash {recorded}, \
             but the file now hashes to {current}; migrations are immutable"
    )]
    ContentChanged {
        version: u32,
        name: String,
        recorded: String,
        current: String,
    },

    #[error(
        "tracking table reports migration {version} ({name}) but this binary \
             does not contain that migration"
    )]
    UnknownAppliedMigration { version: u32, name: String },

    #[error("migrations list is not strictly ordered by version: {0}")]
    UnsortedMigrations(String),
}

/// Apply all pending migrations in order.
///
/// Idempotent: running twice on the same database is a no-op the second
/// time.
///
/// # Errors
///
/// * [`Error::Database`] — any SQL failure. The failing migration is
///   rolled back; `_migrations` is not updated for it.
/// * [`Error::Internal`] — `MigrationError` variants converted in.
pub fn apply_migrations(conn: &mut Connection) -> Result<Vec<AppliedMigration>> {
    ensure_sorted(MIGRATIONS).map_err(to_core_err)?;
    ensure_tracking_table(conn)?;

    let already = read_applied(conn)?;
    verify_history(MIGRATIONS, &already).map_err(to_core_err)?;

    let mut newly_applied = Vec::new();

    for migration in MIGRATIONS {
        if already.iter().any(|m| m.version == migration.version) {
            continue;
        }

        let tx = conn
            .transaction()
            .map_err(|e| Error::Database(format!("begin tx for {}: {e}", migration.name)))?;

        tx.execute_batch(migration.sql)
            .map_err(|e| Error::Database(format!("apply {}: {e}", migration.name)))?;

        let content_hash = hash_sql(migration.sql);
        let applied_at = format_rfc3339(OffsetDateTime::now_utc());

        tx.execute(
            "INSERT INTO _migrations (version, name, content_hash, applied_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![migration.version, migration.name, content_hash, applied_at,],
        )
        .map_err(|e| Error::Database(format!("record {}: {e}", migration.name)))?;

        tx.commit()
            .map_err(|e| Error::Database(format!("commit {}: {e}", migration.name)))?;

        newly_applied.push(AppliedMigration {
            version: migration.version,
            name: migration.name.to_string(),
            content_hash,
            applied_at,
        });
    }

    Ok(newly_applied)
}

fn ensure_tracking_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (\n\
             version      INTEGER NOT NULL PRIMARY KEY,\n\
             name         TEXT    NOT NULL,\n\
             content_hash TEXT    NOT NULL,\n\
             applied_at   TEXT    NOT NULL\n\
         ) STRICT;",
    )
    .map_err(|e| Error::Database(format!("create _migrations: {e}")))?;
    Ok(())
}

fn read_applied(conn: &Connection) -> Result<Vec<AppliedMigration>> {
    let mut stmt = conn
        .prepare(
            "SELECT version, name, content_hash, applied_at \
             FROM _migrations ORDER BY version ASC",
        )
        .map_err(|e| Error::Database(format!("prepare read _migrations: {e}")))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(AppliedMigration {
                version: row.get(0)?,
                name: row.get(1)?,
                content_hash: row.get(2)?,
                applied_at: row.get(3)?,
            })
        })
        .map_err(|e| Error::Database(format!("query _migrations: {e}")))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| Error::Database(format!("read _migrations row: {e}")))?);
    }
    Ok(out)
}

fn verify_history(
    known: &[Migration],
    applied: &[AppliedMigration],
) -> std::result::Result<(), MigrationError> {
    for a in applied {
        match known.iter().find(|m| m.version == a.version) {
            None => {
                return Err(MigrationError::UnknownAppliedMigration {
                    version: a.version,
                    name: a.name.clone(),
                });
            }
            Some(m) => {
                let current = hash_sql(m.sql);
                if current != a.content_hash {
                    return Err(MigrationError::ContentChanged {
                        version: a.version,
                        name: a.name.clone(),
                        recorded: a.content_hash.clone(),
                        current,
                    });
                }
            }
        }
    }
    Ok(())
}

fn ensure_sorted(migrations: &[Migration]) -> std::result::Result<(), MigrationError> {
    let mut prev: Option<u32> = None;
    for m in migrations {
        if let Some(p) = prev {
            if m.version <= p {
                return Err(MigrationError::UnsortedMigrations(format!(
                    "version {} follows {}",
                    m.version, p
                )));
            }
        }
        prev = Some(m.version);
    }
    Ok(())
}

fn hash_sql(sql: &str) -> String {
    let mut h = Sha256::new();
    h.update(sql.as_bytes());
    hex::encode(h.finalize())
}

fn format_rfc3339(t: OffsetDateTime) -> String {
    // Second precision, always 'Z'. Format: YYYY-MM-DDTHH:MM:SSZ.
    t.replace_nanosecond(0)
        .unwrap_or(t)
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn to_core_err(e: MigrationError) -> Error {
    Error::Internal(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::open_in_memory;
    use rusqlite::Connection;

    fn table_exists(conn: &Connection, name: &str) -> bool {
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                rusqlite::params![name],
                |r| r.get(0),
            )
            .unwrap();
        n == 1
    }

    #[test]
    fn migrations_are_sorted() {
        ensure_sorted(MIGRATIONS).unwrap();
    }

    #[test]
    fn apply_creates_expected_tables() {
        let mut conn = open_in_memory().unwrap();
        let applied = apply_migrations(&mut conn).unwrap();
        assert_eq!(applied.len(), MIGRATIONS.len());
        assert_eq!(applied[0].version, 1);
        assert_eq!(applied[0].name, "0001_initial");
        assert_eq!(applied[1].version, 2);
        assert_eq!(applied[1].name, "0002_provenance");

        // Tables created by 0001_initial.
        for t in [
            "cases",
            "evidence",
            "jobs",
            "audit_events",
            "sanitization_operations",
            "_migrations",
        ] {
            assert!(table_exists(&conn, t), "missing table: {t}");
        }
        // Tables created by 0002_provenance.
        for t in ["provenance_nodes", "recovery_results"] {
            assert!(table_exists(&conn, t), "missing table: {t}");
        }
    }

    #[test]
    fn apply_is_idempotent() {
        let mut conn = open_in_memory().unwrap();
        let first = apply_migrations(&mut conn).unwrap();
        assert_eq!(first.len(), MIGRATIONS.len());

        let second = apply_migrations(&mut conn).unwrap();
        assert!(second.is_empty(), "second run must be a no-op");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, MIGRATIONS.len() as i64);
    }

    #[test]
    fn existing_v4_database_upgrades_with_recovered_artifact_path() {
        let mut conn = open_in_memory().unwrap();
        ensure_tracking_table(&conn).unwrap();
        for migration in MIGRATIONS.iter().take(4) {
            let tx = conn.transaction().unwrap();
            tx.execute_batch(migration.sql).unwrap();
            tx.execute(
                "INSERT INTO _migrations (version, name, content_hash, applied_at) \
                 VALUES (?1, ?2, ?3, '2026-10-02T00:00:00Z')",
                rusqlite::params![migration.version, migration.name, hash_sql(migration.sql)],
            )
            .unwrap();
            tx.commit().unwrap();
        }

        let applied = apply_migrations(&mut conn).unwrap();
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].version, 5);
        let column_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('recovery_results') \
                 WHERE name = 'artifact_path'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(column_exists, 1);
    }

    #[test]
    fn editing_an_applied_migration_is_rejected() {
        // Simulate a migration file being edited after application by
        // rewriting the stored content hash to a wrong value.
        let mut conn = open_in_memory().unwrap();
        apply_migrations(&mut conn).unwrap();

        conn.execute(
            "UPDATE _migrations SET content_hash = ?1 WHERE version = 1",
            rusqlite::params!["0".repeat(64)],
        )
        .unwrap();

        let err = apply_migrations(&mut conn).unwrap_err();
        match err {
            Error::Internal(msg) => {
                assert!(msg.contains("immutable"), "unexpected: {msg}");
            }
            other => panic!("expected Error::Internal, got {other:?}"),
        }
    }

    #[test]
    fn foreign_keys_are_enforced() {
        // Inserting evidence with a non-existent case_id must fail.
        let mut conn = open_in_memory().unwrap();
        apply_migrations(&mut conn).unwrap();

        let result = conn.execute(
            "INSERT INTO evidence (\
                 id, case_id, source_type, source_path, size_bytes, \
                 hash_algorithm, hash_digest, integrity_state, \
                 read_only, tool_version, created_at\
             ) VALUES (\
                 'EVID-00000000000000000000000000000000', \
                 'CASE-does-not-exist', \
                 'file', '/tmp/x', 0, \
                 'sha256', \
                 '0000000000000000000000000000000000000000000000000000000000000000', \
                 'unknown', 1, '0.1.0', '2026-09-28T12:55:00Z'\
             )",
            [],
        );

        assert!(result.is_err(), "FK violation must be rejected");
    }
}
