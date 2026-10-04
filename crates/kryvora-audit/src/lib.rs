//! Tamper-evident audit chain.
//!
//! Every meaningful action in KRYVORA produces one row in `audit_events`.
//! Each row's `current_hash` is the SHA-256 of:
//!
//! ```text
//! previous_hash || 0x00 || sequence_be_u64 || 0x00 || event_type
//!               || 0x00 || created_at      || 0x00 || actor
//!               || 0x00 || object_id       || 0x00 || job_id
//!               || 0x00 || details_canonical_json
//! ```
//!
//! Any mutation to any row invalidates every subsequent row's hash.
//! [`verify_chain`] walks the chain from sequence 0 and reports the
//! first break, if any.
//!
//! This crate is deliberately **not** a keyed hash (no HMAC, no signature).
//! Section 14 of the master prompt asks for "tamper-evident", which a plain
//! hash chain provides. A keyed chain is a strictly stronger property and
//! would be a separate decision; the [`AuditChain`] API does not change if
//! that decision is later made.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod canonical;
pub mod chain;
pub mod event_type;

pub use chain::{
    append, count, latest_hash, verify_chain, AuditChain, AuditError, ChainBreakReason,
    ChainStatus, EventDraft,
};
pub use event_type::{EventType, EventTypeParseError};
