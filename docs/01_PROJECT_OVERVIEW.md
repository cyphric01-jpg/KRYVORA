# 01. Project Overview

## 1.1 Identity

KRYVORA is a focused digital forensics and secure data sanitization workstation built for Windows desktop operation. It brings together evidence registration, SHA-256 integrity validation, bounded carved-file recovery, provenance-aware results, audit-chain verification, and controlled sanitization workflows in a single security-oriented platform.

## 1.2 Problem Domain

Modern forensic and security investigation workflows require more than a single scan or a simple erase action. The examiner needs a reliable relationship between case context, evidence source, integrity values, recovered artifacts, and the operation history that produced them. Sanitization introduces an additional requirement: destructive actions must be transparent about scope, outcome, and safety boundaries.

KRYVORA addresses this challenge within a bounded, reviewable architecture. It preserves the provenance of regular-file evidence, records hash-based verification, and makes the system's operational limits explicit rather than masking them behind unsupported claims.

## 1.3 Intended Users

The platform is designed for:

- forensic investigators and digital evidence analysts
- incident response and security operations teams
- security researchers evaluating workflow integrity and recovery logic
- controlled lab-style or authorized case handling environments

## 1.4 Current Capabilities

| Capability | Status | Scope |
|---|---|---|
| Case and evidence registration | Implemented | Case creation and regular-file evidence registration with canonical path, size and digest |
| Evidence integrity | Implemented | Streaming SHA-256 verification with byte-size comparison and audit events |
| File/folder sanitization | Implemented & Safety-Controlled | Implemented with target validation, symlink/reparse protection, controlled processing, verification, and explicit outcome states |
| Drive sanitization | Controlled / Inspection & Planning Boundary | Device inspection, target identity assessment, safety policy, and sanitization planning are available; physical drive erase execution remains disabled pending platform-specific validation |
| File carving and recovery | Implemented — Supported Artifact Scope | Operational bounded carving and validated recovery for supported artifact formats, with integrity checks, validation, provenance, and controlled output |
| Validated recovery | Validated Recovery for Supported Formats | Unsupported or inconclusive candidates are not represented as successful recovery |
| Fragmented reconstruction | Future Extension | Fragmented-file reconstruction and advanced cross-fragment recovery are future extensions; current recovery stops at the validated supported-artifact boundary |
| Jobs | Implemented in scope | Job lifecycle persistence with workflow visibility focused on persisted results |
| Audit trail | Implemented | Canonical hash-linked event sequence and chain verification |
| Reporting | Implemented in scope | HTML recovery report creation with persisted metadata and digest accounting |
| Validation and performance views | Implemented in scope | Persistence and verification views are present; they do not claim exhaustive certification |

## 1.5 Technology Foundation

- React 18, TypeScript, Vite, and React Router
- Tauri 2 for desktop shell integration
- Rust 2021 workspace with a modular crate layout
- SQLite persistence through `rusqlite`
- SHA-256 hashing through `sha2`
- CLI workflows for supported evidence and recovery actions

## 1.6 Engineering Characteristics

The differentiators supported by the codebase are a shared Rust backend, streaming integrity hashing, audit chain verification, evidence-linked recovery metadata, explicit sanitization outcomes, and a transparent safety boundary around unsupported device-level operations. These are practical engineering characteristics for a disciplined forensic workstation rather than broad claims of universal certification.

## 1.7 SIH Alignment

KRYVORA aligns with the SIH requirement set through modular evidence handling, sanitization controls, recovery workflows, auditability, provenance, reporting, and validation. The project maps these capabilities directly to the repository documents and source evidence in the SIH traceability matrix.

See [04. SIH Requirement Mapping](04_SIH_REQUIREMENT_MAPPING.md).

## 1.8 Status Vocabulary

- Implemented: the described code path exists and is exercised by repository logic or tests.
- Boundary-limited: an operational capability has a defined supported scope or missing integration points.
- Controlled / inspection and planning boundary: device inspection or planning exists while destructive execution is disabled to preserve safety.
- Unsupported: the current application does not expose the capability as an active workflow.

## 1.9 Source References

The authoritative implementation is the workspace source and tests, including:

- `app/src/lib.rs` and `app/src/commands.rs`
- `frontend/src/App.tsx` and the route modules under `frontend/src/pages/`
- crate modules under `crates/`
- SQLite migrations under `crates/kryvora-db/migrations/`
- test files under each crate's `tests/` directory

Related references:

- [05. System Architecture](05_SYSTEM_ARCHITECTURE.md)
- [06. Module Documentation](06_MODULE_DOCUMENTATION.md)
- [15. Known Limitations](15_KNOWN_LIMITATIONS.md)
