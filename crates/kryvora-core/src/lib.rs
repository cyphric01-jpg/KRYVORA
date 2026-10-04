//! KRYVORA core domain.
//!
//! This crate contains **only** pure domain logic: typed identifiers,
//! structured errors, and state enums. It must not perform I/O,
//! spawn tasks, or depend on any database or GUI crate.
//!
//! Note on `Default` for identifiers: every typed ID implements `Default`
//! by generating a fresh random UUID. This is required by some persistence
//! and serde patterns, but callers should prefer explicit `::new()` at
//! construction sites for clarity.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
// `unwrap()` and `expect()` are permitted inside tests only. Production
// code is still governed by the workspace-level clippy deny.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod error;
pub mod ids;
pub mod state;

pub use error::{Error, ErrorKind, Result};
pub use ids::{
    AuditEventId, CaseId, EvidenceId, FragmentId, JobId, ProvenanceId, RecoveryResultId, ReportId,
    SanitizationOperationId,
};
pub use state::{
    Confidence, IntegrityState, JobState, ReconstructionState, SanitizationOperationState,
    ValidationState,
};
