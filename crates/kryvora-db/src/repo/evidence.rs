//! Repository for the `evidence` table.
//!
//! Evidence rows are **immutable after registration**. There is no
//! update method, no delete method, and no way to change the recorded
//! hash, size, or integrity state through this repository. Re-verification
//! is recorded as audit events by `kryvora-audit`, not by mutating this
//! row.

use super::{map_sql_err, now_rfc3339};
use kryvora_core::{CaseId, Error, EvidenceId, Result};
use rusqlite::Connection;

/// Fields a caller must supply to register evidence.
#[derive(Debug, Clone)]
pub struct NewEvidence {
    pub case_id: CaseId,
    pub source_type: String,
    pub source_path: String,
    pub size_bytes: u64,
    pub hash_algorithm: String,
    pub hash_digest: String,
    pub integrity_state: String,
    pub read_only: bool,
    pub tool_version: String,
    pub acquisition_metadata: Option<String>,
    pub notes: Option<String>,
}

/// A row from the `evidence` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRow {
    pub id: EvidenceId,
    pub case_id: CaseId,
    pub source_type: String,
    pub source_path: String,
    pub size_bytes: u64,
    pub hash_algorithm: String,
    pub hash_digest: String,
    pub integrity_state: String,
    pub read_only: bool,
    pub tool_version: String,
    pub acquisition_metadata: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

pub trait EvidenceRepository {
    fn insert(&self, new_evidence: &NewEvidence) -> Result<EvidenceId>;
    fn get(&self, id: &EvidenceId) -> Result<Option<EvidenceRow>>;
    fn list_by_case(&self, case_id: &CaseId) -> Result<Vec<EvidenceRow>>;
}

#[derive(Debug)]
pub struct SqliteEvidenceRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteEvidenceRepository<'a> {
    #[must_use]
    pub const fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl EvidenceRepository for SqliteEvidenceRepository<'_> {
    fn insert(&self, new_evidence: &NewEvidence) -> Result<EvidenceId> {
        if new_evidence.hash_digest.len() != 64
            || !new_evidence
                .hash_digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(Error::InvalidInput(
                "evidence hash_digest must be 64 hex characters".into(),
            ));
        }
        if new_evidence.hash_algorithm != "sha256" {
            return Err(Error::InvalidInput(
                "evidence hash_algorithm must be 'sha256'".into(),
            ));
        }
        if new_evidence.source_path.trim().is_empty() {
            return Err(Error::InvalidInput(
                "evidence source_path must not be empty".into(),
            ));
        }

        let id = EvidenceId::new();
        let now = now_rfc3339();

        self.conn
            .execute(
                "INSERT INTO evidence (\
                     id, case_id, source_type, source_path, size_bytes, \
                     hash_algorithm, hash_digest, integrity_state, \
                     read_only, tool_version, acquisition_metadata, notes, \
                     created_at\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                rusqlite::params![
                    id.to_string(),
                    new_evidence.case_id.to_string(),
                    new_evidence.source_type,
                    new_evidence.source_path,
                    new_evidence.size_bytes as i64,
                    new_evidence.hash_algorithm,
                    new_evidence.hash_digest.to_ascii_lowercase(),
                    new_evidence.integrity_state,
                    i64::from(new_evidence.read_only),
                    new_evidence.tool_version,
                    new_evidence.acquisition_metadata,
                    new_evidence.notes,
                    now,
                ],
            )
            .map_err(|e| map_sql_err("insert evidence", e))?;

        Ok(id)
    }

    fn get(&self, id: &EvidenceId) -> Result<Option<EvidenceRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, case_id, source_type, source_path, size_bytes, \
                        hash_algorithm, hash_digest, integrity_state, \
                        read_only, tool_version, acquisition_metadata, notes, \
                        created_at \
                 FROM evidence WHERE id = ?1",
            )
            .map_err(|e| map_sql_err("prepare get evidence", e))?;

        let mut rows = stmt
            .query_map(rusqlite::params![id.to_string()], row_to_evidence)
            .map_err(|e| map_sql_err("query get evidence", e))?;

        match rows.next() {
            None => Ok(None),
            Some(Ok(row)) => Ok(Some(row)),
            Some(Err(e)) => Err(map_sql_err("read evidence row", e)),
        }
    }

    fn list_by_case(&self, case_id: &CaseId) -> Result<Vec<EvidenceRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, case_id, source_type, source_path, size_bytes, \
                        hash_algorithm, hash_digest, integrity_state, \
                        read_only, tool_version, acquisition_metadata, notes, \
                        created_at \
                 FROM evidence WHERE case_id = ?1 ORDER BY created_at ASC, id ASC",
            )
            .map_err(|e| map_sql_err("prepare list evidence", e))?;

        let rows = stmt
            .query_map(rusqlite::params![case_id.to_string()], row_to_evidence)
            .map_err(|e| map_sql_err("query list evidence", e))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| map_sql_err("read evidence row", e))?);
        }
        Ok(out)
    }
}

fn row_to_evidence(row: &rusqlite::Row<'_>) -> rusqlite::Result<EvidenceRow> {
    let id_str: String = row.get(0)?;
    let case_str: String = row.get(1)?;

    let id = super::parse_stored_id(&id_str)
        .map(EvidenceId::from_uuid)
        .ok_or_else(|| conversion_err(0, "evidence.id is not a valid UUID"))?;

    let case_id = super::parse_stored_id(&case_str)
        .map(CaseId::from_uuid)
        .ok_or_else(|| conversion_err(1, "evidence.case_id is not a valid UUID"))?;

    let size_i: i64 = row.get(4)?;
    let read_only_i: i64 = row.get(8)?;

    Ok(EvidenceRow {
        id,
        case_id,
        source_type: row.get(2)?,
        source_path: row.get(3)?,
        size_bytes: size_i as u64,
        hash_algorithm: row.get(5)?,
        hash_digest: row.get(6)?,
        integrity_state: row.get(7)?,
        read_only: read_only_i != 0,
        tool_version: row.get(9)?,
        acquisition_metadata: row.get(10)?,
        notes: row.get(11)?,
        created_at: row.get(12)?,
    })
}

fn conversion_err(idx: usize, msg: &'static str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        idx,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, msg)),
    )
}
