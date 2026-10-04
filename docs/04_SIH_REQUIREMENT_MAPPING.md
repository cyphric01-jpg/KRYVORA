# 04. SIH Requirement Mapping

## 4.1 Traceability Matrix

The matrix below reflects repository behavior and implementation status, not a claim of full product certification. The status labels describe the actual capability present in the project.

| SIH requirement | KRYVORA module | Implemented capability | User-facing location | Backend component | Validation method | Status |
|---|---|---|---|---|---|---|
| Secure drive eraser | Drive Eraser | Device inspection and policy models exist; no active drive-sanitize execution is exposed | Drive Eraser route | `kryvora-storage`, `kryvora-policy` | Storage inspection and policy tests | Controlled boundary |
| Secure file and folder eraser | File / Folder Eraser | Random overwrite, digest comparison, path protections and outcome reporting | Sanitize route | `kryvora-sanitize` | Crate integration tests and CLI tests | Partial |
| Advanced file carving and recovery | Analyze / Recover | Streaming scan and validation for JPEG, PNG and PDF candidates | Recover and Investigate routes | `kryvora-carving`, `kryvora-recovery`, `kryvora-provenance` | Scanner and recovery tests | Partial |
| Reporting and audit management | Reports, Audit | Audit verification and recovery report generation with stored metadata | Audit and Reports routes | `kryvora-audit`, `kryvora-report`, `kryvora-db` | Audit-chain and report tests | Implemented in scope |
| Dashboard | Command Center | Case, evidence, job, result and report summaries | Dashboard route | Tauri list queries | Frontend build and route inspection | Implemented |
| Validation | Validation Center | Persistence states and audit chain verification | Validation route | Tauri verification calls | Rust verification tests | Implemented in scope |
| Testing documentation | QA records | Repository test results and validation coverage | Documentation and validation pages | Workspace test suites | `cargo test --workspace --quiet` | Implemented |
| User manual | Documentation package | Workflow procedures and operational guidance | Documentation indexes and manual | Markdown and source review | Documentation review | Implemented |
| Technical documentation | Documentation package | Architecture, module, workflow, security, and risk coverage | Documentation pages | Source review and repo checks | Cross-check against code/tests | Implemented |
| Performance evaluation | Performance package | Synthetic benchmark protocol and baseline measurements | Documentation and evaluation route | Benchmark fixtures and tests | release-mode benchmark runs | Partial |

## 4.2 SIH Coverage Summary

| Status | Count |
|---|---:|
| Implemented | 4 |
| Partial | 4 |
| Implemented in scope | 2 |
| Controlled boundary | 1 |
| Total mapped deliverables | 11 |

The summary reflects implementation coverage and operational boundaries, not a weighted claim of full forensic product completion.

## 4.3 Non-Claims

- no universal drive-erasure guarantee is made
- no full forensic imaging workflow is claimed
- no partition or filesystem reconstruction is presented as complete
- no production-media sanitization certification is claimed
- no live benchmark study is described as representative of all hardware or deployments

## 4.4 Related Documents

- [06. Module Documentation](06_MODULE_DOCUMENTATION.md)
- [12. Validation and Testing](12_VALIDATION_AND_TESTING.md)
- [15. Known Limitations](15_KNOWN_LIMITATIONS.md)
