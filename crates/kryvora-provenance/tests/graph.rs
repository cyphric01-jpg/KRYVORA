#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Integration tests for the provenance graph against a real database.

use kryvora_core::{Error, ProvenanceId};
use kryvora_db::repo::{
    CaseRepository, NewCase, NewProvenanceNode, ProvenanceKind, ProvenanceRepository,
    SqliteCaseRepository, SqliteProvenanceRepository,
};
use kryvora_db::{apply_migrations, open};
use kryvora_provenance::ProvenanceGraph;
use rusqlite::Connection;

fn fresh_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.sqlite");
    let mut conn = open(&path).unwrap();
    apply_migrations(&mut conn).unwrap();
    (dir, conn)
}

fn make_case(conn: &Connection, title: &str) -> kryvora_core::CaseId {
    SqliteCaseRepository::new(conn)
        .insert(&NewCase {
            title: title.into(),
            examiner: None,
            notes: None,
        })
        .unwrap()
}

#[test]
fn children_are_returned_for_a_parent() {
    let (_dir, conn) = fresh_db();
    let case_id = make_case(&conn, "case");
    let repo = SqliteProvenanceRepository::new(&conn);

    let root = repo
        .insert(&NewProvenanceNode {
            case_id,
            kind: ProvenanceKind::Evidence,
            object_id: "EVID-x".into(),
            parent_id: None,
            note: None,
        })
        .unwrap();

    for i in 0..3 {
        repo.insert(&NewProvenanceNode {
            case_id,
            kind: ProvenanceKind::Candidate,
            object_id: format!("CAND-{i}"),
            parent_id: Some(root),
            note: None,
        })
        .unwrap();
    }

    let graph = ProvenanceGraph::new(&conn);
    let children = graph.children_of(&root).unwrap();
    assert_eq!(children.len(), 3);
}

#[test]
fn missing_start_node_is_not_found() {
    let (_dir, conn) = fresh_db();
    let graph = ProvenanceGraph::new(&conn);
    let err = graph.path_to_root(&ProvenanceId::new()).unwrap_err();
    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

#[test]
fn deeply_nested_chain_is_walked() {
    let (_dir, conn) = fresh_db();
    let case_id = make_case(&conn, "case");
    let repo = SqliteProvenanceRepository::new(&conn);

    let mut parent: Option<ProvenanceId> = None;
    let mut last: Option<ProvenanceId> = None;
    for i in 0..10 {
        let id = repo
            .insert(&NewProvenanceNode {
                case_id,
                kind: if i == 0 {
                    ProvenanceKind::Evidence
                } else {
                    ProvenanceKind::Candidate
                },
                object_id: format!("OBJ-{i}"),
                parent_id: parent,
                note: None,
            })
            .unwrap();
        parent = Some(id);
        last = Some(id);
    }

    let graph = ProvenanceGraph::new(&conn);
    let path = graph.path_to_root(&last.unwrap()).unwrap();
    assert_eq!(path.elements.len(), 10);
    assert_eq!(path.root().unwrap().object_id, "OBJ-0");
    assert_eq!(path.leaf().unwrap().object_id, "OBJ-9");
    assert_eq!(path.depth(), 9);
}
