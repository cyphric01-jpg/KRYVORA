//! Policy: the layer between the UI and destructive execution.
//!
//! Every destructive operation must pass through this crate. It takes
//! a [`TargetProfile`](kryvora_storage::TargetProfile) and an
//! [`OperationRequest`], and returns an [`OperationPlan`]: either a
//! refusal with a reason, or an acceptance that names exactly which
//! confirmations are required and which warnings must be displayed.
//!
//! The crate is pure. It performs no I/O, holds no state, and consults
//! no external systems. Every decision is a function of its inputs.
//! That makes it exhaustively testable and auditable.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod decision;
pub mod operation;

pub use decision::{
    evaluate, Confirmation, PolicyDecision, PolicyError, SafetyAssessment, Warning,
};
pub use operation::{OperationKind, OperationRequest};
