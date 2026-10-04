# 16. Roadmap

## 16.1 Planning principle

Roadmap entries are proposed work, not current capability. Prioritize evidence safety, reproducible verification and tested integration before increasing operation scope.

## 16.2 Phased roadmap

| Phase | Theme | Current position | Proposed next work |
|---|---|---|---|
| 1 | Core foundation | IMPLEMENTED | Stabilize command/error contracts, migrations and release build pipeline; add application-level regression gates. |
| 2 | Evidence and integrity | PARTIAL | Add image/container metadata, optional immutable acquisition copy, verification history model, exportable hash manifest and write-blocker integration design. |
| 3 | Sanitization | PARTIAL for file/folder; drive UNSUPPORTED | Wire Rust policy assessments into Tauri; bind confirmations to canonical target identity; keep device operation disabled until independently reviewed method/device support exists; add safe handle-bound deletion only if platform primitive is defensible. |
| 4 | Recovery/carving | PARTIAL | Expand supported formats with validators and fixtures; add candidate/reconstruction states; address fragmentation only with format-aware reconstruction and validation. |
| 5 | Validation and provenance | PARTIAL | Persist complete job links; expose confidence reasons/validation facts; connect report nodes; add testable validation actions and negative/fuzz/resource-bound tests. |
| 6 | Reporting and audit | PARTIAL | Correct report confidence reconstruction; add report digest verification, sanitization report workflow, more precise terminal audit event types, actor identity and external audit-head anchoring. |
| 7 | Performance and hardening | PARTIAL | Synthetic release-mode scanner/recovery baseline and resource/cancellation regressions exist; repeated representative workloads, CPU/I/O sampling and broader reliability study remain. |
| 8 | Production readiness | PLANNED | Independent forensic/security review, platform-specific validation, signed/reproducible releases, backup/restore, access control, operator procedures and acceptance evidence. |

## 16.3 Release gates

A phase should not be declared complete solely because its UI exists. Require source review, tests for success and failure paths, reproducible validation output, accurate audit/provenance, platform-specific behavior evidence, and documentation updates. Destructive features additionally require independently reviewed target binding, policy enforcement and verification semantics.

## 16.4 Near-term order

1. Fix the recovery-report confidence reconstruction and add a regression test.
2. Wire `kryvora-policy` into file/folder command orchestration; bind typed confirmations to backend policy state.
3. Complete job/evidence/report relationships and expose stored validation/confidence explanations.
4. Add report digest verification and sanitizer reporting only after data semantics are defined.
5. Keep drive erase, acquisition, filesystem parsing and performance claims explicitly disabled/pending until implementation and tests exist.

## 16.5 Related documents

Implementation scope: [04. SIH Requirement Mapping](04_SIH_REQUIREMENT_MAPPING.md). Risk inventory: [14. Threat Model](14_THREAT_MODEL.md). Remaining gaps: [15. Known Limitations](15_KNOWN_LIMITATIONS.md).
