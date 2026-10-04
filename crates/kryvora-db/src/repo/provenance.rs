//! Repository for the `provenance_nodes` table.

use super::{map_sql_err, now_rfc3339};
use kryvora_core::{CaseId, Error, ProvenanceId, Result};
use rusqlite::Connection;

/// The kind of object a provenance node points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProvenanceKind {
    Evidence,
    Candidate,
    Job,
    Artifact,
    Report,
}

impl ProvenanceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Evidence => "evidence",
            Self::Candidate => "candidate",
            Self::Job => "job",
            Self::Artifact => "artifact",
            Self::Report => "report",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "evidence" => Some(Self::Evidence),
            "candidate" => Some(Self::Candidate),
            "job" => Some(Self::Job),
            "artifact" => Some(Self::Artifact),
            "report" => Some(Self::Report),
            _ => None,
        }
    }
}

/// Caller-supplied fields for a new provenance node.
#[derive(Debug, Clone)]
pub struct NewProvenanceNode {
    pub case_id: CaseId,
    pub kind: ProvenanceKind,
    pub object_id: String,
    pub parent_id: Option<ProvenanceId>,
    pub note: Option<String>,
}

/// A row from the `provenance_nodes` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceNodeRow {
    pub id: ProvenanceId,
    pub case_id: CaseId,
    pub kind: ProvenanceKind,
    pub object_id: String,
    pub parent_id: Option<ProvenanceId>,
    pub note: Option<String>,
    pub created_at: String,
}

pub trait ProvenanceRepository {
    fn insert(&self, node: &NewProvenanceNode) -> Result<ProvenanceId>;
    fn get(&self, id: &ProvenanceId) -> Result<Option<ProvenanceNodeRow>>;
    fn children_of(&self, parent: &ProvenanceId) -> Result<Vec<ProvenanceNodeRow>>;
    fn find_by_object(&self, case_id: &CaseId, object_id: &str) -> Result<Vec<ProvenanceNodeRow>>;
}

#[derive(Debug)]
pub struct SqliteProvenanceRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteProvenanceRepository<'a> {
    #[must_use]
    pub const fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl ProvenanceRepository for SqliteProvenanceRepository<'_> {
    fn insert(&self, node: &NewProvenanceNode) -> Result<ProvenanceId> {
        if node.object_id.trim().is_empty() {
            return Err(Error::InvalidInput("object_id must not be empty".into()));
        }

        let id = ProvenanceId::new();
        let now = now_rfc3339();

        self.conn
            .execute(
                "INSERT INTO provenance_nodes (id, case_id, kind, object_id, parent_id, note, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    id.to_string(),
                    node.case_id.to_string(),
                    node.kind.as_str(),
                    node.object_id,
                    node.parent_id.as_ref().map(std::string::ToString::to_string),
                    node.note,
                    now,
                ],
            )
            .map_err(|e| map_sql_err("insert provenance node", e))?;

        Ok(id)
    }

    fn get(&self, id: &ProvenanceId) -> Result<Option<ProvenanceNodeRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, case_id, kind, object_id, parent_id, note, created_at \
                 FROM provenance_nodes WHERE id = ?1",
            )
            .map_err(|e| map_sql_err("prepare get provenance", e))?;

        let mut rows = stmt
            .query_map(rusqlite::params![id.to_string()], row_to_node)
            .map_err(|e| map_sql_err("query get provenance", e))?;

        match rows.next() {
            None => Ok(None),
            Some(Ok(row)) => Ok(Some(row)),
            Some(Err(e)) => Err(map_sql_err("read provenance row", e)),
        }
    }

    fn children_of(&self, parent: &ProvenanceId) -> Result<Vec<ProvenanceNodeRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, case_id, kind, object_id, parent_id, note, created_at \
                 FROM provenance_nodes WHERE parent_id = ?1 ORDER BY created_at ASC, id ASC",
            )
            .map_err(|e| map_sql_err("prepare children_of", e))?;

        let rows = stmt
            .query_map(rusqlite::params![parent.to_string()], row_to_node)
            .map_err(|e| map_sql_err("query children_of", e))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| map_sql_err("read provenance row", e))?);
        }
        Ok(out)
    }

    fn find_by_object(&self, case_id: &CaseId, object_id: &str) -> Result<Vec<ProvenanceNodeRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, case_id, kind, object_id, parent_id, note, created_at \
                 FROM provenance_nodes WHERE case_id = ?1 AND object_id = ?2 \
                 ORDER BY created_at ASC, id ASC",
            )
            .map_err(|e| map_sql_err("prepare find_by_object", e))?;

        let rows = stmt
            .query_map(
                rusqlite::params![case_id.to_string(), object_id],
                row_to_node,
            )
            .map_err(|e| map_sql_err("query find_by_object", e))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| map_sql_err("read provenance row", e))?);
        }
        Ok(out)
    }
}

fn row_to_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProvenanceNodeRow> {
    let id_str: String = row.get(0)?;
    let case_str: String = row.get(1)?;
    let kind_str: String = row.get(2)?;
    let parent_str: Option<String> = row.get(4)?;

    let id = super::parse_stored_id(&id_str)
        .map(ProvenanceId::from_uuid)
        .ok_or_else(|| conv_err(0, "provenance_nodes.id is not a valid UUID"))?;

    let case_id = super::parse_stored_id(&case_str)
        .map(CaseId::from_uuid)
        .ok_or_else(|| conv_err(1, "provenance_nodes.case_id is not a valid UUID"))?;

    let kind = ProvenanceKind::parse(&kind_str)
        .ok_or_else(|| conv_err(2, "provenance_nodes.kind is not a known kind"))?;

    let parent_id = match parent_str.as_deref() {
        None => None,
        Some(s) => Some(
            super::parse_stored_id(s)
                .map(ProvenanceId::from_uuid)
                .ok_or_else(|| conv_err(4, "provenance_nodes.parent_id is not a valid UUID"))?,
        ),
    };

    Ok(ProvenanceNodeRow {
        id,
        case_id,
        kind,
        object_id: row.get(3)?,
        parent_id,
        note: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn conv_err(idx: usize, msg: &'static str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        idx,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, msg)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_kind_roundtrip() {
        for k in [
            ProvenanceKind::Evidence,
            ProvenanceKind::Candidate,
            ProvenanceKind::Job,
            ProvenanceKind::Artifact,
            ProvenanceKind::Report,
        ] {
            assert_eq!(ProvenanceKind::parse(k.as_str()), Some(k));
        }
    }

    #[test]
    fn provenance_kind_rejects_unknown() {
        assert_eq!(ProvenanceKind::parse("bogus"), None);
    }
}
