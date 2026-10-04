# 01. Project Overview

## 1.1 Identity

**KRYVORA — Digital Forensics & Secure Data Sanitization Workstation** is a Windows desktop prototype for registering evidence, recording and checking SHA-256 integrity, running limited signature carving, retaining recovery results with source offsets, maintaining a hash-linked audit log, and generating HTML recovery reports. It combines those forensic workflows with file/folder overwrite attempts whose outcomes are deliberately limited by platform and media guarantees.

## 1.2 Problem domain

Investigations need more than a file-recovery command: source identity, integrity checks, analysis results, provenance, operation records and report output must remain connected. Sanitization is a separate destructive workflow and must not be described as secure merely because bytes were overwritten. KRYVORA's architecture puts these concerns in one Rust-backed desktop application; it is not a full acquisition or forensic analysis suite.

## 1.3 Users and use

Intended users include digital-forensics investigators, incident responders, SOC/security teams and security researchers evaluating the prototype. Use only authorized evidence and disposable targets for sanitization experiments.

## 1.4 Current capabilities

| Capability | Status | Scope |
|---|---|---|
| Case and evidence registry | IMPLEMENTED | Case CRUD is limited to create/list; regular-file evidence is registered with path, size, SHA-256 and a read-only flag. |
| Evidence integrity | IMPLEMENTED | Streaming SHA-256 and size comparison; verification emits an audit event. Reverification does not update the stored evidence row. |
| File/folder sanitization | PARTIAL | Random overwrite, digest comparison, path protections and audit details exist. Safe unlink is intentionally disabled; no physical-media guarantee. |
| Drive sanitization | UNSUPPORTED | Storage inspection and policy models exist in Rust, but no drive erase command is registered with Tauri. |
| Carving and recovery | PARTIAL | Contiguous JPEG, PNG and PDF signature candidates; format validators, confidence assessment, result persistence and source offsets. No fragmented reconstruction. |
| Jobs | PARTIAL | Rust runner persists lifecycle/progress and audit events; desktop exposes list only, without create/cancel controls or live event streaming. |
| Audit | IMPLEMENTED | Append and full-chain verification over canonical event data. It is tamper-evident, not an externally anchored signature. |
| Reports | PARTIAL | Tauri creates and hashes HTML recovery reports. Sanitization-report generation exists in a Rust crate but has no Tauri command/UI generation path. |
| Validation, documentation, performance UI | UI WORKFLOW ONLY | Validation displays persisted states; documentation is an index; performance UI has no live metrics (manual synthetic baselines are documented separately). |

## 1.5 Technology

- React 18, TypeScript, React Router and Vite.
- Tauri 2 desktop shell and command bridge.
- Rust 2021 workspace; toolchain pinned to 1.90.0.
- SQLite through `rusqlite`, with three migrations.
- SHA-256 through `sha2`; signature scanning uses `memchr`.
- Rust CLI alongside the desktop app.

## 1.6 Engineering characteristics

The differentiators supported by source are a shared Rust domain/backend, streaming integrity hashing, evidence-linked recovery offsets, a canonical hash chain, explicit sanitization outcomes, and transparent unsupported states. These are prototype engineering characteristics, not claims of forensic certification or production readiness.

## 1.7 SIH alignment

The desktop includes a dashboard and workflows for evidence, limited carving, file/folder overwrite attempts, audit and recovery reporting. Drive erasure is unsupported. See [04. SIH Requirement Mapping](04_SIH_REQUIREMENT_MAPPING.md) for the traceability matrix and counted coverage.

## 1.8 Status vocabulary

- **IMPLEMENTED**: the described code path is present and testable.
- **PARTIAL**: a real path exists but scope, integration or guarantee is incomplete.
- **UI WORKFLOW ONLY**: presentation/interaction exists without an end-to-end backend capability.
- **UNSUPPORTED**: the current application explicitly cannot perform the operation.
- **NOT IMPLEMENTED**: no current operation exists for the described capability.
- **PLANNED**: proposed future work, not a current feature.

## 1.9 Source references

See [05. System Architecture](05_SYSTEM_ARCHITECTURE.md), [06. Module Documentation](06_MODULE_DOCUMENTATION.md), and [15. Known Limitations](15_KNOWN_LIMITATIONS.md). The implementation authority is the Rust source, Tauri handler registration in `app/src/lib.rs`, frontend routes in `frontend/src/App.tsx`, migrations in `crates/kryvora-db/migrations/`, and tests under each crate's `tests/` directory.
