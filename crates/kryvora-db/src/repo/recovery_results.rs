//! Repository for the `recovery_results` table.

use super::{map_sql_err, now_rfc3339};
use kryvora_core::{EvidenceId, JobId, ProvenanceId, RecoveryResultId, Result};
use rusqlite::Connection;

/// Caller-supplied fields for a new recovery result.
#[derive(Debug, Clone)]
pub struct NewRecoveryResult {
    pub id: Option<RecoveryResultId>,
    pub evidence_id: EvidenceId,
    pub job_id: Option<JobId>,
    pub provenance_id: Option<ProvenanceId>,
    pub source_offset: u64,
    pub source_length: u64,
    pub detected_type: String,
    pub category: String,
    pub validation_state: String,
    pub confidence_level: String,
    pub confidence_reasons: String,
    pub recovery_method: String,
    pub reconstruction_state: String,
    pub artifact_sha256: String,
    pub artifact_path: Option<String>,
    pub validation_facts: String,
}

/// A row from the `recovery_results` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryResultRow {
    pub id: RecoveryResultId,
    pub evidence_id: EvidenceId,
    pub job_id: Option<JobId>,
    pub provenance_id: Option<ProvenanceId>,
    pub source_offset: u64,
    pub source_length: u64,
    pub detected_type: String,
    pub category: String,
    pub validation_state: String,
    pub confidence_level: String,
    pub confidence_reasons: String,
    pub recovery_method: String,
    pub reconstruction_state: String,
    pub artifact_sha256: String,
    pub artifact_path: Option<String>,
    pub validation_facts: String,
    pub created_at: String,
}

pub trait RecoveryResultRepository {
    fn insert(&self, result: &NewRecoveryResult) -> Result<RecoveryResultId>;
    fn get(&self, id: &RecoveryResultId) -> Result<Option<RecoveryResultRow>>;
    fn list_by_evidence(&self, evidence_id: &EvidenceId) -> Result<Vec<RecoveryResultRow>>;
    fn count_by_evidence(&self, evidence_id: &EvidenceId) -> Result<u64>;
}

#[derive(Debug)]
pub struct SqliteRecoveryResultRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteRecoveryResultRepository<'a> {
    #[must_use]
    pub const fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl RecoveryResultRepository for SqliteRecoveryResultRepository<'_> {
    fn insert(&self, result: &NewRecoveryResult) -> Result<RecoveryResultId> {
        if result.source_length == 0 {
            return Err(kryvora_core::Error::InvalidInput(
                "source_length must be > 0".into(),
            ));
        }
        if result.artifact_sha256.len() != 64
            || !result
                .artifact_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(kryvora_core::Error::InvalidInput(
                "artifact_sha256 must be 64 hex characters".into(),
            ));
        }

        let id = match result.id {
            Some(id) => id,
            None => RecoveryResultId::new(),
        };
        let now = now_rfc3339();

        self.conn
            .execute(
                "INSERT INTO recovery_results (\
                     id, evidence_id, job_id, provenance_id, \
                     source_offset, source_length, detected_type, category, \
                     validation_state, confidence_level, confidence_reasons, \
                     recovery_method, reconstruction_state, \
                     artifact_sha256, artifact_path, validation_facts, created_at\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                rusqlite::params![
                    id.to_string(),
                    result.evidence_id.to_string(),
                    result.job_id.as_ref().map(std::string::ToString::to_string),
                    result
                        .provenance_id
                        .as_ref()
                        .map(std::string::ToString::to_string),
                    result.source_offset as i64,
                    result.source_length as i64,
                    result.detected_type,
                    result.category,
                    result.validation_state,
                    result.confidence_level,
                    result.confidence_reasons,
                    result.recovery_method,
                    result.reconstruction_state,
                    result.artifact_sha256.to_ascii_lowercase(),
                    result.artifact_path,
                    result.validation_facts,
                    now,
                ],
            )
            .map_err(|e| map_sql_err("insert recovery result", e))?;

        Ok(id)
    }

    fn get(&self, id: &RecoveryResultId) -> Result<Option<RecoveryResultRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, evidence_id, job_id, provenance_id, \
                        source_offset, source_length, detected_type, category, \
                        validation_state, confidence_level, confidence_reasons, \
                        recovery_method, reconstruction_state, \
                        artifact_sha256, artifact_path, validation_facts, created_at \
                 FROM recovery_results WHERE id = ?1",
            )
            .map_err(|e| map_sql_err("prepare get recovery result", e))?;

        let mut rows = stmt
            .query_map(rusqlite::params![id.to_string()], row_to_result)
            .map_err(|e| map_sql_err("query get recovery result", e))?;

        match rows.next() {
            None => Ok(None),
            Some(Ok(row)) => Ok(Some(row)),
            Some(Err(e)) => Err(map_sql_err("read recovery result row", e)),
        }
    }

    fn list_by_evidence(&self, evidence_id: &EvidenceId) -> Result<Vec<RecoveryResultRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, evidence_id, job_id, provenance_id, \
                        source_offset, source_length, detected_type, category, \
                        validation_state, confidence_level, confidence_reasons, \
                        recovery_method, reconstruction_state, \
                        artifact_sha256, artifact_path, validation_facts, created_at \
                 FROM recovery_results WHERE evidence_id = ?1 \
                 ORDER BY source_offset ASC, id ASC",
            )
            .map_err(|e| map_sql_err("prepare list recovery results", e))?;

        let rows = stmt
            .query_map(rusqlite::params![evidence_id.to_string()], row_to_result)
            .map_err(|e| map_sql_err("query list recovery results", e))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| map_sql_err("read recovery result row", e))?);
        }
        Ok(out)
    }

    fn count_by_evidence(&self, evidence_id: &EvidenceId) -> Result<u64> {
        let n: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM recovery_results WHERE evidence_id = ?1",
                rusqlite::params![evidence_id.to_string()],
                |r| r.get(0),
            )
            .map_err(|e| map_sql_err("count recovery results", e))?;
        Ok(n.max(0) as u64)
    }
}

