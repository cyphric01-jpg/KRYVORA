//! Signature-based file carving.
//!
//! This crate scans a byte source — a file, a disk image, a memory
//! image — for the byte signatures of known file formats, and emits
//! **candidates**: byte ranges that begin with a valid header and end
//! with a valid footer within a bounded window.
//!
//! A candidate is not a recovered file. It is a claim that a byte range
//! *might* contain a file of a known format. Validation, structural
//! checking, reconstruction, classification, and confidence assessment
//! are the responsibility of `kryvora-recovery`, which is delivered in
//! a later batch.
//!
//! The crate performs **no writes**. It reads the source; it does not
//! modify it, and it does not write carved artifacts to disk.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod candidate;
pub mod job;
pub mod scanner;
pub mod signature;
pub mod signatures;

pub use candidate::{CarvedCandidate, CarvingError, CarvingReport};
pub use job::{
	run_carving_job, run_carving_job_for_evidence, run_carving_job_for_evidence_with_cancel,
};
pub use scanner::{scan_source, scan_source_with_checkpoint, ScanOptions};
pub use signature::Signature;
pub use signatures::all_signatures;
