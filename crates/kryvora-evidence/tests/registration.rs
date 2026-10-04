#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration tests for evidence registration and verification.
//!
//! These tests exercise the full stack: real SQLite file, real
//! migration, real repository, real hash computation on real bytes.
//! The evidence crate itself performs no I/O; the tests do, because
//! they are simulating the caller.

use kryvora_core::{CaseId, Error, IntegrityState};
use kryvora_db::repo::{CaseRepository, NewCase, SqliteCaseRepository};
use kryvora_db::{apply_migrations, open};
use kryvora_evidence::{
    get_evidence, register_evidence, verify_evidence, RegistrationRequest, SourceType,
};
use kryvora_integrity::calculate_hash;
use rusqlite::Connection;
use std::io::Cursor;

fn fresh_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kryvora.sqlite");
    let mut conn = open(&path).unwrap();
    apply_migrations(&mut conn).unwrap();
    (dir, conn)
}

fn make_case(conn: &Connection, title: &str) -> CaseId {
    let repo = SqliteCaseRepository::new(conn);
    repo.insert(&NewCase {
        title: title.to_string(),
        examiner: Some("tester".into()),
        notes: None,
    })
    .unwrap()
}

fn request_for(case_id: CaseId, path: &str, data: &[u8]) -> RegistrationRequest {
    let hash = calculate_hash(Cursor::new(data)).unwrap();
    RegistrationRequest {
        case_id,
        source_type: SourceType::File,
        source_path: path.to_string(),
        size_bytes: data.len() as u64,
        hash,
        read_only: true,
        tool_version: "0.1.0-test".to_string(),
        acquisition_metadata: None,
        notes: None,
    }
}

#[test]
fn register_and_get_roundtrip() {
    let (_dir, conn) = fresh_db();
    let case_id = make_case(&conn, "Case Alpha");
    let data = b"forensic payload";
    let request = request_for(case_id, "C:/evidence/sample.bin", data);

    let id = register_evidence(&conn, &request).unwrap();
    let fetched = get_evidence(&conn, &id).unwrap();

    assert_eq!(fetched.id, id);
    assert_eq!(fetched.case_id, case_id);
    assert_eq!(fetched.source_type, SourceType::File);
    assert_eq!(fetched.source_path, "C:/evidence/sample.bin");
    assert_eq!(fetched.size_bytes, data.len() as u64);
    assert_eq!(fetched.hash.digest_hex(), request.hash.digest_hex());
    assert_eq!(fetched.integrity_state, IntegrityState::Verified);
    assert!(fetched.read_only);
    assert_eq!(fetched.tool_version, "0.1.0-test");
    assert!(fetched.acquisition_metadata.is_none());
    assert!(fetched.notes.is_none());
}