fn row_to_result(row: &rusqlite::Row<'_>) -> rusqlite::Result<RecoveryResultRow> {
    let id_str: String = row.get(0)?;
    let ev_str: String = row.get(1)?;
    let job_str: Option<String> = row.get(2)?;
    let prov_str: Option<String> = row.get(3)?;

    let id = super::parse_stored_id(&id_str)
        .map(RecoveryResultId::from_uuid)
        .ok_or_else(|| conv_err(0, "recovery_results.id is not a valid UUID"))?;

    let evidence_id = super::parse_stored_id(&ev_str)
        .map(EvidenceId::from_uuid)
        .ok_or_else(|| conv_err(1, "recovery_results.evidence_id is not a valid UUID"))?;

    let job_id = match job_str.as_deref() {
        None => None,
        Some(s) => Some(
            super::parse_stored_id(s)
                .map(JobId::from_uuid)
                .ok_or_else(|| conv_err(2, "recovery_results.job_id is not a valid UUID"))?,
        ),
    };

    let provenance_id = match prov_str.as_deref() {
        None => None,
        Some(s) => Some(
            super::parse_stored_id(s)
                .map(ProvenanceId::from_uuid)
                .ok_or_else(|| conv_err(3, "recovery_results.provenance_id is not a valid UUID"))?,
        ),
    };

    Ok(RecoveryResultRow {
        id,
        evidence_id,
        job_id,
        provenance_id,
        source_offset: row.get::<_, i64>(4)? as u64,
        source_length: row.get::<_, i64>(5)? as u64,
        detected_type: row.get(6)?,
        category: row.get(7)?,
        validation_state: row.get(8)?,
        confidence_level: row.get(9)?,
        confidence_reasons: row.get(10)?,
        recovery_method: row.get(11)?,
        reconstruction_state: row.get(12)?,
        artifact_sha256: row.get(13)?,
        artifact_path: row.get(14)?,
        validation_facts: row.get(15)?,
        created_at: row.get(16)?,
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
    // Repository behavior is exercised in the crate-level integration
    // tests. This module contains no unit tests of its own.
}
