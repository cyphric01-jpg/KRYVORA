//! SQLite persistence for KRYVORA.
//!
//! This crate owns:
//!   * Opening and configuring a connection (see [`connection`]).
//!   * Applying versioned migrations (see [`migrate`]).
//!   * Repositories that map between Rust types and the schema
//!     (see [`repo`]).

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
// `unwrap()`, `expect()`, and `panic!` are permitted inside tests only.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod connection;
pub mod migrate;
pub mod repo;

pub use connection::open;
pub use migrate::{apply_migrations, AppliedMigration, MigrationError, MIGRATIONS};
pub use repo::{
    AuditEventRepository, AuditEventRow, CaseRepository, CaseRow, EvidenceRepository, EvidenceRow,
    JobRepository, JobRow, NewAuditEvent, NewCase, NewEvidence, NewJob, NewProvenanceNode,
    NewRecoveryResult, NewReport, ProvenanceKind, ProvenanceNodeRow, ProvenanceRepository,
    RecoveryResultRepository, RecoveryResultRow, ReportKind, ReportRepository, ReportRow,
    SqliteAuditEventRepository, SqliteCaseRepository, SqliteEvidenceRepository,
    SqliteJobRepository, SqliteProvenanceRepository, SqliteRecoveryResultRepository,
    SqliteReportRepository, GENESIS_PREVIOUS_HASH,
};
