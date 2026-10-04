# 12. Validation and Testing

## 12.1 Validation scope

Validation claims below describe checked source tests and commands, not certification. On 2026-09-30, `cargo test --workspace --quiet` completed with **287 passed tests across 44 test suites**. The run exercises the Rust workspace; it is not a GUI end-to-end, penetration, usability or production-readiness assessment.

## 12.2 Repository test categories

| Category | Existing evidence | Status |
|---|---|---|
| Unit testing | Core IDs/states, hash, audit canonicalization, validators, policy decisions and crate logic | PASS for included tests in recorded workspace run |
| Integration testing | SQLite migrations/repos, evidence registration, audit, recovery, provenance, reports, sanitizer and CLI tests | PASS for included tests in recorded workspace run |
| Functional testing | CLI end-to-end workflow tests and Rust crate operation tests | PASS for covered cases; GUI flows not included |
| UI testing | TypeScript/Vite build and manual/browser route inspection | PARTIAL; no checked-in Playwright/WebDriver suite |
| Security testing | Negative input/path/policy cases within crate tests | PARTIAL; no independent penetration test or dependency audit |
| Evidence integrity | Known vectors, matching/mismatch/size checks, registration tests | PASS for covered tests |
| Recovery testing | Synthetic JPEG/PNG/PDF, garbage rejection, scanner corpus | PASS for covered tests; corpus is not representative of all real-world media |
| Sanitization testing | Disposable file/directory, protected-path and outcome tests | PASS for covered tests; never a production-media erasure certification |
| Audit testing | Append/sequence/hash-chain intact and tamper cases | PASS for covered tests |
| Report testing | HTML creation, digest recomputation, existing-target/no-overwrite, invalid path tests | PASS for covered crate tests |
| Negative testing | Invalid IDs/inputs, missing files, mismatches, malformed/unsupported candidates | PARTIAL; only enumerated test cases |
| Regression testing | Full workspace suite | PASS for the recorded run |

## 12.3 Representative test cases

| TEST-ID | Title / objective | Preconditions and steps | Expected result | Actual result / status | Evidence |
|---|---|---|---|---|---|
| INT-001 | Verify SHA-256 known vectors | Hash empty input and `abc` | Standard SHA-256 digest and byte count | PASS in recorded workspace suite | `crates/kryvora-integrity/src/streaming.rs`; `tests/nist_vectors.rs` |
| EVD-002 | Detect content/size mismatch | Compare source against registered digest/size | Return mismatch; no false verified state | PASS in recorded suite | `crates/kryvora-integrity/src/streaming.rs`; `crates/kryvora-evidence/tests/registration.rs` |
| REC-003 | Accept supported synthetic formats | Scan and validate synthetic JPEG/PNG/PDF | Valid candidates retain source offsets | PASS in recorded suite | `crates/kryvora-recovery/tests/validators.rs`; `tests/recovery_job.rs` |
| REC-004 | Reject garbage candidate | Feed non-format bytes through validator/job | Candidate is rejected, not promoted | PASS in recorded suite | `crates/kryvora-recovery/tests/recovery_job.rs` |
| AUD-005 | Detect chain tampering | Alter event content/link and verify chain | Broken status identifies first break | PASS in recorded suite | `crates/kryvora-audit/tests/chain.rs` |
| SAN-006 | Protect system paths / report honest outcomes | Use disposable targets and system-path probes | Refusal or explicit partial/not-verified result | PASS in recorded suite; platform-specific coverage is conditional | `crates/kryvora-sanitize/tests/` |
| REP-007 | Hash generated report and prevent replacement | Generate HTML, recompute hash, target existing file | Digest matches bytes; existing file unchanged | PASS in recorded suite | `crates/kryvora-report/tests/report.rs` |
| UI-008 | Build and inspect routes | Run `npm run build`; inspect routes at desktop/mobile widths | Build succeeds; no broken route/overflow in observed checks | PASS for build and manual/browser inspection; not automated regression coverage | `frontend/package.json`; `frontend/src/App.tsx` |
| PERF-009 | Benchmark file throughput | No collector/workload harness currently present | Record reproducible measured metrics | NOT TESTED | No benchmark suite found |
| GUI-010 | End-to-end Tauri IPC workflow | Desktop-driven automated test harness absent | Verify IPC against app database | NOT TESTED | No GUI E2E suite found |

## 12.4 Commands and recorded gates

From repository root:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
```

The Tauri app is a separate Cargo workspace; check it from `app/` with `cargo check`. The frontend build is `npm run build` from `frontend/`. These gates passed during the 2026-09-30 implementation session. Re-run them for a release candidate; prior results do not certify later changes.

## 12.5 Status discipline

`PASS` means an actual repository test/command completed as described. `FAIL` requires an observed failed check. `NOT TESTED` means no result was collected. `UNSUPPORTED` means the feature itself is not available. `INCONCLUSIVE` means the check did not establish the property. Do not mark future test plans as PASS.

## 12.6 Not covered

No GUI IPC automation, hardware write-blocker test, drive erasure test, independent security review, fuzzing campaign, multi-gigabyte/representative-media performance study, Windows installer signing validation, crash-recovery study or cross-platform certification is claimed. One-run synthetic scanner/recovery measurements are documented in [13. Performance Evaluation](13_PERFORMANCE_EVALUATION.md).
