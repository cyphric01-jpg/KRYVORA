# 06. Module Documentation

## 6.1 Secure Drive Eraser

**Purpose:** Intended to provide controlled drive sanitization. **Current status: UNSUPPORTED in the desktop product.**

`kryvora-storage` can enumerate platform-reported disks with `sysinfo` and build a `TargetProfile`; `kryvora-policy` models a device identity confirmation. Neither capability is connected to a Tauri command, and no drive overwrite/erase executor exists. No drive target, safety assessment, operation plan, execution, verification, audit certificate or successful outcome can be produced by the UI.

| Requested stage | Current implementation |
|---|---|
| Identify/inspect | Partial Rust disk enumeration; not exposed over Tauri |
| Safety assessment | Policy decision model and tests; not wired into app command |
| Confirm | Policy model has identity phrase concept; no executable device workflow |
| Execute/verify | Not implemented |
| Audit/certificate | No drive-specific operation or certificate path |

No universal erasure guarantee should be claimed. Media remapping, SSD/NVMe wear levelling, controller behavior and device firmware require method-specific validation beyond this prototype. See [15](15_KNOWN_LIMITATIONS.md).

## 6.2 Secure File and Folder Eraser

**Status: PARTIAL.** Tauri exposes `sanitize_file` and `sanitize_folder`. The current UI has file/folder selection, path input, a target review dialog and typed final confirmation; the command receives a boolean confirmation, not the typed phrase. Backend path/type/system-path checks remain active, but the separate `kryvora-policy` evaluator is not called by these commands.

### Single-file path

1. Reject protected system paths.
2. `symlink_metadata` must identify a regular file; read-only files are denied.
3. Open without following symlinks/reparse points and compare the opened handle with the path handle.
4. Hash original bytes, write cryptographically random bytes over the original length using a 1 MiB buffer, sync, and hash again.
5. If the original digest remains, return `failed`; if write/sync/re-hash fails, return `partial` or an error.
6. With the app's `unlink_after=true`, return `partial` after successful overwrite because safe path-based unlink is disabled. The file is retained.
7. Record an audit event containing outcome, bytes, elapsed seconds and reason.

### Directory path

The directory routine collects entries, skips symlinks/reparse points, applies the file operation per regular file, aggregates discovered/processed/removed/failed/byte counts, and tries to remove empty child directories. Because safe unlink is disabled, file retention commonly causes a partial outcome. A directory-level summary is returned to the UI; per-file audit behavior comes from the file routine.

### Explicit limits

- No batch queue or job-based cancellation UI.
- No backend-provided target inspection wizard or filesystem/mount summary in Tauri.
- Permissions, locked files and I/O failures may stop or partially complete work.
- Overwriting a file does not guarantee physical erasure on SSD/NVMe or remapped media.
- The UI's typed phrase is a user-interface guard, not a backend typed-phrase policy check.

Tests: `crates/kryvora-sanitize/tests/file_sanitize.rs`, `directory_sanitize.rs`, `safety.rs`; CLI tests include disposable-target behavior. Use only disposable test files.

## 6.3 Advanced File Carving and Recovery

**Status: PARTIAL.** The scanner is read-only and streaming. It searches registered signatures in bounded windows, tracks headers across windows, requires a footer, and enforces minimum/maximum candidate sizes. The current registry is JPEG, PNG and PDF.

A scan emits **candidates**, not recovered files. Recovery reads the candidate byte range, invokes a format validator, computes a candidate digest, and promotes only `Valid` or `Partial` validation states; `Invalid`, `Inconclusive` and `Unknown` are rejected. Persisted accepted results include evidence ID, source offset/length, detected type/category, validation state/facts, confidence level/reasons, recovery method, reconstruction state and artifact SHA-256.

Current reconstruction state is contiguous signature carving. Fragmented reconstruction, filesystem-aware deleted-file recovery, broad format support and artifact export to a separate recovery directory are not implemented. The Tauri `carve_source` flow creates a new case from the supplied title instead of using the global selected case.

Tests: `crates/kryvora-carving/tests/scanner.rs`, `corpus.rs`; `crates/kryvora-recovery/tests/validators.rs`, `recovery_job.rs`; `crates/kryvora-provenance/tests/chain.rs`.

## 6.4 Reporting and Audit Management

**Status: PARTIAL overall.**

- Audit events are appended to SQLite with event type, optional actor/object/job IDs, JSON details, sequence, timestamp, previous hash and current hash.
- Verification checks the complete ordered chain and reports the first break.
- Rust report crate supports recovery and sanitization HTML builders. The Tauri command currently exposes **recovery** report generation only.
- Tauri confines report names to the application-owned directory, rejects unsafe report-directory symlinks and creates output with create-new semantics.
- Generated report bytes are SHA-256 hashed; size/path/digest are stored and a `ReportGenerated` audit event is appended.
- No UI/Tauri command verifies an existing report digest, generates a sanitization certificate, or exports through a separate secure action.
- Provenance nodes currently link evidence → candidate → artifact. Report-kind nodes and job links are not wired into the generated recovery report path.

The recovery report command reconstructs result objects from persisted rows; the current command does not restore the stored confidence level into the reconstructed confidence assessment. Treat report confidence fidelity as a known limitation pending a fix and a focused test.

Tests: `crates/kryvora-audit/tests/chain.rs`, `crates/kryvora-report/tests/report.rs`; report path behavior is implemented in `app/src/commands.rs`.

## 6.5 Related modules

| Crate | Current role |
|---|---|
| `kryvora-core` | Typed IDs, errors, state and domain enums |
| `kryvora-db` | SQLite open/migrations/repositories |
| `kryvora-evidence` | Evidence registration validation and digest comparison |
| `kryvora-integrity` | Streaming SHA-256 and digest/size comparison |
| `kryvora-jobs` | Persisted job lifecycle, progress, cancellation abstraction and audit events |
| `kryvora-storage` | File/directory/device profiles and inspection helpers |
| `kryvora-policy` | Pure target/operation assessments; not integrated into sanitizer Tauri handlers |

See [05. System Architecture](05_SYSTEM_ARCHITECTURE.md) and [15. Known Limitations](15_KNOWN_LIMITATIONS.md).
