# 02. Problem Statement

## 2.1 Problem

Digital investigations often combine separate activities: managing case context, preserving source identity, checking hashes, recovering candidate content, evaluating the result, recording operator actions and producing a report. If these activities are disconnected, the examiner must manually preserve relationships and explain how a result relates to its source.

Sanitization presents a different risk. A delete operation is not proof that data is unrecoverable. Results depend on the target, storage medium, operating system, filesystem and method. Device-level erasure requires reliable device identification and safety controls; overwriting a regular file cannot establish that remapped or flash-storage cells were erased.

## 2.2 Impact

- An unrecorded or changed source can undermine later conclusions.
- A signature hit can be mistaken for a complete, valid file.
- Missing source offsets or validation facts make recovery difficult to review.
- Operations without a tamper-evident record weaken accountability.
- A report can overstate what the tool actually checked.
- Destructive actions against the wrong path or device can cause irreversible loss.

## 2.3 Existing gap

KRYVORA currently addresses a bounded subset of these needs: local case/evidence records, SHA-256, an audit chain, limited carving and recovery, provenance rows, file/folder overwrite attempts, and HTML recovery reports. It does not acquire forensic images, parse partitions/filesystems, recover deleted filesystem entries, wipe drives, reconstruct fragmented files, or collect benchmark telemetry. The current UI is not a replacement for a complete forensic suite.

## 2.4 Required capability

A defensible workflow should identify the case and source, preserve the source during analysis, record integrity values, distinguish detected candidates from validated artifacts, retain offsets and provenance, make operation outcomes auditable, and communicate limitations in reports. Destructive workflows additionally need target identity checks, policy assessment, explicit confirmation and honest verification semantics.

## 2.5 KRYVORA response

KRYVORA supplies a local Rust/Tauri application with SQLite persistence and a React interface. It hashes a regular file at evidence registration, supports re-verification, scans for the supported contiguous signatures, validates candidates with format-specific validators, persists accepted results and source offsets, and can generate recovery HTML reports with a digest. File and directory sanitization attempt random overwrite and compare hashes, but safe path-based unlink is disabled and the operation does not guarantee flash-media erasure.

## 2.6 Problem-to-response summary

| Problem | Current response | Status |
|---|---|---|
| Case/source linkage | Case IDs, evidence IDs and foreign keys | IMPLEMENTED |
| Evidence identity | Canonical regular-file path, byte size and SHA-256 | IMPLEMENTED |
| Change detection | Re-hash and compare digest and size | IMPLEMENTED |
| Deleted-file analysis | No filesystem metadata parser or deleted-entry engine | UNSUPPORTED |
| Raw drive sanitization | Storage inspection exists in a crate; no Tauri erase command | UNSUPPORTED |
| File recovery | Signature scan plus JPEG/PNG/PDF validators | PARTIAL |
| Traceability | Evidence → candidate → artifact provenance nodes with offsets on result rows | PARTIAL |
| Accountability | Canonical SHA-256-linked audit sequence | IMPLEMENTED, with no external anchor |
| Reporting | Tauri recovery HTML generation, digest and database record | PARTIAL |
| Performance evidence | No application benchmark collection | NOT IMPLEMENTED |

## 2.7 Scope boundary

The source repository is the authority for current behavior. The SIH status matrix in [04](04_SIH_REQUIREMENT_MAPPING.md) and limitations in [15](15_KNOWN_LIMITATIONS.md) should be read before a demonstration. No unsupported competitor comparisons or universal security claims are made.
