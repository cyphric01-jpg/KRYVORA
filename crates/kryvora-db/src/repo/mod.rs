//! Repositories: the only place in KRYVORA that talks to SQLite.
//!
//! Each repository is a trait plus a concrete `Sqlite*` implementation
//! that borrows a `&rusqlite::Connection`. Callers never see
//! `rusqlite::Error`; all SQL errors are mapped into
//! [`kryvora_core::Error`] at the repository boundary.
//!
//! Domain types (`Case`, `Evidence`, `Job`, `AuditEvent`) do **not** live
//! in this crate. This crate owns *row* structs (`*Row`) that mirror the
//! schema one-to-one, and *insert* structs (`New*`) that carry the fields
//! a caller must supply. Higher-level crates (`kryvora-evidence`,
//! `kryvora-jobs`, `kryvora-audit`) convert between their domain types
//! and these rows. That preserves the Section 4 responsibility split.

use time::OffsetDateTime;

pub mod audit_events;
pub mod cases;
pub mod evidence;
pub mod jobs;
pub mod provenance;
pub mod recovery_results;
pub mod reports;
pub mod sanitization_operations;

pub use audit_events::{
    AuditEventRepository, AuditEventRow, NewAuditEvent, SqliteAuditEventRepository,
    GENESIS_PREVIOUS_HASH,
};
pub use cases::{CaseRepository, CaseRow, NewCase, SqliteCaseRepository};
pub use evidence::{EvidenceRepository, EvidenceRow, NewEvidence, SqliteEvidenceRepository};
pub use jobs::{JobRepository, JobRow, NewJob, SqliteJobRepository};
pub use provenance::{
    NewProvenanceNode, ProvenanceKind, ProvenanceNodeRow, ProvenanceRepository,
    SqliteProvenanceRepository,
};
pub use recovery_results::{
    NewRecoveryResult, RecoveryResultRepository, RecoveryResultRow, SqliteRecoveryResultRepository,
};
pub use reports::{NewReport, ReportKind, ReportRepository, ReportRow, SqliteReportRepository};
pub use sanitization_operations::{
    NewSanitizationOperation, SanitizationOperationRepository, SanitizationOperationRow,
    SqliteSanitizationOperationRepository,
};

/// Format a timestamp as RFC 3339 UTC with second precision.
///
/// This is the single source of truth for the timestamp wire format
/// agreed in the schema conventions.
pub(crate) fn now_rfc3339() -> String {
    format_rfc3339(OffsetDateTime::now_utc())
}

pub(crate) fn format_rfc3339(t: OffsetDateTime) -> String {
    t.replace_nanosecond(0)
        .unwrap_or(t)
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

/// Parse a stored ID string into a `uuid::Uuid`.
///
/// KRYVORA stores typed IDs using the same prefixed form that
/// `kryvora-core`'s `Display` impl produces, e.g.
/// `CASE-1a2b3c4d5e6f7890abcdef1234567890` or the equivalent with
/// `EVID-`, `JOB-`, `AUDIT-`, `PROV-`, `REC-`, `FRAG-`, `REPORT-`,
/// `SAN-` prefixes.
///
/// The function first tries the canonical hyphenated UUID form for
/// forward compatibility with future storage changes, then falls back
/// to `<PREFIX>-<32 hex chars>` (the `simple()` form produced by
/// `kryvora-core`).
///
/// Returns `None` on any malformed input.
pub(crate) fn parse_stored_id(s: &str) -> Option<uuid::Uuid> {
    if let Ok(u) = uuid::Uuid::parse_str(s) {
        return Some(u);
    }

    let (_, tail) = s.rsplit_once('-')?;
    if tail.len() == 32 {
        uuid::Uuid::parse_str(tail).ok()
    } else {
        None
    }
}

/// Map a `rusqlite::Error` into a [`kryvora_core::Error`] with context.
///
/// Constraint violations are folded into [`kryvora_core::Error::InvalidInput`]
/// because they are caused by caller-supplied data (duplicate key, FK
/// violation, CHECK failure) rather than by infrastructure failure.
/// Everything else becomes [`kryvora_core::Error::Database`].
pub(crate) fn map_sql_err(context: &str, e: rusqlite::Error) -> kryvora_core::Error {
    use kryvora_core::Error;
    use rusqlite::ErrorCode;

    if let rusqlite::Error::SqliteFailure(inner, ref msg) = e {
        match inner.code {
            ErrorCode::ConstraintViolation => {
                let detail = msg.as_deref().unwrap_or("constraint violation");
                return Error::InvalidInput(format!("{context}: {detail}"));
            }
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => {
                return Error::Database(format!("{context}: database busy or locked"));
            }
            _ => {}
        }
    }
    Error::Database(format!("{context}: {e}"))
}
