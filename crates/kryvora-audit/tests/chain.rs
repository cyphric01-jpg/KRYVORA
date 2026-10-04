#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration tests for the audit chain against a real on-disk SQLite
//! database. Includes explicit tamper-detection tests: for each way a
//! row can be corrupted, the verifier must report a break at the
//! correct sequence with the correct reason.

use kryvora_audit::{
    append, count, latest_hash, verify_chain, AuditChain, ChainBreakReason, ChainStatus,
    EventDraft, EventType,
};
use kryvora_db::repo::GENESIS_PREVIOUS_HASH;
use kryvora_db::{apply_migrations, open};
use rusqlite::Connection;
use serde_json::json;

fn fresh_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kryvora.sqlite");
    let mut conn = open(&path).unwrap();
    apply_migrations(&mut conn).unwrap();
    (dir, conn)
}

fn draft(kind: EventType, tag: &str) -> EventDraft {
    EventDraft {
        event_type: kind,
        actor: Some("examiner".into()),
        object_id: Some(format!("CASE-{tag}")),
        job_id: None,
        details: json!({"tag": tag}),
    }
}

#[test]
fn empty_chain_is_empty() {
    let (_dir, conn) = fresh_db();
    assert_eq!(verify_chain(&conn).unwrap(), ChainStatus::Empty);
    assert_eq!(count(&conn).unwrap(), 0);
    assert_eq!(latest_hash(&conn).unwrap(), GENESIS_PREVIOUS_HASH);
}

#[test]
fn append_one_event_and_verify() {
    let (_dir, conn) = fresh_db();
    let _ = append(&conn, &draft(EventType::CaseCreated, "one")).unwrap();

    assert_eq!(count(&conn).unwrap(), 1);
    assert_ne!(latest_hash(&conn).unwrap(), GENESIS_PREVIOUS_HASH);
    assert_eq!(
        verify_chain(&conn).unwrap(),
        ChainStatus::Intact { length: 1 }
    );
}

#[test]
fn append_many_events_and_verify() {
    let (_dir, conn) = fresh_db();
    for i in 0..10 {
        let _ = append(&conn, &draft(EventType::CaseCreated, &format!("e{i}"))).unwrap();
    }
    assert_eq!(count(&conn).unwrap(), 10);
    assert_eq!(
        verify_chain(&conn).unwrap(),
        ChainStatus::Intact { length: 10 }
    );
}

#[test]
fn concurrent_connections_append_a_valid_chain() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("concurrent.sqlite");
    {
        let mut conn = open(&path).unwrap();
        apply_migrations(&mut conn).unwrap();
    }

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(5));
    let mut workers = Vec::new();
    for worker in 0..4 {
        let path = path.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let conn = open(&path).unwrap();
            barrier.wait();
            for item in 0..8 {
                append(
                    &conn,
                    &draft(EventType::CaseCreated, &format!("{worker}-{item}")),
                )
                .unwrap();
            }
        }));
    }
    barrier.wait();
    for worker in workers {
        worker.join().unwrap();
    }

    let conn = open(&path).unwrap();
    assert_eq!(count(&conn).unwrap(), 32);
    assert_eq!(
        verify_chain(&conn).unwrap(),
        ChainStatus::Intact { length: 32 }
    );
}

#[test]
fn chain_persists_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kryvora.sqlite");

    let hash_after_five: String;
    {
        let mut conn = open(&path).unwrap();
        apply_migrations(&mut conn).unwrap();
        for i in 0..5 {
            let _ = append(&conn, &draft(EventType::CaseCreated, &format!("e{i}"))).unwrap();
        }
        hash_after_five = latest_hash(&conn).unwrap();
        assert_eq!(
            verify_chain(&conn).unwrap(),
            ChainStatus::Intact { length: 5 }
        );
    }

    {
        let conn = open(&path).unwrap();
        assert_eq!(count(&conn).unwrap(), 5);
        assert_eq!(latest_hash(&conn).unwrap(), hash_after_five);
        assert_eq!(
            verify_chain(&conn).unwrap(),
            ChainStatus::Intact { length: 5 }
        );

        // Append after reopen: the chain continues from the persisted
        // tip.
        let _ = append(&conn, &draft(EventType::CaseCreated, "six")).unwrap();
        assert_eq!(count(&conn).unwrap(), 6);
        assert_eq!(
            verify_chain(&conn).unwrap(),
            ChainStatus::Intact { length: 6 }
        );
    }
}

#[test]
fn audit_chain_handle_matches_free_functions() {
    let (_dir, conn) = fresh_db();
    let chain = AuditChain::new(&conn);
    assert_eq!(chain.verify().unwrap(), ChainStatus::Empty);
    let _ = chain.append(&draft(EventType::CaseCreated, "one")).unwrap();
    assert_eq!(chain.count().unwrap(), 1);
    assert_eq!(chain.verify().unwrap(), ChainStatus::Intact { length: 1 });
    assert_eq!(chain.latest_hash().unwrap(), latest_hash(&conn).unwrap());
}

