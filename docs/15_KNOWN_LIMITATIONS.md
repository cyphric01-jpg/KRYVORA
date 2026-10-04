# 15. Known Limitations

## 15.1 Implementation Status

KRYVORA is a focused, operational forensic workstation with concrete defined boundaries. The project documents functionality and limits intentionally, rather than implicitly claiming capabilities that are beyond the current code path.

## 15.2 Implemented

- case creation and regular-file evidence registration
- streaming SHA-256 hashing and size comparison
- local audit-chain verification
- job-state persistence and workflow tracking
- contiguous JPEG, PNG, and PDF signature scanning
- accepted recovery results with provenance metadata
- HTML recovery reporting with stored metadata
- file/directory overwrite workflow with explicit outcomes

## 15.3 Scope and Boundary Limitations

- drive sanitization remains a controlled inspection and planning boundary; physical erase execution is intentionally disabled pending platform-specific validation
- carving and validated recovery are limited to supported contiguous signatures and selected artifact formats
- file/folder sanitization does not guarantee safe unlink or physical-media erasure on flash or remapped media
- provenance is strong for evidence-to-candidate-to-artifact links, but broader graph completeness remains a future enhancement
- performance measurements are representative baselines, not broad production benchmarks
- the UI and local app path are designed for a desktop workstation, not a full enterprise multi-user forensic suite

## 15.4 Unsupported Areas

| Area | Current state |
|---|---|
| Device-level media erasure | Not exposed as an active command |
| Forensic acquisition | Not implemented as a workflow |
| Partition and filesystem analysis | Outside current scope |
| Deleted-entry recovery | Not implemented in current operational path |
| Fragmented reconstruction | Future extension; the current recovery pipeline intentionally stops at the validated supported-artifact boundary |
| Automated GUI E2E suite | Not included as a checked-in test harness |

## 15.5 Operational Guidance

Use authorized evidence sources, preserve the original artifact, use disposable targets for sanitization tests, and review the audit trail before relying on a result. The platform's value is in disciplined, transparent handling rather than unsupported universal claims.

See [16. Roadmap](16_ROADMAP.md).
