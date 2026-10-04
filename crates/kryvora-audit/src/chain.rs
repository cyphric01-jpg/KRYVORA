//! The public API of the audit chain: append, verify, count, latest.
//!
//! The chain is a linear sequence of rows in `audit_events`. Each row's
//! `current_hash` is `SHA-256(canonical_bytes(previous_hash, sequence,
//! event_type, created_at, actor, object_id, job_id, details))`.
//!
//! Appending acquires the database write lock before reading the tip,
//! computing the event hash, and inserting the event.

use crate::canonical::{compute_hash, CanonicalInput};
use crate::event_type::EventType;
use kryvora_core::{AuditEventId, JobId, Result};
use kryvora_db::repo::{
    AuditEventRepository, AuditEventRow, NewAuditEvent, SqliteAuditEventRepository,
    GENESIS_PREVIOUS_HASH,
};
use rusqlite::Connection;
use serde_json::Value;
use thiserror::Error;

/// Caller-supplied fields for a new audit event.
///
/// `sequence`, `previous_hash`, and `current_hash` are computed by
/// [`append`]; they are not part of the draft.
#[derive(Debug, Clone)]
pub struct EventDraft {
    pub event_type: EventType,
    pub actor: Option<String>,
    pub object_id: Option<String>,
    pub job_id: Option<JobId>,
    /// Required. Pass `serde_json::json!({})` if there is nothing to add.
    pub details: Value,
}

/// Result of walking the chain from sequence 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainStatus {
    /// No rows yet. Not an error; the chain is simply not started.
    Empty,

    /// Every row verified. `length` is the number of rows checked.
    Intact { length: u64 },

    /// The chain broke at `first_bad_sequence`. Every row after that
    /// point is unverifiable, so only the first break is reported.
    Broken {
        first_bad_sequence: u64,
        reason: ChainBreakReason,
    },
}

/// Why a chain broke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainBreakReason {
    /// Expected a specific sequence number but found a different one.
    /// Covers deleted rows (found > expected) and any situation where
    /// the sequence numbers themselves are not contiguous.
    SequenceGap { expected: u64, found: u64 },

    /// The row's `previous_hash` is not the previous row's `current_hash`.
    /// Covers row swaps and any edit to the cryptographic links.
    PreviousHashMismatch { expected: String, found: String },

    /// The row's `current_hash` does not equal the SHA-256 of its
    /// canonical bytes. Covers edits to `event_type`, `created_at`,
    /// `actor`, `object_id`, `job_id`, or `details`.
    CurrentHashMismatch { expected: String, found: String },

    /// A row could not be interpreted (invalid timestamp, invalid
    /// event_type, etc.). The message carries the detail.
    MalformedRow { message: String },
}

/// Errors produced by the audit API above and beyond
/// [`kryvora_core::Error`].
#[derive(Debug, Error)]
pub enum AuditError {
    #[error("event_type {value:?} is not a known audit event type")]
    UnknownEventType { value: String },

    #[error("row {sequence} has an invalid created_at: {message}")]
    InvalidTimestamp { sequence: u64, message: String },

    #[error("chain invariant violated: {0}")]
    Invariant(String),
}

/// Append a new event to the chain.
///
/// Reads the current tip, computes `sequence`, `previous_hash`, and
/// `current_hash`, and inserts through the repository. Returns the new
/// row's id.
///
/// # Errors
///
/// * [`Error::Database`] — any SQL failure in the tip read or insert.
/// * [`Error::Internal`] — invariant violation (never expected in
///   practice; the canonical format has no failure modes).
pub fn append(conn: &Connection, draft: &EventDraft) -> Result<AuditEventId> {
    let created_at = now_rfc3339();
    append_at(conn, draft, &created_at)
}

