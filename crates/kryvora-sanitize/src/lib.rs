// file: crates/kryvora-sanitize/src/lib.rs
//! Secure file and directory sanitization.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod directory;
pub mod file;
pub mod media;
pub mod outcome;
pub mod result;
pub mod safety;

pub use directory::{
	sanitize_directory, sanitize_directory_bound, FileFailure, SanitizeDirectoryReport,
};
pub use file::{sanitize_file, sanitize_file_bound, SanitizeOptions};
pub use media::{detect_media_type, MediaType};
pub use outcome::SanitizeOutcome;
pub use result::{SanitizationResult, SanitizeError};
pub use safety::{is_system_path, SystemPathReason};
