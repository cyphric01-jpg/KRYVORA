//! Repository for the `audit_events` table.
//!
//! This repository stores audit events. It **does not** compute the
//! hash chain. Chain computation (previous_hash + canonical_event_data →
//! current_hash) belongs to `kryvora-audit`, which is added later. The
//! repository takes `previous_hash`, `current_hash`, and `created_at`
//! from the caller. Hashes are validated for shape (64 hex chars), and the
//! supplied timestamp is stored verbatim.
//!
//! Sequence numbers are allocated by the repository, inside the same
//! transaction as the insert, using `MAX(sequence) + 1`. This keeps
//! sequence strictly increasing and gap-free for a single-writer
//! connection, which is what v0.1 has.

use super::map_sql_err;
use kryvora_core::{AuditEventId, Error, JobId, Result};
use rusqlite::Connection;

/// The genesis sentinel: 64 zero characters.
pub const GENESIS_PREVIOUS_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone)]
pub struct NewAuditEvent {
    pub event_type: String,
    pub actor: Option<String>,
    pub object_id: Option<String>,
    pub job_id: Option<JobId>,
    /// Canonical JSON of the event's details. The repository does not
    /// interpret this; it stores it verbatim.
    pub details: String,
    /// RFC 3339 timestamp included in the event hash and stored verbatim.
    pub created_at: String,
    /// Hex digest of the previous event, or [`GENESIS_PREVIOUS_HASH`]
    /// for the first event in the chain.
    pub previous_hash: String,
    /// The current event's hash. Computed by the caller.
    pub current_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEventRow {
    pub id: AuditEventId,
    pub sequence: u64,
    pub event_type: String,
    pub actor: Option<String>,
    pub object_id: Option<String>,
    pub job_id: Option<JobId>,
    pub details: String,
    pub previous_hash: String,
    pub current_hash: String,
    pub created_at: String,
}

pub trait AuditEventRepository {
    /// Append an event to the chain. Allocates the next sequence.
    /// Returns the new row's id and sequence.
    fn append(&self, new_event: &NewAuditEvent) -> Result<(AuditEventId, u64)>;

    fn get(&self, id: &AuditEventId) -> Result<Option<AuditEventRow>>;
    fn latest(&self) -> Result<Option<AuditEventRow>>;
    fn list_range(&self, from_sequence: u64, limit: u32) -> Result<Vec<AuditEventRow>>;
    fn count(&self) -> Result<u64>;
}

#[derive(Debug)]
pub struct SqliteAuditEventRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteAuditEventRepository<'a> {
    #[must_use]
    pub const fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Build and append an event while holding the database write lock.
    ///
    /// The callback receives the exact sequence and previous hash that will
    /// be persisted. This keeps chain hashing atomic with tip allocation.
    pub fn append_with_tip(
        &self,
        build: impl FnOnce(u64, String) -> Result<NewAuditEvent>,
    ) -> Result<(AuditEventId, u64)> {
        const SAVEPOINT: &str = "kryvora_audit_append";
        let owns_transaction = self.conn.is_autocommit();

        if owns_transaction {
            self.conn
                .execute_batch("BEGIN IMMEDIATE")
                .map_err(|e| map_sql_err("begin audit append", e))?;
        } else {
            self.conn
                .execute_batch(&format!("SAVEPOINT {SAVEPOINT}"))
                .map_err(|e| map_sql_err("begin nested audit append", e))?;
        }

        let result = (|| -> Result<(AuditEventId, u64)> {
            let tip = self.latest()?;
            let (sequence, previous_hash) = match tip {
                Some(row) => (
                    row.sequence
                        .checked_add(1)
                        .ok_or_else(|| Error::Internal("audit sequence overflow".into()))?,
                    row.current_hash,
                ),
                None => (0, GENESIS_PREVIOUS_HASH.to_string()),
            };
            let event = build(sequence, previous_hash)?;
            validate_event(&event)?;

            let sequence_i64 = i64::try_from(sequence).map_err(|_| {
                Error::Internal("audit sequence exceeds SQLite integer range".into())
            })?;
            let id = AuditEventId::new();
            self.conn
                .execute(
                    "INSERT INTO audit_events (\
                         id, sequence, event_type, actor, object_id, job_id, \
                         details, previous_hash, current_hash, created_at\
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    rusqlite::params![
                        id.to_string(),
                        sequence_i64,
                        event.event_type,
                        event.actor,
                        event.object_id,
                        event.job_id.as_ref().map(std::string::ToString::to_string),
                        event.details,
                        event.previous_hash.to_ascii_lowercase(),
                        event.current_hash.to_ascii_lowercase(),
                        event.created_at,
                    ],
                )
                .map_err(|e| map_sql_err("insert audit event", e))?;

            Ok((id, sequence))
        })();

