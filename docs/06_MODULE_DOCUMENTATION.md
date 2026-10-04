# 06. Module Documentation

## 6.1 Drive Sanitization — Controlled / Inspection & Planning Boundary

KRYVORA provides device inspection, target identity assessment, safety policy, and sanitization planning. Physical drive erase execution remains intentionally disabled pending platform-specific validation.

## 6.2 File/Folder Sanitization — Implemented & Safety-Controlled

The sanitization workflow is implemented with target validation, symlink/reparse protection, controlled processing, verification, and explicit outcome states. It does not treat a destructive overwrite as equivalent to a universal erase guarantee.

Key behavior includes:

- reject unsafe or protected paths
- require a regular file or validated directory target
- refuse symlink/reparse path handling that would bypass the target
- write randomized bytes and compare digest before/after the operation
- classify outcomes as success, partial, failed, or not verified

## 6.3 File Carving and Recovery — Implemented — Supported Artifact Scope

The carving layer provides operational bounded scanning and validated recovery for supported contiguous JPEG, PNG, and PDF formats, with integrity checks, validation, provenance, and controlled output. Recovery records offset, length, validation state, and confidence details.

Current capabilities are intentionally scoped and reviewable:

- signatures are scanned in bounded windows
- only certain recognized file types are validated
- unsupported or inconclusive candidates are rejected or left metadata-only, not represented as successful recovery
- accepted results are persisted with source linkage and artifact metadata

Fragmented-file reconstruction and advanced cross-fragment recovery are future extensions. The current recovery pipeline intentionally stops at the validated supported-artifact boundary.

## 6.4 Reporting and Audit Management

KRYVORA stores audit events and report metadata in SQLite, verifies the local audit chain, and generates HTML recovery reports. These components serve as the system's evidence-trace layer, helping maintain a consistent story from source to recovered output.

## 6.5 Related Modules

| Crate | Role |
|---|---|
| `kryvora-core` | Shared domain models and errors |
| `kryvora-db` | SQLite migrations and repositories |
| `kryvora-evidence` | Evidence registration and integrity checks |
| `kryvora-integrity` | SHA-256 streaming and comparison |
| `kryvora-jobs` | Operation lifecycle and persisted workflow state |
| `kryvora-storage` | Device and target inspection helpers |
| `kryvora-policy` | Target and safety decision models |
| `kryvora-sanitize` | File/folder overwrite workflow |
| `kryvora-carving` | Candidate detection and scanning |
| `kryvora-recovery` | Validation and result classification |
| `kryvora-report` | Recovery report generation |
| `kryvora-audit` | Chain verification and event logging |
| `kryvora-provenance` | Provenance graph and links |

See [05. System Architecture](05_SYSTEM_ARCHITECTURE.md) and [15. Known Limitations](15_KNOWN_LIMITATIONS.md).
