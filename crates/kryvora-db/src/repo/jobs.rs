//! Repository for the `jobs` table.
//!
//! Jobs are mutable: state, progress, checkpoint, and error message all
//! change during their lifetime. `created_at` is set on insert and never
//! changes; `started_at` and `completed_at` are set when the job
//! transitions.

use super::{map_sql_err, now_rfc3339};
use kryvora_core::{CaseId, Error, EvidenceId, JobId, Result};
use rusqlite::Connection;

#[derive(Debug, Clone)]
pub struct NewJob {
    pub case_id: Option<CaseId>,
    pub evidence_id: Option<EvidenceId>,
    pub job_type: String,
    pub configuration: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobRow {
    pub id: JobId,
    pub case_id: Option<CaseId>,
    pub evidence_id: Option<EvidenceId>,
    pub job_type: String,
    pub state: String,
    pub progress: f64,
    pub configuration: Option<String>,
    pub checkpoint: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

pub trait JobRepository {
    fn insert(&self, new_job: &NewJob) -> Result<JobId>;
    fn get(&self, id: &JobId) -> Result<Option<JobRow>>;
    fn list_by_case(&self, case_id: &CaseId) -> Result<Vec<JobRow>>;
    fn list_by_state(&self, state: &str, limit: u32) -> Result<Vec<JobRow>>;

    /// Transition to a new state. If the new state is a "started" state
    /// (`running`, `verifying`) and `started_at` is NULL, it is set now.
    /// If the new state is terminal, `completed_at` is set now.
    fn update_state(&self, id: &JobId, state: &str, error_message: Option<&str>) -> Result<()>;

    /// Update progress in 0.0..=1.0. Rejects values outside that range.
    fn update_progress(&self, id: &JobId, progress: f64) -> Result<()>;

    /// Update the opaque checkpoint blob.
    fn update_checkpoint(&self, id: &JobId, checkpoint: Option<&str>) -> Result<()>;
}

#[derive(Debug)]
pub struct SqliteJobRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteJobRepository<'a> {
    #[must_use]
    pub const fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

const VALID_STATES: &[&str] = &[
    "created",
    "queued",
    "running",
    "verifying",
    "succeeded",
    "failed",
    "partial",
    "cancelled",
    "inconclusive",
];

fn is_terminal_state(state: &str) -> bool {
    matches!(
        state,
        "succeeded" | "failed" | "partial" | "cancelled" | "inconclusive"
    )
}

fn is_started_state(state: &str) -> bool {
    matches!(state, "running" | "verifying")
}

fn check_state(state: &str) -> Result<()> {
    if !VALID_STATES.contains(&state) {
        return Err(Error::InvalidInput(format!("unknown job state: {state}")));
    }
    Ok(())
}

impl JobRepository for SqliteJobRepository<'_> {
    fn insert(&self, new_job: &NewJob) -> Result<JobId> {
        if new_job.job_type.trim().is_empty() {
            return Err(Error::InvalidInput("job_type must not be empty".into()));
        }

        let id = JobId::new();
        let now = now_rfc3339();

        self.conn
            .execute(
                "INSERT INTO jobs (\
                     id, case_id, evidence_id, job_type, state, progress, \
                     configuration, checkpoint, error_message, \
                     created_at, started_at, completed_at\
                 ) VALUES (?1, ?2, ?3, ?4, 'created', 0.0, ?5, NULL, NULL, ?6, NULL, NULL)",
                rusqlite::params![
                    id.to_string(),
                    new_job
                        .case_id
                        .as_ref()
                        .map(std::string::ToString::to_string),
                    new_job
                        .evidence_id
                        .as_ref()
                        .map(std::string::ToString::to_string),
                    new_job.job_type,
                    new_job.configuration,
                    now,
                ],
            )
            .map_err(|e| map_sql_err("insert job", e))?;

        Ok(id)
    }

    fn get(&self, id: &JobId) -> Result<Option<JobRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, case_id, evidence_id, job_type, state, progress, \
                        configuration, checkpoint, error_message, \
                        created_at, started_at, completed_at \
                 FROM jobs WHERE id = ?1",
            )
            .map_err(|e| map_sql_err("prepare get job", e))?;

        let mut rows = stmt
            .query_map(rusqlite::params![id.to_string()], row_to_job)
            .map_err(|e| map_sql_err("query get job", e))?;

