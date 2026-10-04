# KRYVORA Architecture Summary

This file is a short repository map. The evaluator-facing, source-checked architecture is in [docs/05_SYSTEM_ARCHITECTURE.md](docs/05_SYSTEM_ARCHITECTURE.md); implementation status is defined in [docs/01_PROJECT_OVERVIEW.md](docs/01_PROJECT_OVERVIEW.md).

## 1. Repository layers

| Layer | Location | Responsibility |
|---|---|---|
| Frontend | `frontend/` | React, TypeScript, Vite, routes and Tauri IPC calls |
| Desktop integration | `app/` | Tauri 2 startup, application-data database path and command handlers; separate Cargo workspace |
| Domain/workflow crates | `crates/` | Core IDs/errors, SQLite, evidence, integrity, storage, policy, sanitization, carving, recovery, jobs, audit, provenance and reports |
| Database schema | `crates/kryvora-db/migrations/` | Three SQLite migrations for cases/evidence/jobs/audit, recovery/provenance and reports |
| Documentation | `docs/` | SIH requirement map, technical package, manual, testing, limitations and roadmap |

## 2. Crate status

| Crate | Responsibility | Current status |
|---|---|---|
| `kryvora-core` | Typed IDs, errors and state enums | IMPLEMENTED |
| `kryvora-integrity` | Streaming SHA-256 and digest/size verification | IMPLEMENTED |
| `kryvora-db` | SQLite migrations and repositories | IMPLEMENTED |
| `kryvora-evidence` | Evidence metadata validation and integrity comparison | IMPLEMENTED |
| `kryvora-audit` | Append and verify canonical local hash chain | IMPLEMENTED; no external anchor |
| `kryvora-provenance` | Evidence → candidate → artifact persistence | PARTIAL; job/report relationships incomplete |
| `kryvora-jobs` | Persisted job lifecycle/progress/audit | PARTIAL at product level; Tauri lists jobs only |
| `kryvora-storage` | File/directory/device profile inspection | PARTIAL; not exposed through device Tauri commands |
| `kryvora-policy` | Pure target/operation assessments and confirmation models | PARTIAL; not called by sanitizer Tauri handlers |
| `kryvora-sanitize` | File/folder overwrite, digest comparison and outcomes | PARTIAL; unlink disabled; no drive erase |
| `kryvora-carving` | Streaming signature scanner and candidates | PARTIAL; JPEG, PNG, PDF |
| `kryvora-recovery` | Validators, classification and confidence | PARTIAL; contiguous supported formats |
| `kryvora-report` | Recovery and sanitization HTML builders | PARTIAL at desktop level; Tauri exposes recovery reports only |
| `kryvora-cli` | CLI entry point/workflows | IMPLEMENTED; separate from desktop UI |
| `kryvora-app` | Tauri command orchestration | PARTIAL integration; GUI E2E coverage absent |

## 3. Important implementation boundaries

1. Tauri exposes case/evidence, hash/verify, file/folder sanitize, carve/recovery queries, jobs/audit/report queries and recovery report generation. It does not expose drive erasure, device inspection, acquisition, filesystem analysis or report re-verification.
2. Evidence registration stores a canonical regular-file path, SHA-256, size and metadata; it does not acquire an image or make the source OS-immutable.
3. Sanitizer commands accept a boolean confirmation and do not invoke `kryvora-policy`; the UI typed phrase is not validated by Rust.
4. Audit is a local SHA-256-linked sequence. Not every action is recorded, and there is no external signature/anchor.
5. Recovery provenance currently links evidence → candidate → artifact. Result job IDs and report provenance are incomplete.
6. File/folder overwrite does not safely unlink; it returns an explicit partial/not-verified outcome when guarantees are insufficient.
7. Tauri report output is confined to its app-owned directory and created without replacement.

## 4. Source of truth

When this summary conflicts with implementation, source code and tests win. Start with Tauri registration in `app/src/lib.rs`, command implementation in `app/src/commands.rs`, SQL migrations, and the relevant crate tests. The SIH traceability matrix is [docs/04_SIH_REQUIREMENT_MAPPING.md](docs/04_SIH_REQUIREMENT_MAPPING.md).