        match result {
            Ok(value) => {
                let finish = if owns_transaction {
                    self.conn.execute_batch("COMMIT")
                } else {
                    self.conn
                        .execute_batch(&format!("RELEASE SAVEPOINT {SAVEPOINT}"))
                };
                finish.map_err(|e| map_sql_err("commit audit append", e))?;
                Ok(value)
            }
            Err(error) => {
                if owns_transaction {
                    let _ = self.conn.execute_batch("ROLLBACK");
                } else {
                    let _ = self.conn.execute_batch(&format!(
                        "ROLLBACK TO SAVEPOINT {SAVEPOINT}; RELEASE SAVEPOINT {SAVEPOINT}"
                    ));
                }
                Err(error)
            }
        }
    }
}

fn check_hash_shape(field: &str, value: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::InvalidInput(format!(
            "{field} must be 64 hex characters, got {} chars",
            value.len()
        )));
    }
    Ok(())
}

fn validate_event(new_event: &NewAuditEvent) -> Result<()> {
    check_hash_shape("previous_hash", &new_event.previous_hash)?;
    check_hash_shape("current_hash", &new_event.current_hash)?;
    if new_event.event_type.trim().is_empty() {
        return Err(Error::InvalidInput("event_type must not be empty".into()));
    }
    if new_event.details.trim().is_empty() {
        return Err(Error::InvalidInput("details must not be empty".into()));
    }
    Ok(())
}

