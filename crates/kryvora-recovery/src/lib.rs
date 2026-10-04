//! Structural validation, classification, and confidence for
//! carved candidates.
//!
//! A *candidate* from `kryvora-carving` becomes an *artifact* here, but
//! only after passing a format-specific structural validator. A
//! candidate that fails validation is discarded; it is not promoted.
//!
//! This crate performs **no writes to disk**. It reads candidate bytes
//! into memory (bounded by the candidate's length, which is bounded by
//! the signature's `max_size`), validates them, and returns them as
//! [`RecoveryResult`] values. Persisting results and writing artifact
//! bytes is deferred to a later batch.
//!
//! Confidence is computed deterministically from explicit signals.
//! Every assessment carries its reasons so an analyst can see *why* a
//! particular level was assigned.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod artifact;
pub mod confidence;
pub mod job;
pub mod result;
pub mod validator;

pub use artifact::{ArtifactCategory, RecoveryMethod};
pub use confidence::{ConfidenceAssessment, ConfidenceReason, ConfidenceSignal};
pub use job::{run_recovery_job, run_recovery_job_for_case};
pub use result::{RecoveryError, RecoveryReport, RecoveryResult};
pub use validator::{validator_for, FormatValidator, ValidationOutcome, ValidationReason};
