# Documentation Coverage Matrix

**Interpretation:** the Status column reports documentation-package coverage. It does not promote the underlying feature status; see [04. SIH Requirement Mapping](04_SIH_REQUIREMENT_MAPPING.md) for feature status.

| Document | Purpose | SIH requirement | Implementation evidence referenced | Documentation status |
|---|---|---|---|---|
| [01 Project Overview](01_PROJECT_OVERVIEW.md) | Executive product identity, scope, stack and inventory | Integrated tool; dashboard; supporting package | Tauri command registry, crate map, routes, migrations | IMPLEMENTED |
| [02 Problem Statement](02_PROBLEM_STATEMENT.md) | Problem → impact → gap → required capability → response | Integrated tool | Evidence/integrity/audit/recovery modules and explicit gaps | IMPLEMENTED |
| [03 Proposed Solution](03_PROPOSED_SOLUTION.md) | Integrated concept and workflow diagrams | Integrated tool; dashboard | Tauri orchestration and current command boundary | IMPLEMENTED |
| [04 SIH Requirement Mapping](04_SIH_REQUIREMENT_MAPPING.md) | Ten-row traceability matrix and counted coverage | All named SIH deliverables | Tauri handlers, UI routes, Rust tests, explicit unsupported states | IMPLEMENTED |
| [05 System Architecture](05_SYSTEM_ARCHITECTURE.md) | Components, IPC, database and data flows | Technical documentation | Workspace manifests, app handlers, migrations, repositories | IMPLEMENTED |
| [06 Module Documentation](06_MODULE_DOCUMENTATION.md) | Drive, file/folder, recovery, reporting/audit module behavior | Integrated software tool | Storage, policy, sanitize, carving, recovery, report and audit source/tests | IMPLEMENTED |
| [07 Forensic Workflow](07_FORENSIC_WORKFLOW.md) | Evidence, sanitization and drive workflow states | Integrated software tool; dashboard | Registration/verification/carve/report command flow; unsupported drive path | IMPLEMENTED |
| [08 Security Architecture](08_SECURITY_ARCHITECTURE.md) | Boundaries, controls, gaps and hardening needs | Integrated tool; technical documentation | Tauri allow-list/CSP, path checks, hash, audit, sanitizer command path | IMPLEMENTED |
| [09 Evidence Integrity](09_EVIDENCE_INTEGRITY.md) | SHA-256, size, registration and verification semantics | Recovery; technical documentation | `kryvora-integrity`, `kryvora-evidence`, evidence schema/commands/tests | IMPLEMENTED |
| [10 Audit and Provenance](10_AUDIT_AND_PROVENANCE.md) | Hash chain, event fields, graph and missing links | Reporting/audit; recovery | Audit schema/chain, provenance migration and integration tests | IMPLEMENTED |
| [11 User Manual](11_USER_MANUAL.md) | First-time procedures, results, warnings and limits | User Manual | Frontend routes and actual Tauri commands | IMPLEMENTED |
| [12 Validation and Testing](12_VALIDATION_AND_TESTING.md) | Test inventory, actual run and coverage gaps | Validation; Testing Documentation | 287 passing Rust tests/44 suites on 2026-09-30; test source paths | IMPLEMENTED |
| [13 Performance Evaluation](13_PERFORMANCE_EVALUATION.md) | Environment/metric schema and benchmark protocol | Performance Evaluation Report | Sanitizer per-operation elapsed field; no benchmark collector | PARTIAL; measurements pending |
| [14 Threat Model](14_THREAT_MODEL.md) | Twelve requested threats, controls and residual risk | Technical/security documentation | Sanitizer path checks, integrity, job, audit and report behavior | IMPLEMENTED |
| [15 Known Limitations](15_KNOWN_LIMITATIONS.md) | Implemented, partial, unsupported and missing scope | All SIH deliverables | Cross-check of registered commands, crates, migrations and UI | IMPLEMENTED |
| [16 Roadmap](16_ROADMAP.md) | Eight phases mapped to actual implementation | Technical roadmap | Current module status and gaps | IMPLEMENTED |

## Documentation package status

- Requested numbered documents: **16 / 16 present**.
- Relative links audited: **all targets exist** at the time of this check.
- Operational status: consult [04](04_SIH_REQUIREMENT_MAPPING.md); documentation completeness does not imply product completion.
