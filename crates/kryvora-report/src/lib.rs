//! HTML report generation.
//!
//! Two report kinds:
//!
//! * Recovery — lists validated artifacts with their source offsets,
//!   confidence, and validation facts.
//! * Sanitization — records the outcome of a file sanitization.
//!
//! Every report is written to a caller-supplied path. Its SHA-256
//! digest is computed with `kryvora-integrity` and returned alongside
//! the path so the caller can persist it.
//!
//! Report generation writes to disk. It does not touch the target of
//! any sanitization or recovery operation; it only writes the report
//! file itself.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod builder;
pub mod html;
pub mod persisted;

pub use builder::{
    generate_recovery_report, generate_sanitization_report, GeneratedReport, ReportError,
};
pub use html::{escape_html, render_recovery_html, render_sanitization_html};
pub use persisted::{
    generate_persisted_case_report, render_persisted_case_report, PersistedArtifact,
    PersistedAuditStatus, PersistedCaseReport, PersistedEvidence, PersistedJob,
};
