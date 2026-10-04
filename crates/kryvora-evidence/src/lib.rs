//! Evidence registration and verification.
//!
//! This crate composes:
//!
//! * [`kryvora_core`] for typed IDs and errors.
//! * [`kryvora_integrity`] for the [`Hash`] value type.
//! * [`kryvora_db`] for persistence via `EvidenceRepository`.
//!
//! It performs **no file I/O**. The caller opens the source, computes
//! the hash with `kryvora_integrity::calculate_hash`, and passes both
//! the source path and the computed hash into [`register_evidence`].
//! This keeps the crate independent of *how* bytes are obtained — a
//! file today, a block device or memory image later — and preserves the
//! Section 4 responsibility split.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
// `unwrap()`, `expect()`, and `panic!` are permitted inside tests only.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod evidence;
pub mod source_type;

pub use evidence::{
    get_evidence, register_evidence, verify_evidence, Evidence, EvidenceError, RegistrationRequest,
};
pub use source_type::{SourceType, SourceTypeParseError};