#[test]
fn tampering_with_details_is_detected() {
    let (_dir, conn) = fresh_db();
    for i in 0..5 {
        let _ = append(&conn, &draft(EventType::CaseCreated, &format!("e{i}"))).unwrap();
    }

    // Rewrite the details of row 3, leaving current_hash untouched.
    conn.execute(
        "UPDATE audit_events SET details = ?1 WHERE sequence = 3",
        rusqlite::params![r#"{"tag":"tampered"}"#],
    )
    .unwrap();

    let status = verify_chain(&conn).unwrap();
    match status {
        ChainStatus::Broken {
            first_bad_sequence,
            reason,
        } => {
            assert_eq!(first_bad_sequence, 3);
            assert!(
                matches!(reason, ChainBreakReason::CurrentHashMismatch { .. }),
                "{reason:?}"
            );
        }
        other => panic!("expected Broken at 3, got {other:?}"),
    }
}

#[test]
fn tampering_with_event_type_is_detected() {
    let (_dir, conn) = fresh_db();
    for i in 0..4 {
        let _ = append(&conn, &draft(EventType::CaseCreated, &format!("e{i}"))).unwrap();
    }

    conn.execute(
        "UPDATE audit_events SET event_type = 'examiner_note' WHERE sequence = 2",
        [],
    )
    .unwrap();

    match verify_chain(&conn).unwrap() {
        ChainStatus::Broken {
            first_bad_sequence,
            reason,
        } => {
            assert_eq!(first_bad_sequence, 2);
            assert!(
                matches!(reason, ChainBreakReason::CurrentHashMismatch { .. }),
                "{reason:?}"
            );
        }
        other => panic!("expected Broken at 2, got {other:?}"),
    }
}

#[test]
fn tampering_with_previous_hash_is_detected() {
    let (_dir, conn) = fresh_db();
    for i in 0..4 {
        let _ = append(&conn, &draft(EventType::CaseCreated, &format!("e{i}"))).unwrap();
    }

    // Replace the previous_hash of row 2 with a valid-looking but wrong
    // 64-char hex string.
    conn.execute(
        "UPDATE audit_events SET previous_hash = ?1 WHERE sequence = 2",
        rusqlite::params!["f".repeat(64)],
    )
    .unwrap();

    match verify_chain(&conn).unwrap() {
        ChainStatus::Broken {
            first_bad_sequence,
            reason,
        } => {
            assert_eq!(first_bad_sequence, 2);
            assert!(
                matches!(reason, ChainBreakReason::PreviousHashMismatch { .. }),
                "{reason:?}"
            );
        }
        other => panic!("expected Broken at 2, got {other:?}"),
    }
}

#[test]
fn tampering_with_current_hash_is_detected() {
    let (_dir, conn) = fresh_db();
    for i in 0..4 {
        let _ = append(&conn, &draft(EventType::CaseCreated, &format!("e{i}"))).unwrap();
    }

    conn.execute(
        "UPDATE audit_events SET current_hash = ?1 WHERE sequence = 1",
        rusqlite::params!["a".repeat(64)],
    )
    .unwrap();

    // The first break is at sequence 1: current_hash does not equal the
    // recomputed value.
    match verify_chain(&conn).unwrap() {
        ChainStatus::Broken {
            first_bad_sequence,
            reason,
        } => {
            assert_eq!(first_bad_sequence, 1);
            assert!(
                matches!(reason, ChainBreakReason::CurrentHashMismatch { .. }),
                "{reason:?}"
            );
        }
        other => panic!("expected Broken at 1, got {other:?}"),
    }
}

#[test]
fn deleted_row_is_detected() {
    let (_dir, conn) = fresh_db();
    for i in 0..5 {
        let _ = append(&conn, &draft(EventType::CaseCreated, &format!("e{i}"))).unwrap();
    }

    conn.execute("DELETE FROM audit_events WHERE sequence = 2", [])
        .unwrap();

    match verify_chain(&conn).unwrap() {
        ChainStatus::Broken {
            first_bad_sequence,
            reason,
        } => {
            assert_eq!(first_bad_sequence, 2);
            match reason {
                ChainBreakReason::SequenceGap { expected, found } => {
                    assert_eq!(expected, 2);
                    assert_eq!(found, 3);
                }
                other => panic!("expected SequenceGap, got {other:?}"),
            }
        }
        other => panic!("expected Broken at 2, got {other:?}"),
    }
}

#[test]
fn reordered_rows_are_detected() {
    // The schema has a UNIQUE constraint on `sequence`, so we cannot
    // duplicate sequence numbers. We swap the sequences of two rows in
    // the middle of the chain. The sequence numbers in the resulting
    // set are still 0..N contiguous, but the cryptographic links no
    // longer match: the row now at position 1 carries a `previous_hash`
    // that points to the original row 1's `current_hash`, not to the
    // `current_hash` of the row that is actually at position 0.
    //
    // Detection therefore comes from `PreviousHashMismatch`, not
    // `SequenceGap`. Both are valid tamper detectors; they fire on
    // different mutations:
    //   * Deleting a row          -> SequenceGap
    //   * Swapping adjacent rows  -> PreviousHashMismatch
    let (_dir, conn) = fresh_db();
    for i in 0..4 {
        let _ = append(&conn, &draft(EventType::CaseCreated, &format!("e{i}"))).unwrap();
    }

    // Move row 1 out of the way, then move row 2 into position 1.
    // SQLite treats `UPDATE ... SET sequence = X` as a per-row
    // constraint check, so a three-step swap is required.
    conn.execute(
        "UPDATE audit_events SET sequence = 99 WHERE sequence = 1",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE audit_events SET sequence = 1 WHERE sequence = 2",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE audit_events SET sequence = 2 WHERE sequence = 99",
        [],
    )
    .unwrap();

    // Sequence order is now 0, 1, 2, 3 (contiguous), but rows 1 and 2
    // are swapped. The verifier detects the cryptographic break at 1.
    match verify_chain(&conn).unwrap() {
        ChainStatus::Broken {
            first_bad_sequence,
            reason,
        } => {
            assert_eq!(first_bad_sequence, 1);
            assert!(
                matches!(reason, ChainBreakReason::PreviousHashMismatch { .. }),
                "{reason:?}"
            );
        }
        other => panic!("expected Broken at 1, got {other:?}"),
    }
}

#[test]
fn malformed_event_type_is_reported_as_malformed_row() {
    let (_dir, conn) = fresh_db();
    for i in 0..3 {
        let _ = append(&conn, &draft(EventType::CaseCreated, &format!("e{i}"))).unwrap();
    }

    conn.execute(
        "UPDATE audit_events SET event_type = 'invented_event' WHERE sequence = 1",
        [],
    )
    .unwrap();

    match verify_chain(&conn).unwrap() {
        ChainStatus::Broken {
            first_bad_sequence,
            reason,
        } => {
            assert_eq!(first_bad_sequence, 1);
            assert!(
                matches!(reason, ChainBreakReason::MalformedRow { .. }),
                "{reason:?}"
            );
        }
        other => panic!("expected Broken at 1, got {other:?}"),
    }
}

#[test]
fn append_after_break_does_not_repair_the_chain() {
    let (_dir, conn) = fresh_db();
    for i in 0..3 {
        let _ = append(&conn, &draft(EventType::CaseCreated, &format!("e{i}"))).unwrap();
    }

    conn.execute(
        "UPDATE audit_events SET details = ?1 WHERE sequence = 1",
        rusqlite::params![r#"{"tag":"tampered"}"#],
    )
    .unwrap();

    // Append a fresh event. The chain is still broken at 1 because
    // append uses the recorded tip, not a recomputed one.
    let _ = append(&conn, &draft(EventType::ExaminerNote, "post")).unwrap();

    match verify_chain(&conn).unwrap() {
        ChainStatus::Broken {
            first_bad_sequence,
            reason,
        } => {
            assert_eq!(first_bad_sequence, 1);
            assert!(
                matches!(reason, ChainBreakReason::CurrentHashMismatch { .. }),
                "{reason:?}"
            );
        }
        other => panic!("expected Broken at 1, got {other:?}"),
    }
}

#[test]
fn key_order_in_details_does_not_affect_the_chain() {
    // The details of two events differ only in key order. Both produce
    // the same canonical bytes, so both hash the same way, and the
    // chain verifies.
    let (_dir, conn) = fresh_db();

    let a = EventDraft {
        event_type: EventType::CaseCreated,
        actor: Some("examiner".into()),
        object_id: Some("CASE-x".into()),
        job_id: None,
        details: json!({"alpha": 1, "beta": 2}),
    };
    let b = EventDraft {
        event_type: EventType::CaseCreated,
        actor: Some("examiner".into()),
        object_id: Some("CASE-x".into()),
        job_id: None,
        details: json!({"beta": 2, "alpha": 1}),
    };

    let _ = append(&conn, &a).unwrap();
    let _ = append(&conn, &b).unwrap();

    assert_eq!(
        verify_chain(&conn).unwrap(),
        ChainStatus::Intact { length: 2 }
    );
}

#[test]
fn empty_details_object_is_valid() {
    let (_dir, conn) = fresh_db();
    let d = EventDraft {
        event_type: EventType::ExaminerNote,
        actor: None,
        object_id: None,
        job_id: None,
        details: json!({}),
    };
    let _ = append(&conn, &d).unwrap();
    assert_eq!(
        verify_chain(&conn).unwrap(),
        ChainStatus::Intact { length: 1 }
    );
}