impl AuditEventRepository for SqliteAuditEventRepository<'_> {
    fn append(&self, new_event: &NewAuditEvent) -> Result<(AuditEventId, u64)> {
        validate_event(new_event)?;

        // Sequence allocation and insert happen in the same transaction.
        // `Connection` is not `Sync`, and this repository takes `&Connection`,
        // so we cannot call `transaction()` (which needs `&mut`). We use
        // an explicit BEGIN/COMMIT/ROLLBACK pair instead.
        self.conn
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(|e| map_sql_err("begin audit insert", e))?;

        let result = (|| -> Result<(AuditEventId, u64)> {
            let next_seq: i64 = self
                .conn
                .query_row(
                    "SELECT COALESCE(MAX(sequence), -1) + 1 FROM audit_events",
                    [],
                    |r| r.get(0),
                )
                .map_err(|e| map_sql_err("allocate audit sequence", e))?;

            if next_seq < 0 {
                return Err(Error::Internal("audit sequence underflow".into()));
            }
            let next_seq_u64 = next_seq as u64;

            let id = AuditEventId::new();

            self.conn
                .execute(
                    "INSERT INTO audit_events (\
                         id, sequence, event_type, actor, object_id, job_id, \
                         details, previous_hash, current_hash, created_at\
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    rusqlite::params![
                        id.to_string(),
                        next_seq,
                        new_event.event_type,
                        new_event.actor,
                        new_event.object_id,
                        new_event
                            .job_id
                            .as_ref()
                            .map(std::string::ToString::to_string),
                        new_event.details,
                        new_event.previous_hash.to_ascii_lowercase(),
                        new_event.current_hash.to_ascii_lowercase(),
                        new_event.created_at,
                    ],
                )
                .map_err(|e| map_sql_err("insert audit event", e))?;

            Ok((id, next_seq_u64))
        })();

        match result {
            Ok(v) => {
                self.conn
                    .execute_batch("COMMIT")
                    .map_err(|e| map_sql_err("commit audit insert", e))?;
                Ok(v)
            }
            Err(e) => {
                // Best-effort rollback. If it also fails, the original
                // error is more informative, so we return that.
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    fn get(&self, id: &AuditEventId) -> Result<Option<AuditEventRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, sequence, event_type, actor, object_id, job_id, \
                        details, previous_hash, current_hash, created_at \
                 FROM audit_events WHERE id = ?1",
            )
            .map_err(|e| map_sql_err("prepare get audit event", e))?;

        let mut rows = stmt
            .query_map(rusqlite::params![id.to_string()], row_to_audit_event)
            .map_err(|e| map_sql_err("query get audit event", e))?;

        match rows.next() {
            None => Ok(None),
            Some(Ok(row)) => Ok(Some(row)),
            Some(Err(e)) => Err(map_sql_err("read audit event row", e)),
        }
    }

    fn latest(&self) -> Result<Option<AuditEventRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, sequence, event_type, actor, object_id, job_id, \
                        details, previous_hash, current_hash, created_at \
                 FROM audit_events ORDER BY sequence DESC LIMIT 1",
            )
            .map_err(|e| map_sql_err("prepare latest audit event", e))?;

        let mut rows = stmt
            .query_map([], row_to_audit_event)
            .map_err(|e| map_sql_err("query latest audit event", e))?;

        match rows.next() {
            None => Ok(None),
            Some(Ok(row)) => Ok(Some(row)),
            Some(Err(e)) => Err(map_sql_err("read latest audit event", e)),
        }
    }

    fn list_range(&self, from_sequence: u64, limit: u32) -> Result<Vec<AuditEventRow>> {
        if limit == 0 {
            return Err(Error::InvalidInput("list limit must be > 0".into()));
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, sequence, event_type, actor, object_id, job_id, \
                        details, previous_hash, current_hash, created_at \
                 FROM audit_events WHERE sequence >= ?1 \
                 ORDER BY sequence ASC LIMIT ?2",
            )
            .map_err(|e| map_sql_err("prepare list audit range", e))?;

        let rows = stmt
            .query_map(
                rusqlite::params![from_sequence as i64, limit],
                row_to_audit_event,
            )
            .map_err(|e| map_sql_err("query list audit range", e))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| map_sql_err("read audit event row", e))?);
        }
        Ok(out)
    }

    fn count(&self) -> Result<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM audit_events", [], |r| r.get(0))
            .map_err(|e| map_sql_err("count audit events", e))?;
        Ok(n.max(0) as u64)
    }
}

fn row_to_audit_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditEventRow> {
    let id_str: String = row.get(0)?;
    let seq_i: i64 = row.get(1)?;
    let job_str: Option<String> = row.get(5)?;

    let id = super::parse_stored_id(&id_str)
        .map(AuditEventId::from_uuid)
        .ok_or_else(|| conv_err(0, "audit_events.id is not a valid UUID"))?;

    if seq_i < 0 {
        return Err(conv_err(1, "audit_events.sequence is negative"));
    }

    let job_id = match job_str.as_deref() {
        None => None,
        Some(s) => Some(
            super::parse_stored_id(s)
                .map(JobId::from_uuid)
                .ok_or_else(|| conv_err(5, "audit_events.job_id is not a valid UUID"))?,
        ),
    };

    Ok(AuditEventRow {
        id,
        sequence: seq_i as u64,
        event_type: row.get(2)?,
        actor: row.get(3)?,
        object_id: row.get(4)?,
        job_id,
        details: row.get(6)?,
        previous_hash: row.get(7)?,
        current_hash: row.get(8)?,
        created_at: row.get(9)?,
    })
}

fn conv_err(idx: usize, msg: &'static str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        idx,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, msg)),
    )
}
