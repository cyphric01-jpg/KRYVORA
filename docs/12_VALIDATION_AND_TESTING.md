# 12. Validation and Testing

## 12.1 Validation Scope

The project includes a Rust workspace test suite covering integrity verification, audit-chain behavior, sanitization safety, report generation, and recovery validation. The recorded workspace run completed 287 passing tests across 44 suites.

## 12.2 Test Categories

| Category | Status |
|---|---|
| Unit and integration tests | Pass for covered repository logic |
| Evidence integrity tests | Pass |
| Recovery and carving tests | Pass for supported synthetic fixtures |
| Sanitization safety tests | Pass for controlled path and outcome checks |
| Audit-chain tests | Pass |
| Report generation tests | Pass |
| UI build verification | Pass for frontend build checks |
| GUI automation | Not implemented as a checked-in E2E suite |

## 12.3 Representative Test Coverage

The repository includes tests for:

- SHA-256 vectors and mismatch detection
- evidence registration and verification behavior
- JPEG, PNG, and PDF candidate evaluation
- audit chain integrity and tampering detection
- report generation and digest handling
- protected path handling and sanitization outcome reporting

## 12.4 Commands

Repository-root validation commands:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
```

Frontend and desktop checks:

```powershell
cd frontend
npm run build
cd ..pp
cargo check
```

## 12.5 Status Discipline

The project distinguishes between:

- pass: a repository test or command completed successfully
- fail: an observed failure is present
- not tested: no repository evidence was collected
- controlled boundary: the feature is restricted and not exposed as an active capability

## 12.6 Coverage Boundaries

The current test suite is substantial but not exhaustive. It does not claim end-to-end desktop automation coverage, hardware write-blocker certification, or production-media sanitization validation. Those remain future hardening targets.
