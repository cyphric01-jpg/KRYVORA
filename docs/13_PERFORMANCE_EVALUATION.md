# 13. Performance Evaluation

## 13.1 Current result

**MANUAL SYNTHETIC BASELINE RECORDED; COMPREHENSIVE PERFORMANCE EVALUATION REMAINS INCOMPLETE.** The release-mode scanner and recovery harnesses below report actual single-run measurements. They are not representative-evidence benchmarks, repeated trials, or a production performance guarantee. The UI still has no live metric collector.

## 13.2 Environment record

Record these fields for every future run; current project-level values are not captured as a benchmark environment.

| Field | Value |
|---|---|
| CPU model / core count | AMD Ryzen 7 7445HS; 6 cores / 12 logical processors |
| RAM | 15.27 GiB reported by Windows |
| GPU | Not measured; not used by these CPU scanner/recovery paths |
| OS version / build | Windows 11 Home Single Language, 10.0.26200 (build 26200) |
| Source and destination storage | WD PC SN5000S 512 GB NVMe SSD; fixtures stored under the workspace on C: |
| KRYVORA version / commit | 0.1.0; commit identity not recorded |
| Rust toolchain | rustc 1.90.0 (1159e78c4, 2025-09-14), x86_64-pc-windows-msvc |
| Power mode / background load | Not controlled or recorded |

## 13.3 Metrics

| Metric | Definition | Availability |
|---|---|---|
| Input size | Bytes consumed or targeted | Available per evidence/hash and sanitizer result |
| Execution time | Wall time per operation, with warm/cold run noted | Sanitizer result has elapsed seconds; general benchmark collection absent |
| Throughput | Input bytes divided by elapsed seconds | Not calculated by app; PENDING MEASUREMENT |
| CPU / memory | Process-level sampled resources | CPU not sampled; peak working set sampled once per benchmark process below |
| Disk I/O | Read/write counters and bytes | Not collected; PENDING MEASUREMENT |
| Artifacts / errors | Backend-returned candidate/result/error counts | Available for a carve operation; no benchmark aggregation |
| Job completion | persisted final state and timestamps | Available for jobs that are created; UI lists persisted rows |

## 13.4 Workloads

For each operation and media type, define fixed small, medium and large source files using recorded byte sizes and cryptographic test-fixture hashes. Include an all-zero/no-signature corpus, a supported synthetic corpus, and representative authorized evidence with known ground truth. Keep source read-only and run from a clean database per trial. Do not use confidential evidence in shared benchmark artifacts.

Suggested measurements: evidence hash/verification; JPEG/PNG/PDF carving; recovery validation; report generation; and sanitizer overwrite on disposable targets only. **Do not benchmark drive erase:** no such operation exists.

## 13.5 Reproduction protocol

1. Record hardware/software/environment fields and KRYVORA build identity.
2. Verify fixture hash/size before each run; preserve a read-only master.
3. Define cache policy, concurrency, run count and warm-up before collecting data.
4. Use a fresh test database or record its initial state; separate setup from timed operation.
5. Capture wall time, OS process CPU/memory, bytes read/written, status, counts and errors using an external measurement harness.
6. Repeat each workload and publish raw observations, summary method and limitations; do not report a single best run as representative.
7. Re-run tests and integrity checks after any destructive test fixture operation.

## 13.6 Reproducible baseline (2026-10-02)

Command forms, run from the repository root:

```powershell
cargo test -p kryvora-carving --release --test scanner benchmark_streaming_and_candidate_heavy_fixtures -- --ignored --nocapture
cargo test -p kryvora-recovery --release --test recovery_job benchmark_valid_jpeg_recovery_sizes -- --ignored --nocapture
```

Each row is one release-mode run. Throughput is measured bytes divided by elapsed seconds, expressed as MiB/s. Scanner throughput uses `bytes_scanned`; recovery throughput uses the candidate length. Large zero fixtures had no signatures. The JPEG-dense fixture contains repeated synthetic JPEG header/footer ranges; scanning stopped at the candidate cap. Recovery fixtures are synthetic, structurally valid JPEG candidates. Fixture creation is outside the timed scan/recovery interval.

| Operation / fixture | Fixture size | Bytes processed | Elapsed | Throughput | Candidates considered / validated | Limits / outcome |
|---|---:|---:|---:|---:|---:|---|
| Carving, zero/no-signature | 16 MiB | 16 MiB | 0.025059 s | 638.50 MiB/s | 0 / 0 | No limit reached |
| Carving, zero/no-signature | 64 MiB | 64 MiB | 0.057571 s | 1,111.68 MiB/s | 0 / 0 | No limit reached |
| Carving, zero/no-signature | 256 MiB | 256 MiB | 0.184431 s | 1,388.05 MiB/s | 0 / 0 | No limit reached |
| Carving, JPEG-dense | 16 MiB | 3 MiB | 0.016667 s | 179.99 MiB/s | 10,000 / not run through recovery | Candidate limit reached; 137,511 headers dropped |
| Recovery, valid synthetic JPEG | 1 MiB | 1 MiB | 0.004863 s | 205.65 MiB/s | 1 / 1 | Validation `valid` |
| Recovery, valid synthetic JPEG | 16 MiB | 16 MiB | 0.035432 s | 451.57 MiB/s | 1 / 1 | Validation `valid` |
| Recovery, valid synthetic JPEG | 64 MiB | 64 MiB | 0.143182 s | 446.98 MiB/s | 1 / 1 | Validation `valid`; recovery size ceiling |

Windows process live sampling reported peak working sets of 8.37 MiB for the scanner harness and 135.54 MiB for the recovery harness. Sampling is approximate and includes the test harness/runtime; it is not an isolated allocation profile. Disk-I/O counters, CPU utilization, cache state, repeated-run distributions, and multi-gigabyte runs were not measured. Single-run variation is visible; these numbers must not be generalized to other systems.

Default resource ceilings exercised by the scanner are 1 MiB read windows, 4 GiB maximum scan bytes, 10,000 candidates, 4,096 open headers, and a 16-byte preview. The recovery engine accepts candidate ranges up to 64 MiB. Candidate-limit results are persisted as partial jobs, not successful full scans.

## 13.7 Reporting template

Every future row should contain: operation, source-fixture identifier (not private evidence), input size, run count, duration distribution, throughput calculation, CPU, peak memory, disk I/O, returned artifacts, errors, job result, environment and limitations. The rows above are single-run synthetic baselines only; all unmeasured fields remain pending.
