//! The provenance graph traversal API.

use crate::node::NodeKind;
use kryvora_core::{Error, ProvenanceId, Result};
use kryvora_db::repo::{ProvenanceNodeRow, ProvenanceRepository, SqliteProvenanceRepository};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// A typed reference in a provenance path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenancePathElement {
    pub id: ProvenanceId,
    pub kind: NodeKind,
    pub object_id: String,
}

impl From<ProvenanceNodeRow> for ProvenancePathElement {
    fn from(row: ProvenanceNodeRow) -> Self {
        Self {
            id: row.id,
            kind: NodeKind::from_db_kind(row.kind),
            object_id: row.object_id,
        }
    }
}

/// A path from a leaf node up to the root (which must be an evidence
/// node).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenancePath {
    /// Ordered from the leaf (first element) to the root (last element).
    pub elements: Vec<ProvenancePathElement>,
}

impl ProvenancePath {
    /// The leaf of the path.
    #[must_use]
    pub fn leaf(&self) -> Option<&ProvenancePathElement> {
        self.elements.first()
    }

    /// The root of the path.
    #[must_use]
    pub fn root(&self) -> Option<&ProvenancePathElement> {
        self.elements.last()
    }

    /// Depth of the path in edges.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.elements.len().saturating_sub(1)
    }
}

/// A read-only view over a connection that answers provenance queries.
#[derive(Debug)]
pub struct ProvenanceGraph<'a> {
    conn: &'a Connection,
}

impl<'a> ProvenanceGraph<'a> {
    #[must_use]
    pub const fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Walk from a node to the root of its chain.
    ///
    /// # Errors
    ///
    /// * [`Error::NotFound`] — the starting node does not exist.
    /// * [`Error::Internal`] — a cycle was detected, which is a
    ///   violation of the graph's invariants.
    pub fn path_to_root(&self, start: &ProvenanceId) -> Result<ProvenancePath> {
        let repo = SqliteProvenanceRepository::new(self.conn);

        let mut elements: Vec<ProvenancePathElement> = Vec::new();
        let mut visited: Vec<ProvenanceId> = Vec::new();
        let mut current: Option<ProvenanceId> = Some(*start);

        while let Some(node_id) = current {
            if visited.contains(&node_id) {
                return Err(Error::Internal(format!(
                    "provenance cycle detected at {node_id}"
                )));
            }
            visited.push(node_id);

            let row = repo
                .get(&node_id)?
                .ok_or_else(|| Error::NotFound(format!("provenance {node_id}")))?;

            let parent = row.parent_id;
            elements.push(row.into());
            current = parent;
        }

        Ok(ProvenancePath { elements })
    }

    /// Return all direct children of a node.
    ///
    /// # Errors
    ///
    /// Propagates any repository error.
    pub fn children_of(&self, parent: &ProvenanceId) -> Result<Vec<ProvenancePathElement>> {
        let repo = SqliteProvenanceRepository::new(self.conn);
        let rows = repo.children_of(parent)?;
        Ok(rows.into_iter().map(ProvenancePathElement::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kryvora_db::repo::{NewProvenanceNode, ProvenanceRepository, SqliteProvenanceRepository};
    use kryvora_db::{apply_migrations, open};

    fn fresh_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.sqlite");
        let mut conn = open(&path).unwrap();
        apply_migrations(&mut conn).unwrap();
        (dir, conn)
    }

    fn make_case(conn: &Connection) -> kryvora_core::CaseId {
        use kryvora_db::repo::{CaseRepository, NewCase, SqliteCaseRepository};
        SqliteCaseRepository::new(conn)
            .insert(&NewCase {
                title: "case".into(),
                examiner: None,
                notes: None,
            })
            .unwrap()
    }

    #[test]
    fn path_to_root_walks_parent_links() {
        let (_dir, conn) = fresh_db();
        let case_id = make_case(&conn);
        let repo = SqliteProvenanceRepository::new(&conn);

        let root = repo
            .insert(&NewProvenanceNode {
                case_id,
                kind: kryvora_db::repo::ProvenanceKind::Evidence,
                object_id: "EVID-1".into(),
                parent_id: None,
                note: None,
            })
            .unwrap();

        let mid = repo
            .insert(&NewProvenanceNode {
                case_id,
                kind: kryvora_db::repo::ProvenanceKind::Candidate,
                object_id: "FRAG-1".into(),
                parent_id: Some(root),
                note: None,
            })
            .unwrap();

        let leaf = repo
            .insert(&NewProvenanceNode {
                case_id,
                kind: kryvora_db::repo::ProvenanceKind::Artifact,
                object_id: "REC-1".into(),
                parent_id: Some(mid),
                note: None,
            })
            .unwrap();

        let graph = ProvenanceGraph::new(&conn);
        let path = graph.path_to_root(&leaf).unwrap();

        assert_eq!(path.elements.len(), 3);
        assert_eq!(path.leaf().unwrap().object_id, "REC-1");
        assert_eq!(path.root().unwrap().object_id, "EVID-1");
        assert_eq!(path.depth(), 2);
    }

    #[test]
    fn path_to_root_returns_not_found_for_missing_id() {
        let (_dir, conn) = fresh_db();
        let graph = ProvenanceGraph::new(&conn);
        let err = graph
            .path_to_root(&kryvora_core::ProvenanceId::new())
            .unwrap_err();
        assert!(matches!(err, Error::NotFound(_)), "{err:?}");
    }
}
