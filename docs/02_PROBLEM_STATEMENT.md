# 02. Problem Statement

## 2.1 Problem

Digital investigations frequently require a connected view of several critical activities: managing case context, preserving source identity, verifying hashes, evaluating recovered content, documenting operator actions, and producing defensible results. When these activities are disconnected, the chain from source evidence to reported finding becomes hard to defend.

Sanitization introduces a separate but related risk. Deleting a file or overwriting data is not proof that the information is unrecoverable. The outcome depends on the target, storage medium, filesystem behavior, and the exact operation method.

## 2.2 Impact

- an unchecked source can undermine a later conclusion
- a raw signature hit can be mistaken for a complete recovered file
- missing offsets, validation details, and provenance reduce reviewability
- evidence without a tamper-evident record weakens accountability
- destructive actions against an incorrect target cause irreversible loss
- an over-stated report creates uncertainty rather than confidence

## 2.3 Existing Gap

KRYVORA addresses a bounded but operational subset of this problem space: local case records, source integrity, audit-centric event logging, limited signature carving, provenance-aware result persistence, and reporting. The platform does not attempt to replace full-scale forensic suites or device-level media erasure systems.

## 2.4 Required Capability

A defensible workflow must preserve source information, validate integrity before and after analysis, distinguish a candidate from a validated artifact, retain provenance and offsets, document result status, and keep operation outcomes honest. Destructive workflows require target review, concrete safety checks, and explicit outcome classification rather than implicit assumptions.

## 2.5 KRYVORA Response

KRYVORA provides a local Rust and Tauri application with SQLite persistence and a React desktop interface. It stores source metadata, computes SHA-256 digests, supports evidence re-verification, scans recognized signatures, validates accepted candidates, and preserves the provenance chain from source evidence to recovered artifact. The sanitization workflow similarly records the actual outcome, with no claim of flash-media erasure certainty.

## 2.6 Problem-to-Response Summary

| Problem | Current response | Status |
|---|---|---|
| Case and source linkage | Case IDs, evidence IDs, and related references | Implemented |
| Evidence integrity | Canonical path, file size, and SHA-256 recorded at registration | Implemented |
| Change detection | Re-hash and size comparison before/after analysis | Implemented |
| Deleted-file analysis | Filesystem-aware deleted-entry parsing is outside the current scope | Controlled boundary |
| Raw drive sanitization | Storage study and policy models exist, but no active drive erase workflow is exposed | Controlled boundary |
| File recovery | Bounded signature scanning and validated recovery for supported contiguous JPEG, PNG, and PDF formats | Implemented — Supported Artifact Scope |
| Traceability | Evidence-to-candidate-to-artifact provenance | Implemented in scope |
| Accountability | Hash-linked audit chain and event verification | Implemented |
| Reporting | HTML recovery report generation with metadata and digest capture | Implemented in scope |
| Performance evidence | Synthetic evaluation baseline and methodology exist | Partial |

## 2.7 Scope Boundary

This repository is the source of truth for current behavior. The feature status matrix in [04. SIH Requirement Mapping](04_SIH_REQUIREMENT_MAPPING.md) and the limitation inventory in [15. Known Limitations](15_KNOWN_LIMITATIONS.md) define the operational boundaries of the present implementation.