        match rows.next() {
            None => Ok(None),
            Some(Ok(row)) => Ok(Some(row)),
            Some(Err(e)) => Err(map_sql_err("read job row", e)),
        }
    }

    fn list_by_case(&self, case_id: &CaseId) -> Result<Vec<JobRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, case_id, evidence_id, job_type, state, progress, \
                        configuration, checkpoint, error_message, \
                        created_at, started_at, completed_at \
                 FROM jobs WHERE case_id = ?1 ORDER BY created_at DESC, id DESC",
            )
            .map_err(|e| map_sql_err("prepare list jobs by case", e))?;

        let rows = stmt
            .query_map(rusqlite::params![case_id.to_string()], row_to_job)
            .map_err(|e| map_sql_err("query list jobs by case", e))?;

        collect_rows(rows, "list jobs by case")
    }

    fn list_by_state(&self, state: &str, limit: u32) -> Result<Vec<JobRow>> {
        check_state(state)?;
        if limit == 0 {
            return Err(Error::InvalidInput("list limit must be > 0".into()));
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, case_id, evidence_id, job_type, state, progress, \
                        configuration, checkpoint, error_message, \
                        created_at, started_at, completed_at \
                 FROM jobs WHERE state = ?1 ORDER BY created_at ASC, id ASC LIMIT ?2",
            )
            .map_err(|e| map_sql_err("prepare list jobs by state", e))?;

        let rows = stmt
            .query_map(rusqlite::params![state, limit], row_to_job)
            .map_err(|e| map_sql_err("query list jobs by state", e))?;

        collect_rows(rows, "list jobs by state")
    }

    fn update_state(&self, id: &JobId, state: &str, error_message: Option<&str>) -> Result<()> {
        check_state(state)?;

        let now = now_rfc3339();

        let sql = if is_started_state(state) {
            "UPDATE jobs SET state = ?1, error_message = ?2, \
                              started_at = COALESCE(started_at, ?3) \
             WHERE id = ?4"
        } else if is_terminal_state(state) {
            "UPDATE jobs SET state = ?1, error_message = ?2, \
                              started_at = COALESCE(started_at, ?3), \
                              completed_at = ?3 \
             WHERE id = ?4"
        } else {
            "UPDATE jobs SET state = ?1, error_message = ?2 WHERE id = ?3"
        };

        let changed = if sql.contains("?4") {
            self.conn
                .execute(
                    sql,
                    rusqlite::params![state, error_message, now, id.to_string()],
                )
                .map_err(|e| map_sql_err("update job state", e))?
        } else {
            self.conn
                .execute(sql, rusqlite::params![state, error_message, id.to_string()])
                .map_err(|e| map_sql_err("update job state", e))?
        };

        if changed == 0 {
            return Err(Error::NotFound(format!("job {id}")));
        }
        Ok(())
    }

    fn update_progress(&self, id: &JobId, progress: f64) -> Result<()> {
        if !(0.0..=1.0).contains(&progress) || progress.is_nan() {
            return Err(Error::InvalidInput(format!(
                "progress must be in 0.0..=1.0, got {progress}"
            )));
        }

        let changed = self
            .conn
            .execute(
                "UPDATE jobs SET progress = ?1 WHERE id = ?2",
                rusqlite::params![progress, id.to_string()],
            )
            .map_err(|e| map_sql_err("update job progress", e))?;

        if changed == 0 {
            return Err(Error::NotFound(format!("job {id}")));
        }
        Ok(())
    }

    fn update_checkpoint(&self, id: &JobId, checkpoint: Option<&str>) -> Result<()> {
        let changed = self
            .conn
            .execute(
                "UPDATE jobs SET checkpoint = ?1 WHERE id = ?2",
                rusqlite::params![checkpoint, id.to_string()],
            )
            .map_err(|e| map_sql_err("update job checkpoint", e))?;

        if changed == 0 {
            return Err(Error::NotFound(format!("job {id}")));
        }
        Ok(())
    }
}

fn row_to_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<JobRow> {
    let id_str: String = row.get(0)?;
    let case_str: Option<String> = row.get(1)?;
    let evid_str: Option<String> = row.get(2)?;

    let id = super::parse_stored_id(&id_str)
        .map(JobId::from_uuid)
        .ok_or_else(|| conv_err(0, "jobs.id is not a valid UUID"))?;

    let case_id = match case_str.as_deref() {
        None => None,
        Some(s) => Some(
            super::parse_stored_id(s)
                .map(CaseId::from_uuid)
                .ok_or_else(|| conv_err(1, "jobs.case_id is not a valid UUID"))?,
        ),
    };

    let evidence_id = match evid_str.as_deref() {
        None => None,
        Some(s) => Some(
            super::parse_stored_id(s)
                .map(EvidenceId::from_uuid)
                .ok_or_else(|| conv_err(2, "jobs.evidence_id is not a valid UUID"))?,
        ),
    };

    Ok(JobRow {
        id,
        case_id,
        evidence_id,
        job_type: row.get(3)?,
        state: row.get(4)?,
        progress: row.get(5)?,
        configuration: row.get(6)?,
        checkpoint: row.get(7)?,
        error_message: row.get(8)?,
        created_at: row.get(9)?,
        started_at: row.get(10)?,
        completed_at: row.get(11)?,
    })
}

fn collect_rows<F>(rows: rusqlite::MappedRows<'_, F>, context: &str) -> Result<Vec<JobRow>>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<JobRow>,
{
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| map_sql_err(context, e))?);
    }
    Ok(out)
}

fn conv_err(idx: usize, msg: &'static str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        idx,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, msg)),
    )
}