fn append_at(conn: &Connection, draft: &EventDraft, created_at: &str) -> Result<AuditEventId> {
    let repo = SqliteAuditEventRepository::new(conn);
    let (id, _sequence) = repo.append_with_tip(|sequence, previous_hash| {
        let canonical = CanonicalInput {
            previous_hash: previous_hash.clone(),
            sequence,
            event_type: draft.event_type.as_str().to_string(),
            created_at: created_at.to_string(),
            actor: draft.actor.clone(),
            object_id: draft.object_id.clone(),
            job_id: draft.job_id.map(|j| j.to_string()),
            details: draft.details.clone(),
        };
        let current_hash = compute_hash(&canonical);
        let details = crate::canonical::canonical_json(&draft.details);

        Ok(NewAuditEvent {
            event_type: draft.event_type.as_str().to_string(),
            actor: draft.actor.clone(),
            object_id: draft.object_id.clone(),
            job_id: draft.job_id,
            details,
            created_at: created_at.to_string(),
            previous_hash,
            current_hash,
        })
    })?;
    Ok(id)
}

/// Return the current tip's `current_hash`, or the genesis sentinel if
/// the chain is empty. Useful for callers that want to record the
/// chain head alongside other state.
///
/// # Errors
///
/// Propagates any repository error.
pub fn latest_hash(conn: &Connection) -> Result<String> {
    let repo = SqliteAuditEventRepository::new(conn);
    match repo.latest()? {
        None => Ok(GENESIS_PREVIOUS_HASH.to_string()),
        Some(row) => Ok(row.current_hash),
    }
}

/// Count events in the chain.
///
/// # Errors
///
/// Propagates any repository error.
pub fn count(conn: &Connection) -> Result<u64> {
    let repo = SqliteAuditEventRepository::new(conn);
    repo.count()
}

/// Walk the entire chain from sequence 0 and verify every hash.
///
/// `O(n)` in the number of rows. Reads them in one query ordered by
/// sequence, then recomputes each `current_hash` and checks that each
/// row's `previous_hash` equals the prior row's `current_hash`.
///
/// Returns the first break if the chain is not intact.
///
/// # Errors
///
/// Propagates any repository error from reading the rows. A *malformed*
/// row is reported as `ChainStatus::Broken { reason: MalformedRow }`,
/// not as an `Err`, because a malformed row is a chain integrity
/// finding, not an infrastructure failure.
pub fn verify_chain(conn: &Connection) -> Result<ChainStatus> {
    let rows = read_all(conn)?;

    if rows.is_empty() {
        return Ok(ChainStatus::Empty);
    }

    // `expected_previous` is the hash that the next row's `previous_hash`
    // field must equal. It starts at the genesis sentinel and advances
    // to each row's `current_hash` after that row is verified.
    let mut expected_previous: String = GENESIS_PREVIOUS_HASH.to_string();

    // The loop index is the expected sequence number. `enumerate()`
    // yields `usize`; `index as u64` is lossless on 64-bit platforms
    // and cannot overflow for any realistic chain length on 32-bit.
    for (index, row) in rows.iter().enumerate() {
        let expected_sequence = index as u64;

        // 1. Sequence monotonicity.
        if row.sequence != expected_sequence {
            return Ok(ChainStatus::Broken {
                first_bad_sequence: expected_sequence,
                reason: ChainBreakReason::SequenceGap {
                    expected: expected_sequence,
                    found: row.sequence,
                },
            });
        }

        // 2. previous_hash link.
        if row.previous_hash != expected_previous {
            return Ok(ChainStatus::Broken {
                first_bad_sequence: row.sequence,
                reason: ChainBreakReason::PreviousHashMismatch {
                    expected: expected_previous.clone(),
                    found: row.previous_hash.clone(),
                },
            });
        }

        // 3. Recompute current_hash from the row's own fields.
        let event_type = match EventType::parse(&row.event_type) {
            Ok(t) => t,
            Err(e) => {
                return Ok(ChainStatus::Broken {
                    first_bad_sequence: row.sequence,
                    reason: ChainBreakReason::MalformedRow {
                        message: format!("event_type: {e}"),
                    },
                });
            }
        };

        let details_value: Value = match serde_json::from_str(&row.details) {
            Ok(v) => v,
            Err(e) => {
                return Ok(ChainStatus::Broken {
                    first_bad_sequence: row.sequence,
                    reason: ChainBreakReason::MalformedRow {
                        message: format!("details: {e}"),
                    },
                });
            }
        };

        let canonical = CanonicalInput {
            previous_hash: row.previous_hash.clone(),
            sequence: row.sequence,
            event_type: event_type.as_str().to_string(),
            created_at: row.created_at.clone(),
            actor: row.actor.clone(),
            object_id: row.object_id.clone(),
            job_id: row.job_id.map(|j| j.to_string()),
            details: details_value,
        };

        let recomputed = compute_hash(&canonical);
        if recomputed != row.current_hash {
            return Ok(ChainStatus::Broken {
                first_bad_sequence: row.sequence,
                reason: ChainBreakReason::CurrentHashMismatch {
                    expected: recomputed,
                    found: row.current_hash.clone(),
                },
            });
        }

        expected_previous = row.current_hash.clone();
    }

    Ok(ChainStatus::Intact {
        length: rows.len() as u64,
    })
}

