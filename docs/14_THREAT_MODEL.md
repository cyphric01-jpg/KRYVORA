# 14. Threat Model

## 14.1 Scope

The threat model addresses the local KRYVORA desktop workflow and its interaction with local files, the SQLite database, and generated reports. It is a practical security inventory designed to document the system's important risks and safeguards.

| Threat | Scenario | Current mitigation | Residual risk |
|---|---|---|---|
| Wrong target selection | User targets an unintended file or directory | Path validation and review steps | Human judgment remains essential |
| Destructive action mistake | User confirms an overwrite that should not proceed | Explicit confirmation workflow and audit record | Authorization model is still local and lightweight |
| Evidence modification | Source changes during analysis | Re-hash check before processing | OS-level write protection is not enforced as a full forensic guarantee |
| Hash mismatch | Source differs from recorded evidence | Re-verification and integrity comparison | Database and source remain local artifacts |
| Permission denial | Access to files or reports fails | Rust error handling and result classification | Platform-specific access conditions vary |
| Device erase confusion | User expects drive erasure from a device path | Device operation remains intentionally disabled | Requires explicit future product hardening |
| Audit tampering | Local database is altered by a privileged actor | Hash-linked audit sequence | Local chain is not an external trust anchor |
| Report alteration | Generated HTML is changed after creation | Stored digest and metadata | Re-verification command remains a future enhancement |

## 14.2 Trust Boundaries

The core trust boundary is the backend layer. Frontend input is not trusted implicitly; Tauri commands validate inputs and the Rust application performs the actual enforcement. That boundary keeps the main security logic in the Rust domain model and data layer.

## 14.3 Out-of-Scope / Future Work

The project does not currently claim enterprise-grade access control, full supply-chain attestation, formal red-team validation, or independent forensic certification. Those requirements remain part of the next hardening stage.

See [08. Security Architecture](08_SECURITY_ARCHITECTURE.md) and [16. Roadmap](16_ROADMAP.md).
