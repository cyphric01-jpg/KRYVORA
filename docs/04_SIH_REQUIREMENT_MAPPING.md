# 04. SIH Requirement Mapping

## 4.1 Traceability matrix

Statuses apply to the deliverable as requested, not merely to the existence of a related crate. Code and tests are listed as evidence; UI-only presentation is not counted as backend implementation.

| SIH requirement | KRYVORA module | Implemented capability | UI location | Backend component | Validation method | Status | Evidence / proof |
|---|---|---|---|---|---|---|---|
| Secure Drive Eraser | Drive Eraser | Rust can enumerate/inspect platform disks; no device-sanitize operation is registered or implemented | Drive Eraser page explains unsupported operation | `kryvora-storage`; no Tauri drive command | Storage enumeration/unknown-device tests; no erase test exists | UNSUPPORTED | `crates/kryvora-storage/src/device.rs`; `app/src/lib.rs`; `frontend/src/pages/Readiness.tsx` |
| Secure File and Folder Eraser | File / Folder Eraser | Random overwrite, post-write digest comparison, system-path checks, outcomes and audit details; unlink disabled | Sanitize route with target review and typed final confirmation | `kryvora-sanitize`; `sanitize_file` and `sanitize_folder` Tauri commands | `file_sanitize.rs`, `directory_sanitize.rs`, `safety.rs`; CLI integration tests | PARTIAL | `crates/kryvora-sanitize/src/file.rs`; `crates/kryvora-sanitize/src/directory.rs`; `app/src/commands.rs` |
| Advanced File Carving and Recovery | Analyze / Recover, Investigate | Streaming header/footer scan; JPEG, PNG and PDF validators; contiguous candidates, confidence, result hash/offset and persistence | Recover and Investigate routes | `kryvora-carving`, `kryvora-recovery`, `kryvora-provenance` | Scanner corpus, validators, recovery job and provenance integration tests | PARTIAL | `crates/kryvora-carving/src/signatures/`; `crates/kryvora-recovery/tests/`; `crates/kryvora-provenance/tests/chain.rs` |
| Reporting and Audit Management | Reports, Audit | Audit append/verify; Tauri recovery HTML generation, digest and report-row persistence. No Tauri sanitization-report workflow or report re-verification command | Audit and Reports routes | `kryvora-audit`, `kryvora-report`, `kryvora-db` | Audit-chain and report integration tests | PARTIAL | `crates/kryvora-audit/tests/chain.rs`; `crates/kryvora-report/tests/report.rs`; `app/src/commands.rs` |
| Dashboard | Command Center | Queries case/evidence/job/recovery/report counts, recent audit events and chain status | Dashboard route | Existing list/verify Tauri commands | Frontend build and route render; no automated GUI E2E suite | IMPLEMENTED | `frontend/src/pages/Dashboard.tsx`; `app/src/lib.rs` |
| Validation | Validation Center | Displays evidence states, persisted recovery validation states and actual audit-chain check; does not execute a test suite | Validation route | `list_cases`, `list_evidence_for_case`, `list_recovery_results`, `verify_chain` | Rust verification tests; UI route check | UI WORKFLOW ONLY | `frontend/src/pages/Readiness.tsx`; `app/src/lib.rs` |
| Testing Documentation | QA package | Documents available Rust tests and actual aggregate run; no claim of complete GUI/security certification | Documentation and Validation routes are indexes/status views | Rust workspace test suites | `cargo test --workspace --quiet`; 287 passed across 44 suites on 2026-09-30 | IMPLEMENTED | `crates/*/tests/`; [12. Validation and Testing](12_VALIDATION_AND_TESTING.md) |
| User Manual | Documentation package | Procedures describe current UI/backend behavior and limitations | Documentation route is an index, not the manual itself | No user-manual runtime component | Markdown/source review | IMPLEMENTED | [11. User Manual](11_USER_MANUAL.md) |
| Technical Documentation | Documentation package | Architecture, module, workflow, security, evidence, audit, threat and limitation docs | Documentation route lists repository references and outlines | Rust/Tauri/SQLite implementation | Cross-check against registered commands, migrations and tests | IMPLEMENTED | [05](05_SYSTEM_ARCHITECTURE.md), [06](06_MODULE_DOCUMENTATION.md), [08](08_SECURITY_ARCHITECTURE.md), [10](10_AUDIT_AND_PROVENANCE.md) |
| Performance Evaluation Report | Performance package | Reproducible protocol plus one-run synthetic scanner/recovery baseline; no representative or repeated-trial study | Performance route has no live collector | Ignored release-mode fixture harnesses | Controlled repeated trials and representative media remain outstanding | PARTIAL | `crates/kryvora-carving/tests/scanner.rs`; `crates/kryvora-recovery/tests/recovery_job.rs`; [13. Performance Evaluation](13_PERFORMANCE_EVALUATION.md) |

## 4.2 SIH coverage summary

Counting the ten deliverable rows above:

| Status | Count |
|---|---:|
| IMPLEMENTED | 4 |
| PARTIAL | 4 |
| UI WORKFLOW ONLY | 1 |
| UNSUPPORTED | 1 |
| NOT IMPLEMENTED | 0 |
| PLANNED | 0 |
| **Total mapped deliverables** | **10** |

These are scope-level counts, not a weighted completion percentage. A deliverable marked PARTIAL can contain implemented subcomponents; it is not equivalent to a complete SIH module.

## 4.3 Non-claims

- No drive erase, disk acquisition, partition parsing, filesystem analysis or deleted-file analysis is claimed.
- The recovery scanner detects bounded header/footer ranges. A candidate is not automatically a recovered file.
- Audit hashes are not a digital signature or an external timestamp/anchor.
- Performance documentation contains single-run synthetic measurements; these are not a controlled or representative performance study.
- Rust unit/integration coverage does not establish GUI end-to-end coverage or production readiness.

## 4.4 Related documents

See [06. Module Documentation](06_MODULE_DOCUMENTATION.md), [12. Validation and Testing](12_VALIDATION_AND_TESTING.md), and [15. Known Limitations](15_KNOWN_LIMITATIONS.md).
