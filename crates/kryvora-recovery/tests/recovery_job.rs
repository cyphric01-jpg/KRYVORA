#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! End-to-end recovery test: carve a synthetic source, then recover.

use kryvora_carving::{scan_source, CarvedCandidate, ScanOptions};
use kryvora_db::{apply_migrations, open};
use kryvora_recovery::run_recovery_job;
use std::io::Cursor;
use std::time::Instant;

fn synthetic_jpeg() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&[0xFF, 0xD8]);
    v.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x64, 0x00, 0xC8, 0x03, 0x01, 0x11, 0x00, 0x02, 0x11,
        0x01, 0x03, 0x11, 0x01,
    ]);
    v.extend_from_slice(&[0x00; 100]);
    v.extend_from_slice(&[0xFF, 0xD9]);
    v
}

fn synthetic_png() -> Vec<u8> {
    const SIG: &[u8; 8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let mut v = Vec::new();
    v.extend_from_slice(SIG);

    // IHDR chunk: length=13, type=IHDR, body=13 bytes, CRC=4 bytes
    // (validator does not check the CRC, so a placeholder is fine).
    v.extend_from_slice(&13u32.to_be_bytes());
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&[0, 0, 0, 100, 0, 0, 0, 200, 8, 6, 0, 0, 0]);
    v.extend_from_slice(&[0u8; 4]);

    // IEND chunk: length=0, type=IEND, body empty, CRC=AE 42 60 82.
    // The CRC is fixed because the chunk body is empty; the carving
    // signature expects exactly these bytes.
    v.extend_from_slice(&0u32.to_be_bytes());
    v.extend_from_slice(b"IEND");
    v.extend_from_slice(&[0xAE, 0x42, 0x60, 0x82]);

    v
}

#[test]
fn carve_then_recover_yields_valid_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("k.sqlite");
    let mut conn = open(&db).unwrap();
    apply_migrations(&mut conn).unwrap();

    // Build a source containing a jpeg and a png.
    let mut source = Vec::new();
    source.extend_from_slice(&[0x00; 50]);
    let jpeg_start = source.len();
    source.extend_from_slice(&synthetic_jpeg());
    source.extend_from_slice(&[0x00; 50]);
    let png_start = source.len();
    source.extend_from_slice(&synthetic_png());

    // Carve it. `scan_source` takes the reader by value, but
    // `Cursor::new(&source)` is fine here because `scan_source` does
    // not require `'static`.
    let carve_report = scan_source(Cursor::new(&source), &ScanOptions::default()).unwrap();
    assert_eq!(carve_report.candidates_found, 2);

    // Recover from it. `run_recovery_job` requires the reader to be
    // `'static`, so we hand it an owned cursor.
    let candidates: Vec<CarvedCandidate> = carve_report.candidates;
    let recovery_report = run_recovery_job(
        &conn,
        Cursor::new(source),
        candidates,
        None,
        Some("tester".into()),
    )
    .unwrap()
    .expect("must produce report");

    assert_eq!(recovery_report.candidates_considered, 2);
    assert_eq!(recovery_report.candidates_validated, 2);
    assert_eq!(recovery_report.candidates_rejected, 0);

    let mut offsets: Vec<u64> = recovery_report
        .results
        .iter()
        .map(|r| r.source_offset)
        .collect();
    offsets.sort_unstable();
    assert_eq!(offsets, vec![jpeg_start as u64, png_start as u64]);

    for r in &recovery_report.results {
        assert_eq!(r.category, kryvora_recovery::ArtifactCategory::Image);
    }
}

#[test]
fn garbage_candidates_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("k.sqlite");
    let mut conn = open(&db).unwrap();
    apply_migrations(&mut conn).unwrap();

    let source = vec![0u8; 200];
    let fake = CarvedCandidate {
        id: kryvora_core::FragmentId::new(),
        format: "jpeg".into(),
        offset: 0,
        length: 200,
        header_preview: vec![],
        sha256: None,
    };

    let report = run_recovery_job(&conn, Cursor::new(source), vec![fake], None, None)
        .unwrap()
        .expect("must produce report");

    assert_eq!(report.candidates_considered, 1);
    assert_eq!(report.candidates_validated, 0);
    assert_eq!(report.candidates_rejected, 1);
}

#[test]
fn oversized_and_overflowing_candidate_ranges_are_rejected_without_reading() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("k.sqlite");
    let mut conn = open(&db).unwrap();
    apply_migrations(&mut conn).unwrap();

    let candidates = vec![
        CarvedCandidate {
            id: kryvora_core::FragmentId::new(),
            format: "jpeg".into(),
            offset: 0,
            length: kryvora_recovery::job::MAX_RECOVERY_CANDIDATE_BYTES + 1,
            header_preview: vec![],
            sha256: None,
        },
        CarvedCandidate {
            id: kryvora_core::FragmentId::new(),
            format: "jpeg".into(),
            offset: u64::MAX,
            length: 1,
            header_preview: vec![],
            sha256: None,
        },
    ];

    let report = run_recovery_job(&conn, Cursor::new(Vec::<u8>::new()), candidates, None, None)
        .unwrap()
        .expect("job should complete with rejected candidates");

    assert_eq!(report.candidates_considered, 2);
    assert_eq!(report.candidates_validated, 0);
    assert_eq!(report.candidates_rejected, 2);
}

#[test]
#[ignore = "manual reproducible recovery performance measurement"]
fn benchmark_valid_jpeg_recovery_sizes() {
    for mebibytes in [1usize, 16, 64] {
        let target_len = mebibytes * 1024 * 1024;
        let mut bytes = synthetic_jpeg();
        bytes.truncate(bytes.len() - 2);
        bytes.resize(target_len - 2, 0x11);
        bytes.extend_from_slice(&[0xFF, 0xD9]);

        let candidate = CarvedCandidate {
            id: kryvora_core::FragmentId::new(),
            format: "jpeg".into(),
            offset: 0,
            length: bytes.len() as u64,
            header_preview: bytes[..bytes.len().min(16)].to_vec(),
            sha256: None,
        };
        let directory = tempfile::tempdir().unwrap();
        let mut connection = open(&directory.path().join("benchmark.sqlite")).unwrap();
        apply_migrations(&mut connection).unwrap();

        let started = Instant::now();
        let report = run_recovery_job(
            &connection,
            Cursor::new(bytes),
            vec![candidate],
            None,
            Some("benchmark".into()),
        )
        .unwrap()
        .expect("recovery job should complete");
        let elapsed = started.elapsed();
        let throughput = report.results[0].source_length as f64
            / (1024.0 * 1024.0)
            / elapsed.as_secs_f64().max(f64::MIN_POSITIVE);
        println!(
            "PERF fixture=valid-jpeg-recovery size_mib={mebibytes} elapsed_s={:.6} throughput_mib_s={throughput:.2} considered={} validated={} rejected={} validation={:?}",
            elapsed.as_secs_f64(),
            report.candidates_considered,
            report.candidates_validated,
            report.candidates_rejected,
            report.results.first().map(|result| result.validation_state),
        );
        assert_eq!(report.candidates_validated, 1);
        assert_eq!(report.candidates_rejected, 0);
    }
}
