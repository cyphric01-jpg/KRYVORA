//! Job lifecycle.
//!
//! A *job* is a long-running operation with a defined start, a
//! progress-reporting body, cancellation support, and a terminal state.
//! Every job is persisted to `jobs` and emits audit events at
//! creation, start, completion, and failure.
//!
//! The crate is deliberately synchronous. The runner executes a
//! caller-supplied closure on the calling thread. Concurrency — running
//! the job on a worker thread so the UI stays responsive — is the
//! caller's concern. This keeps the crate small and eliminates an
//! entire class of "which async runtime?" decisions that would have
//! dominated the design.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod cancel;
pub mod runner;

pub use cancel::CancelToken;
pub use runner::{run_job, JobContext, JobOutcome, JobRequest, JobRunnerError};
