// file: crates/kryvora-storage/src/lib.rs
//! Target inspection.
//!
//! This crate answers factual questions about a target — a regular
//! file, a directory, a filesystem, or a block device — and nothing
//! else. It performs no writes, holds no state, and makes no decisions.
//!
//! The output of every inspection is a [`TargetProfile`]: a
//! serializable description of what was observed. Policy decisions are
//! made by `kryvora-policy` on top of the profile. Operations are run
//! by `kryvora-jobs`.
//!
//! The crate is deliberately conservative. When it cannot determine a
//! fact on the current platform, it records the fact as `Unknown`
//! rather than guessing. `TargetKind::Unknown` is a first-class value.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod device;
pub mod file;
pub mod target;

pub use device::{
    assess_device_target, inspect_device_by_id, is_volume_root, list_devices, DeviceError,
    DeviceSafetyAssessment, DeviceSafetyDecision,
};
pub use file::{inspect_directory, inspect_file, FileInspectionError, TargetIdentity};
pub use target::{FilesystemInfo, InspectionError, TargetKind, TargetProfile, TargetWarning};
