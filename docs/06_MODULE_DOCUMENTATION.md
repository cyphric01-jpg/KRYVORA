# 06. Module Documentation

## 6.1 Secure Drive Eraser

The drive sanitization concept is represented in the storage and policy crates, but the active desktop workflow intentionally keeps drive erasure behind a safety boundary. The application does not expose a raw device wipe command and therefore avoids claiming an operational drive-level erase competency.

## 6.2 Secure File and Folder Eraser

The sanitization workflow is implemented for file and directory targets with path restrictions, type validation, and post-operation hash comparison. The backend returns explicit outcomes and does not treat a destructive overwrite as equivalent to a universal erase guarantee.

Key behavior includes:

- reject unsafe or protected paths
- require a regular file or validated directory target
- refuse symlink/reparse path handling that would bypass the target
- write randomized bytes and compare digest before/after the operation
- classify outcomes as success, partial, failed, or not verified

## 6.3 Advanced File Carving and Recovery

The carving layer supports bounded scanning for contiguous JPEG, PNG, and PDF signatures. Recovery checks candidate validity and records offset, length, validation state, confidence details, and provenance data.

Current capabilities are intentionally scoped and reviewable:

- signatures are scanned in bounded windows
- only certain recognized file types are validated
- contamination or unsupported structures are rejected or left metadata-only
- accepted results are persisted with source linkage and artifact metadata

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