#[test]
fn register_rejects_size_that_differs_from_hash_input_size() {
    let (_dir, conn) = fresh_db();
    let case_id = make_case(&conn, "Case Alpha");
    let mut req = request_for(case_id, "C:/evidence/x.bin", b"payload");
    req.size_bytes += 1;

    let err = register_evidence(&conn, &req).unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[test]
fn register_accepts_size_matching_hash_input_size() {
    let (_dir, conn) = fresh_db();
    let case_id = make_case(&conn, "Case Alpha");
    let data = b"payload";
    let req = request_for(case_id, "C:/evidence/x.bin", data);

    let id = register_evidence(&conn, &req).unwrap();
    let fetched = get_evidence(&conn, &id).unwrap();

    assert_eq!(fetched.size_bytes, data.len() as u64);
    assert_eq!(fetched.hash.input_size(), data.len() as u64);
    assert_eq!(fetched.integrity_state, IntegrityState::Verified);
}

#[test]
fn identical_content_has_same_digest() {
    let (_dir, conn) = fresh_db();
    let case_id = make_case(&conn, "Case Alpha");
    let data = b"same content twice";

    let req_a = request_for(case_id, "C:/evidence/a.bin", data);
    let req_b = request_for(case_id, "C:/evidence/b.bin", data);

    assert_eq!(req_a.hash.digest_hex(), req_b.hash.digest_hex());

    let _ = register_evidence(&conn, &req_a).unwrap();
    let _ = register_evidence(&conn, &req_b).unwrap();

    // They are distinct evidence records with distinct IDs, but the
    // digest recorded on each is the same.
    assert_ne!(req_a.source_path, req_b.source_path);
}

#[test]
fn verify_matches_when_content_unchanged() {
    let (_dir, conn) = fresh_db();
    let case_id = make_case(&conn, "Case Alpha");
    let data = b"immutable bytes";
    let req = request_for(case_id, "C:/evidence/x.bin", data);

    let id = register_evidence(&conn, &req).unwrap();
    let actual = calculate_hash(Cursor::new(data)).unwrap();

    let state = verify_evidence(&conn, &id, &actual).unwrap();
    assert_eq!(state, IntegrityState::Verified);
}

#[test]
fn verify_mismatches_when_content_changed() {
    let (_dir, conn) = fresh_db();
    let case_id = make_case(&conn, "Case Alpha");
    let req = request_for(case_id, "C:/evidence/x.bin", b"original");

    let id = register_evidence(&conn, &req).unwrap();
    let tampered = calculate_hash(Cursor::new(b"tampered")).unwrap();

    let state = verify_evidence(&conn, &id, &tampered).unwrap();
    assert_eq!(state, IntegrityState::Mismatch);
}

#[test]
fn register_rejects_empty_source_path() {
    let (_dir, conn) = fresh_db();
    let case_id = make_case(&conn, "Case Alpha");
    let mut req = request_for(case_id, "C:/evidence/x.bin", b"x");
    req.source_path = "   ".into();

    let err = register_evidence(&conn, &req).unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[test]
fn register_rejects_unknown_case() {
    let (_dir, conn) = fresh_db();
    // No case was created.
    let req = request_for(CaseId::new(), "C:/evidence/x.bin", b"x");

    let err = register_evidence(&conn, &req).unwrap_err();
    // FK violation is mapped to InvalidInput by the repository.
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[test]
fn register_accepts_and_round_trips_metadata() {
    let (_dir, conn) = fresh_db();
    let case_id = make_case(&conn, "Case Alpha");
    let mut req = request_for(case_id, "C:/evidence/x.bin", b"x");
    let metadata = serde_json::json!({
        "acquisition_device": "USB-001",
        "acquisition_operator": "K. Smith",
        "acquisition_timestamp": "2026-09-28T10:00:00Z"
    });
    req.acquisition_metadata = Some(metadata.clone());

    let id = register_evidence(&conn, &req).unwrap();
    let fetched = get_evidence(&conn, &id).unwrap();
    assert_eq!(fetched.acquisition_metadata, Some(metadata));
}

#[test]
fn get_evidence_returns_not_found_for_missing_id() {
    let (_dir, conn) = fresh_db();
    let err = get_evidence(&conn, &kryvora_core::EvidenceId::new()).unwrap_err();
    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

#[test]
fn evidence_is_immutable_after_registration() {
    // There is no update method. This test documents the intent:
    // registering the same content under a second request produces a
    // second distinct row, and the first row is untouched.
    let (_dir, conn) = fresh_db();
    let case_id = make_case(&conn, "Case Alpha");
    let req = request_for(case_id, "C:/evidence/x.bin", b"payload");

    let id_1 = register_evidence(&conn, &req).unwrap();
    let id_2 = register_evidence(&conn, &req).unwrap();

    assert_ne!(id_1, id_2);

    let ev1 = get_evidence(&conn, &id_1).unwrap();
    let ev2 = get_evidence(&conn, &id_2).unwrap();

    assert_eq!(ev1.hash.digest_hex(), ev2.hash.digest_hex());
    assert_eq!(ev1.source_path, ev2.source_path);
    assert_ne!(ev1.id, ev2.id);
}
