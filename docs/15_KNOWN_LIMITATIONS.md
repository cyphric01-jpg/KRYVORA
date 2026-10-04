# 15. Known Limitations

## 15.1 Currently implemented

- Case creation/listing and regular-file evidence registration.
- Streaming SHA-256, input-size recording, and re-verification comparison.
- SQLite migrations/repositories for cases, evidence, jobs, audit, provenance, recovery results and reports.
- Tamper-evident local audit append/verification.
- Job lifecycle/progress persistence in the Rust jobs crate.
- Contiguous signature scanning and format validation for JPEG, PNG and PDF.
- Accepted recovery result persistence with offsets, lengths, digest, validation facts, confidence fields and evidence-rooted provenance.
- Tauri HTML recovery report generation, report hash/size metadata, and report audit event.
- File/folder random-overwrite attempts with explicit outcomes and conservative unlink behavior.

“Implemented” describes a code path, not a certification or universal guarantee.

## 15.2 Partially implemented

- **File/folder sanitization:** overwrite and digest checks exist, but safe unlink is disabled, UI typed phrase is not checked by Rust, and `kryvora-policy` is not wired into Tauri sanitizer commands.
- **Recovery:** only a small signature set and contiguous candidates; recovery DTO and report generation omit some stored validation/confidence details.
- **Provenance:** evidence → candidate → artifact works; job, fragment and report nodes/links are not completed in the desktop flow.
- **Jobs:** lifecycle runner persists state and audit; desktop exposes list only, without create/cancel/checkpoint controls.
- **Reporting:** recovery HTML is exposed; sanitizer-report builder exists only at crate level; no report digest re-verification command.
- **Storage/policy:** file/directory/device inspection helpers and pure policy decisions exist, with tests; desktop command integration is incomplete.
- **Validation UI:** queries persisted evidence/recovery states and verifies audit chain, but does not execute tests.

## 15.3 Unsupported or not implemented

| Area | Current limitation |
|---|---|
| Drive sanitization | No Tauri device inspection or drive-sanitize command; no raw-device erase executor |
| Acquisition | No forensic imaging/acquisition or write-blocker integration |
| Partition/filesystem analysis | No partition table or filesystem parser in the product workflow |
| Deleted-file analysis | No filesystem deleted-entry recovery |
| Fragment reconstruction | Recovery marks contiguous reconstruction; no fragmented-file reassembly |
| Timeline | No unified filesystem/artifact/acquisition timeline |
| Evidence types | Tauri evidence registration requires a regular file; no raw-device source registration or VM-specific workflow |
| Artifact formats | JPEG, PNG, PDF only for current scanner/validators |
| Performance | Manual single-run synthetic scanner/recovery benchmarks exist; no repeated trials, representative evidence, CPU sampling, disk-I/O counters, or live application metrics |
| GUI validation | No automated desktop IPC/end-to-end test suite |
| Report verification | No UI/Tauri action to re-hash a stored report and compare it to metadata |
| User accounts/settings | No authentication, role-based access or editable policy/database settings |
| Sanitization certificate | No Tauri certificate/report workflow |

## 15.4 Platform and evidence handling

The Tauri bundle configuration targets Windows MSI and NSIS. Rust crates contain Unix-specific handling, but this package documents and tests the Windows desktop prototype; cross-platform Tauri support is not certified. Evidence registration stores a path and metadata; it does not make a forensic image. Availability and identity remain dependent on the source path and local host.

## 15.5 Integrity, audit and report caveats

- Re-verification appends an event but does not update the evidence row's `integrity_state`.
- Audit integrity is a local hash chain, not an externally signed or independently anchored log.
- Some actions are not audited; sanitization failure/partial outcomes reuse a `sanitization_started` event type with outcome details.
- Recovery results created by Tauri currently have incomplete job linkage.
- Tauri report reconstruction does not restore stored confidence level into the generated report assessment; confidence may therefore be inaccurate in that report path.
- Generated report hashes are stored at creation, but no UI/Tauri re-verification action exists.

## 15.6 Operational warning

This is an SIH prototype. Do not use it as the sole tool for a legal investigation, as a hardware write blocker, or as proof of certified media sanitization. Use authorized evidence, preserve originals, use disposable sanitization targets and independently verify important findings.

See [16. Roadmap](16_ROADMAP.md) for proposed work.
