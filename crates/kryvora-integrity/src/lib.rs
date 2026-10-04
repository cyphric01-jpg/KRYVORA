//! Streaming hashing and integrity verification.
//!
//! This crate is intentionally narrow:
//!
//! * It knows how to read bytes from any [`std::io::Read`] and produce a
//!   SHA-256 digest using a **bounded** buffer. It never loads the whole
//!   input into memory.
//! * It knows how to compare a freshly computed digest against an
//!   expected digest and return an explicit [`IntegrityState`].
//! * It does not open files, does not touch the filesystem, and does not
//!   depend on any storage or database crate. The caller supplies the
//!   reader.
//!
//! The result type [`Hash`] carries the algorithm, digest, input size,
//! and timestamp so that downstream audit and provenance layers have
//! everything they need without re-hashing.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
// `unwrap()` and `expect()` are permitted inside tests only.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod hash;
pub mod streaming;

pub use hash::{Algorithm, Hash, HashError};
pub use streaming::{calculate_hash, verify_hash, DEFAULT_BUFFER_SIZE};