/// A working handle that caches nothing but gives a name to the pattern
/// `SqliteAuditEventRepository::new(conn)` repeated at every call site.
///
/// Borrows the connection for its lifetime.
#[derive(Debug)]
pub struct AuditChain<'a> {
    conn: &'a Connection,
}

impl<'a> AuditChain<'a> {
    #[must_use]
    pub const fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn append(&self, draft: &EventDraft) -> Result<AuditEventId> {
        append(self.conn, draft)
    }

    pub fn latest_hash(&self) -> Result<String> {
        latest_hash(self.conn)
    }

    pub fn count(&self) -> Result<u64> {
        count(self.conn)
    }

    pub fn verify(&self) -> Result<ChainStatus> {
        verify_chain(self.conn)
    }
}

/// Read every row from `audit_events` in `sequence` order.
fn read_all(conn: &Connection) -> Result<Vec<AuditEventRow>> {
    let repo = SqliteAuditEventRepository::new(conn);
    let n = repo.count()?;
    if n == 0 {
        return Ok(Vec::new());
    }
    // Fetch in one call. `u32` is safe because the schema's UNIQUE on
    // sequence and the practical size of a forensic audit chain keep
    // us well under u32::MAX. If a future case exceeds that, this
    // becomes a paginated loop.
    let limit = u32::try_from(n).unwrap_or(u32::MAX);
    repo.list_range(0, limit)
}

/// Format the current time as RFC 3339 UTC with second precision.
fn now_rfc3339() -> String {
    use time::OffsetDateTime;
    let t = OffsetDateTime::now_utc();
    t.replace_nanosecond(0)
        .unwrap_or(t)
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

#[cfg(test)]
mod tests {
    // Integration-level tests live in `tests/chain.rs`. The unit tests
    // here exercise only the small pure helpers.

    #[test]
    fn rfc3339_is_second_precision() {
        let s = super::now_rfc3339();
        // Ends with 'Z', contains no fractional seconds.
        assert!(s.ends_with('Z'), "{s}");
        assert!(!s.contains('.'), "{s}");
    }

    #[test]
    fn append_hashes_and_persists_the_same_timestamp_at_a_second_boundary() {
        use super::{append_at, verify_chain, ChainStatus, EventDraft, EventType};
        use kryvora_db::{apply_migrations, repo::AuditEventRepository};
        use rusqlite::Connection;
        use serde_json::json;

        let mut conn = Connection::open_in_memory().unwrap();
        apply_migrations(&mut conn).unwrap();
        // A fixed value immediately before a minute boundary makes the test
        // independent of wall-clock timing and avoids sleeps.
        let created_at = "2020-01-01T00:00:59Z";
        let draft = EventDraft {
            event_type: EventType::CaseCreated,
            actor: Some("examiner".into()),
            object_id: Some("CASE-timestamp".into()),
            job_id: None,
            details: json!({"tag": "timestamp"}),
        };

        let id = append_at(&conn, &draft, created_at).unwrap();
        let repo = kryvora_db::repo::SqliteAuditEventRepository::new(&conn);
        let stored = repo.get(&id).unwrap().unwrap();

        assert_eq!(stored.created_at, created_at);
        assert_eq!(
            verify_chain(&conn).unwrap(),
            ChainStatus::Intact { length: 1 }
        );
    }
}
