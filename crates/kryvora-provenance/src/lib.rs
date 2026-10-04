//! Evidence-to-artifact provenance graph.
//!
//! Every recovered artifact traces back through this graph to the
//! evidence it came from, the job that recovered it, and the candidate
//! that produced it. The graph is a DAG rooted at the evidence node.
//!
//! # Node kinds
//!
//! * `evidence` — the top of a chain.
//! * `candidate` — a carved candidate.
//! * `job` — a job that produced downstream nodes.
//! * `artifact` — a validated recovery result.
//! * `report` — a report that references the artifact.
//!
//! # Guarantees
//!
//! * Every row's `parent_id`, if set, references an existing row.
//! * Every chain eventually reaches an `evidence` node.
//! * The graph is acyclic by construction: a node's parent is always
//!   inserted before the node itself.
//!
//! The crate owns no foreign keys to `recovery_results`; it stores
//! the link on the `recovery_results` row via `provenance_id`.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod chain;
pub mod graph;
pub mod node;

pub use chain::{ensure_evidence_root, persist_recovery_result, ChainInputs};
pub use graph::{ProvenanceGraph, ProvenancePath, ProvenancePathElement};
pub use node::{NodeKind, ProvenanceNodeRef};
