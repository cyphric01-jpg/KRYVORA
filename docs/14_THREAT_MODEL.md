# 14. Threat Model

## 14.1 Scope and assumptions

This model covers the local KRYVORA desktop prototype and its source/database/report paths. It is a qualitative engineering inventory, not a formal risk score, penetration-test report or guarantee. Threat status uses IMPLEMENTED, PARTIAL, UNSUPPORTED and NOT TESTED.

| Threat | Scenario | Impact | Current mitigation | Detection | Residual risk | Status |
|---|---|---|---|---|---|---|
| Wrong target selection | Operator enters a path other than the intended target | Irreversible overwrite or data loss | UI review and typed `SANITIZE`; backend file type/system-path/open-handle checks | Returned target path/error and audit details | Typed phrase is not validated by Rust; no full preflight identity summary or undo | PARTIAL |
| Accidental destructive operation | User confirms by mistake | Loss/overwrite of target contents | Confirmation boolean required; UI final dialog; backend checks | Sanitizer outcome/audit event | No authenticated operator or wired policy confirmation evaluator | PARTIAL |
| Evidence modification | External process changes source during analysis | Invalid forensic conclusions | Read-only workflow; source digest/size before/after carve; app does not write carved bytes into source | Re-hash mismatch aborts carving | No OS write blocker or immutable image; external mutation before/after scan remains possible | PARTIAL |
| Hash mismatch | Source changed or registration metadata is wrong | Integrity uncertainty | Streaming SHA-256 and size comparison | `verified`/`mismatch`/`failed` response plus audit event | Reverify result does not update evidence row; no externally protected hash manifest | PARTIAL |
| Permission failure | Source/target/database access denied | Operation fails or is incomplete | Rust I/O error mapping; read-only targets refused by sanitizer | Error/result reason; some sanitizer I/O errors return partial | Platform-specific permissions/ACLs and locked-target cases need broader testing | PARTIAL |
| Device disconnect | Removable media disappears during an operation | Incomplete device operation or data loss | No drive-sanitize operation is exposed | Device workflows cannot start in current UI | No device monitoring/transactional resume; do not perform drive operation | UNSUPPORTED |
| Job interruption | Process exits or cancellation occurs mid-job | Incomplete scan/state | Job crate persists states and supports cancellation token abstraction | Persisted job status and audit lifecycle events | UI has no cancel/checkpoint recovery; crash-injection testing absent | PARTIAL |
| Malformed evidence | Crafted bytes exploit parser assumptions | Crash, resource exhaustion, incorrect result | Bounded signature windows/sizes; small format validators; Rust error handling | Errors/rejected candidates; tests cover synthetic garbage | No documented fuzzing, sandbox or comprehensive resource quota | PARTIAL |
| Parser/validator failure | Format edge case is misclassified | False positive/negative recovery | JPEG/PNG/PDF validators; only valid/partial candidates promoted | Validation facts/state and candidate counts | Narrow format coverage; validators are not independently certified | PARTIAL |
| Database corruption | Disk failure or manual modification affects records | Lost/altered evidence/audit metadata | SQLite constraints, migration history checks and error propagation | DB errors; audit verification catches some event-chain changes | No automatic backup/restore or corruption-recovery workflow | PARTIAL |
| Audit tampering | Local actor edits/deletes/reorders event rows | Reduced accountability | Canonical previous/current SHA-256 chain and full verification | `verify_chain` reports first break | No signature/key, remote append-only store or trusted external chain-head anchor; privileged rewrite may evade | PARTIAL |
| Report integrity failure | Generated HTML is changed after creation | Report no longer matches recorded bytes | SHA-256 stored at generation; create-new and app-owned path | No in-app report verification command | Digest is not rechecked/export-signed by Tauri; report confidence reconstruction gap | PARTIAL |

## 14.2 Trust boundaries

The frontend is not the security boundary. Tauri commands validate and marshal inputs; backend crates access local files and SQLite. Tauri CSP limits web resources, but security still depends on command implementation, platform behavior and the host account. See [08. Security Architecture](08_SECURITY_ARCHITECTURE.md).

## 14.3 Out of scope / required analysis

Not covered: malicious administrator with full host/database access, supply-chain compromise, malware persistence, physical acquisition/write-blocker attacks, multi-user authorization, network sync, cryptographic signing, and independent verification of sanitization on SSD/NVMe. These require production design and external assessment.
