#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Controlled-corpus test: a small container with known signatures at
//! known offsets. Every asserted value derives from the source data
//! visible in this file.

use kryvora_carving::run_carving_job;
use kryvora_carving::{run_carving_job_for_evidence_with_cancel, scan_source, ScanOptions};
use kryvora_db::{apply_migrations, open};
use kryvora_db::repo::{JobRepository, SqliteJobRepository};
use kryvora_jobs::CancelToken;
use rusqlite::Connection;
use std::io::{Cursor, Read};

/// Build a corpus with a jpeg, a png, and a pdf at known offsets.
fn build_corpus() -> (Vec<u8>, Vec<(usize, &'static str)>) {
    let jpeg_header = [0xFF, 0xD8, 0xFF];
    let jpeg_footer = [0xFF, 0xD9];
    let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let png_footer = [0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82];

    let mut data = Vec::new();
    let mut expected = Vec::new();

    // jpeg
    expected.push((data.len(), "jpeg"));
    data.extend_from_slice(&jpeg_header);
    data.extend_from_slice(&[0x42; 200]);
    data.extend_from_slice(&jpeg_footer);

    // filler
    data.extend_from_slice(&[0x00; 100]);

    // png
    expected.push((data.len(), "png"));
    data.extend_from_slice(&png_header);
    data.extend_from_slice(&[0x43; 200]);
    data.extend_from_slice(&png_footer);

    // filler
    data.extend_from_slice(&[0x00; 100]);

    // pdf
    expected.push((data.len(), "pdf"));
    data.extend_from_slice(b"%PDF-1.7\n");
    data.extend_from_slice(&[0x20; 200]);
    data.extend_from_slice(b"%%EOF");

    (data, expected)
}

#[test]
fn corpus_candidates_match_ground_truth_offsets() {
    let (data, expected) = build_corpus();
    let report = scan_source(Cursor::new(data), &ScanOptions::default()).unwrap();

    assert_eq!(
        report.candidates_found,
        expected.len() as u64,
        "candidate count must match ground truth"
    );

    let mut actual: Vec<(u64, String)> = report
        .candidates
        .iter()
        .map(|c| (c.offset, c.format.clone()))
        .collect();
    actual.sort_by_key(|(o, _)| *o);

    let mut truth: Vec<(u64, String)> = expected
        .into_iter()
        .map(|(o, f)| (o as u64, f.to_string()))
        .collect();
    truth.sort_by_key(|(o, _)| *o);

    assert_eq!(actual, truth);
}

#[test]
fn carving_job_appends_audit_event_and_verifies_chain() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("k.sqlite");
    let mut conn = open(&path).unwrap();
    apply_migrations(&mut conn).unwrap();

    let (data, _expected) = build_corpus();
    let report = run_carving_job(
        &conn,
        Cursor::new(data),
        ScanOptions::default(),
        Some("tester".into()),
    )
    .unwrap()
    .expect("job must produce a report");

    assert_eq!(report.candidates_found, 3);
    let completed = SqliteJobRepository::new(&conn)
        .list_by_state("succeeded", 10)
        .unwrap();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].progress, 1.0);

    // Audit chain must contain JobCreated, JobStarted, RecoveryStarted,
    // JobCompleted.
    assert_eq!(kryvora_audit::count(&conn).unwrap(), 4);
    assert_eq!(
        kryvora_audit::verify_chain(&conn).unwrap(),
        kryvora_audit::ChainStatus::Intact { length: 4 }
    );
}

#[test]
fn carving_job_on_empty_source_is_clean() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("k.sqlite");
    let mut conn = open(&path).unwrap();
    apply_migrations(&mut conn).unwrap();

    let report = run_carving_job(
        &conn,
        Cursor::new(Vec::<u8>::new()),
        ScanOptions::default(),
        None,
    )
    .unwrap()
    .expect("job must produce a report");

    assert_eq!(report.candidates_found, 0);
    assert_eq!(report.bytes_scanned, 0);
}

#[test]
fn cancelled_scan_persists_cancelled_job_and_audit_state() {
    struct CancelAfterFirstRead {
        reader: Cursor<Vec<u8>>,
        token: CancelToken,
        cancelled: bool,
    }

    impl Read for CancelAfterFirstRead {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let count = self.reader.read(buffer)?;
            if count > 0 && !self.cancelled {
                self.token.cancel();
                self.cancelled = true;
            }
            Ok(count)
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("k.sqlite");
    let mut conn = open(&path).unwrap();
    apply_migrations(&mut conn).unwrap();
    let cancel = CancelToken::new();
    let source = CancelAfterFirstRead {
        reader: Cursor::new(vec![0xAA; 4096]),
        token: cancel.clone(),
        cancelled: false,
    };

    let report = run_carving_job_for_evidence_with_cancel(
        &conn,
        source,
        ScanOptions::default(),
        Some("tester".into()),
        None,
        None,
        cancel,
    )
    .unwrap();

    assert!(report.is_none());
    let jobs = SqliteJobRepository::new(&conn)
        .list_by_state("cancelled", 10)
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_type, "carve");
    assert_eq!(
        kryvora_audit::verify_chain(&conn).unwrap(),
        kryvora_audit::ChainStatus::Intact { length: 3 }
    );
}

#[test]
fn candidate_limited_scan_returns_results_and_persists_partial_job_state() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("k.sqlite");
    let mut connection = open(&database).unwrap();
    apply_migrations(&mut connection).unwrap();

    let mut corpus = Vec::new();
    for _ in 0..100 {
        corpus.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
        corpus.extend_from_slice(&[0x11; 20]);
        corpus.extend_from_slice(&[0xFF, 0xD9]);
    }
    let report = kryvora_carving::run_carving_job_for_evidence(
        &connection,
        Cursor::new(corpus),
        ScanOptions {
            max_candidates: 16,
            source_size_bytes: Some(2_500),
            ..ScanOptions::default()
        },
        Some("tester".into()),
        None,
        None,
    )
    .unwrap()
    .expect("partial scan still returns bounded candidates");

    assert_eq!(report.candidates_found, 16);
    assert!(report.candidate_limit_reached);
    let jobs = SqliteJobRepository::new(&connection)
        .list_by_state("partial", 10)
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0]
        .error_message
        .as_deref()
        .unwrap_or("")
        .contains("candidate_limit=true"));
    assert_eq!(
        kryvora_audit::verify_chain(&connection).unwrap(),
        kryvora_audit::ChainStatus::Intact { length: 4 }
    );
}

// Silence an unused-import warning from the direct `Connection` import
// in this test file, which is not otherwise named.
#[allow(dead_code)]
fn _assert_conn_type(_c: &Connection) {}
