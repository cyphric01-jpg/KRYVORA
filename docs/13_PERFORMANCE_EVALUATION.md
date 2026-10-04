# 13. Performance Evaluation

## 13.1 Current Result

KRYVORA includes a focused performance evaluation package for synthetic scanner and recovery workloads. The measurements are presented as benchmark baselines and are not described as representative of all environments or all evidence types.

## 13.2 Environment Record

| Field | Example |
|---|---|
| CPU | AMD Ryzen 7 7445HS |
| Memory | 15.27 GiB |
| OS | Windows 11 Home |
| Storage | NVMe SSD |
| Toolchain | Rust 1.90.0 |

## 13.3 Metrics

The evaluation tracks the key practical metrics relevant to the current scanning and recovery pipeline:

- input size
- elapsed time
- candidate counts
- validation outcomes
- throughput estimate for synthetic workloads
- peak working set where sampled

## 13.4 Workloads

The repository includes evaluation scenarios for:

- no-signature scan workload
- JPEG-dense synthetic input
- valid JPEG recovery and validation
- resource-bound scan conditions

These workloads help demonstrate the expected performance profile of the implemented engine but remain intentionally bounded.

## 13.5 Reproducible Baseline

Example commands:

```powershell
cargo test -p kryvora-carving --release --test scanner benchmark_streaming_and_candidate_heavy_fixtures -- --ignored --nocapture
cargo test -p kryvora-recovery --release --test recovery_job benchmark_valid_jpeg_recovery_sizes -- --ignored --nocapture
```

These are synthetic benchmark runs for the local environment and should be interpreted as baseline evidence rather than a universal performance claim.

## 13.6 Reporting Template

A complete future benchmark protocol should include:

- environment details
- fixture identity
- input size
- duration distribution
- throughput calculation
- generated artifacts and rejected candidates
- resource and I/O context when available
- limitations and interpretation notes

See also [12. Validation and Testing](12_VALIDATION_AND_TESTING.md).
