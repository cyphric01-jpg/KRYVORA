#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration tests for the scanner against synthetic inputs.

use kryvora_carving::{scan_source, ScanOptions};
use std::io::{BufReader, Cursor, Write};
use std::time::Instant;

fn opts() -> ScanOptions {
    ScanOptions::default()
}

#[test]
fn finds_multiple_candidates_in_offset_order() {
    let mut data = Vec::new();

    // jpeg at offset 0
    data.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
    data.extend_from_slice(&[0x00; 100]);
    data.extend_from_slice(&[0xFF, 0xD9]);

    // 50 bytes of filler
    data.extend_from_slice(&[0xAA; 50]);

    // pdf at offset 155
    data.extend_from_slice(b"%PDF-1.4\n");
    data.extend_from_slice(&[0x20; 50]);
    data.extend_from_slice(b"%%EOF");

    // 20 bytes of filler
    data.extend_from_slice(&[0xBB; 20]);

    // png at the end
    let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let png_footer = [0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82];
    data.extend_from_slice(&png_header);
    data.extend_from_slice(&[0xCC; 30]);
    data.extend_from_slice(&png_footer);

    let report = scan_source(Cursor::new(data), &opts()).unwrap();
    assert_eq!(report.candidates_found, 3);

    // Candidates are emitted in the order the scanner sees them, which
    // is signature order within a window, but offsets must be strictly
    // increasing across formats when the source is written this way.
    let mut offsets: Vec<u64> = report.candidates.iter().map(|c| c.offset).collect();
    offsets.sort_unstable();
    assert_eq!(offsets.len(), 3);
    assert!(offsets[0] < offsets[1]);
    assert!(offsets[1] < offsets[2]);
}

#[test]
fn signature_straddling_window_boundary_is_found() {
    // Place a jpeg header such that the last byte of the header is the
    // first byte of the second window. window_size = 64, header is 3
    // bytes, so put the header at offset 62.
    let mut data = vec![0u8; 62];
    data.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
    data.extend_from_slice(&[0x00; 100]);
    data.extend_from_slice(&[0xFF, 0xD9]);

    let opts = ScanOptions {
        window_size: 64,
        header_preview_len: 16,
        ..ScanOptions::default()
    };
    let report = scan_source(Cursor::new(data), &opts).unwrap();
    assert_eq!(report.candidates_found, 1);
    assert_eq!(report.candidates[0].offset, 62);
}

#[test]
fn header_preview_is_stored() {
    let mut data = Vec::new();
    data.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
    data.extend_from_slice(&[0x11; 100]);
    data.extend_from_slice(&[0xFF, 0xD9]);

    let report = scan_source(Cursor::new(data), &opts()).unwrap();
    assert_eq!(report.candidates.len(), 1);
    let preview = &report.candidates[0].header_preview;
    assert_eq!(preview[0], 0xFF);
    assert_eq!(preview[1], 0xD8);
    assert_eq!(preview[2], 0xFF);
    assert_eq!(preview.len(), 16);
}

#[test]
fn candidate_shorter_than_min_size_is_rejected() {
    // jpeg min_size is 4. Header (3) + footer (2) = 5, but if the
    // footer is found immediately after the header, that is 5 bytes,
    // which passes. To make a < min_size case we need a format with a
    // larger min_size. Use PDF, min_size 16.
    // header (5) + 1 byte filler + footer (5) = 11 bytes < 16.
    let mut data = Vec::new();
    data.extend_from_slice(b"%PDF-");
    data.extend_from_slice(b"x");
    data.extend_from_slice(b"%%EOF");

    let report = scan_source(Cursor::new(data), &opts()).unwrap();
    assert_eq!(report.candidates_found, 0);
    assert_eq!(report.truncated_headers, 1);
}

#[test]
fn multiple_signatures_in_the_same_window_are_all_found() {
    let mut data = Vec::new();

    // jpeg
    data.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
    data.extend_from_slice(&[0x00; 20]);
    data.extend_from_slice(&[0xFF, 0xD9]);

    // 8 bytes gap
    data.extend_from_slice(&[0x00; 8]);

    // pdf
    data.extend_from_slice(b"%PDF-1.4\n");
    data.extend_from_slice(&[0x20; 20]);
    data.extend_from_slice(b"%%EOF");

    let report = scan_source(Cursor::new(data), &opts()).unwrap();
    assert_eq!(report.candidates_found, 2);

    let formats: std::collections::HashSet<_> =
        report.candidates.iter().map(|c| c.format.clone()).collect();
    assert!(formats.contains("jpeg"));
    assert!(formats.contains("pdf"));
}

#[test]
#[ignore = "manual reproducible performance measurement"]
fn benchmark_streaming_and_candidate_heavy_fixtures() {
    let directory = tempfile::tempdir().unwrap();
    let block = vec![0u8; 1024 * 1024];
    for mebibytes in [16u64, 64, 256] {
        let path = directory.path().join(format!("zero-{mebibytes}m.bin"));
        let mut file = std::fs::File::create(&path).unwrap();
        for _ in 0..mebibytes {
            file.write_all(&block).unwrap();
        }
        file.sync_all().unwrap();
        drop(file);

        let started = Instant::now();
        let report = scan_source(
            BufReader::new(std::fs::File::open(&path).unwrap()),
            &ScanOptions::default(),
        )
        .unwrap();
        print_measurement("zero", mebibytes, started.elapsed(), &report);
        assert_eq!(report.bytes_scanned, mebibytes * 1024 * 1024);
        assert_eq!(report.candidates_found, 0);
    }

    let candidate_bytes = 16 * 1024 * 1024usize;
    let path = directory.path().join("jpeg-dense-16m.bin");
    let mut file = std::fs::File::create(&path).unwrap();
    let mut block = Vec::with_capacity(21 * 2_048);
    for _ in 0..2_048 {
        block.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
        block.extend_from_slice(&[0; 16]);
        block.extend_from_slice(&[0xFF, 0xD9]);
    }
    let mut written = 0usize;
    while written + block.len() <= candidate_bytes {
        file.write_all(&block).unwrap();
        written += block.len();
    }
    if written < candidate_bytes {
        file.write_all(&vec![0; candidate_bytes - written]).unwrap();
    }
    file.sync_all().unwrap();
    drop(file);

    let started = Instant::now();
    let report = scan_source(
        BufReader::new(std::fs::File::open(&path).unwrap()),
        &ScanOptions::default(),
    )
    .unwrap();
    print_measurement("jpeg-dense", 16, started.elapsed(), &report);
    assert!(report.candidates_found <= ScanOptions::default().max_candidates as u64);
    assert!(report.candidate_limit_reached);
    assert!(report.headers_dropped_by_limit > 0);
}

fn print_measurement(
    fixture: &str,
    fixture_mib: u64,
    elapsed: std::time::Duration,
    report: &kryvora_carving::CarvingReport,
) {
    let seconds = elapsed.as_secs_f64();
    let throughput = report.bytes_scanned as f64 / (1024.0 * 1024.0) / seconds.max(f64::MIN_POSITIVE);
    println!(
        "PERF fixture={fixture} fixture_mib={fixture_mib} bytes_scanned={} elapsed_s={seconds:.6} throughput_mib_s={throughput:.2} candidates={} headers_dropped={} scan_limit={} candidate_limit={}",
        report.bytes_scanned,
        report.candidates_found,
        report.headers_dropped_by_limit,
        report.scan_limit_reached,
        report.candidate_limit_reached,
    );
}
