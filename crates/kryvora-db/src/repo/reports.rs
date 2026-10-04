//! Repository for the `reports` table.

use super::{map_sql_err, now_rfc3339};
use kryvora_core::{CaseId, Error, JobId, ReportId, Result};
use rusqlite::Connection;

/// The kind of report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportKind {
    Recovery,
    Sanitization,
}

impl ReportKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recovery => "recovery",
            Self::Sanitization => "sanitization",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "recovery" => Some(Self::Recovery),
            "sanitization" => Some(Self::Sanitization),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewReport {
    pub case_id: Option<CaseId>,
    pub job_id: Option<JobId>,
    pub kind: ReportKind,
    pub path: String,
    pub sha256: String,
    pub byte_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportRow {
    pub id: ReportId,
    pub case_id: Option<CaseId>,
    pub job_id: Option<JobId>,
    pub kind: ReportKind,
    pub path: String,
    pub sha256: String,
    pub byte_size: u64,
    pub created_at: String,
}

pub trait ReportRepository {
    fn insert(&self, report: &NewReport) -> Result<ReportId>;
    fn get(&self, id: &ReportId) -> Result<Option<ReportRow>>;
    fn list_by_case(&self, case_id: &CaseId) -> Result<Vec<ReportRow>>;
}

#[derive(Debug)]
pub struct SqliteReportRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteReportRepository<'a> {
    #[must_use]
    pub const fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl ReportRepository for SqliteReportRepository<'_> {
    fn insert(&self, report: &NewReport) -> Result<ReportId> {
        if report.path.trim().is_empty() {
            return Err(Error::InvalidInput("report path must not be empty".into()));
        }
        if report.sha256.len() != 64 || !report.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::InvalidInput(
                "report sha256 must be 64 hex characters".into(),
            ));
        }

        let id = ReportId::new();
        let now = now_rfc3339();

        self.conn
            .execute(
                "INSERT INTO reports (id, case_id, job_id, kind, path, sha256, byte_size, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    id.to_string(),
                    report.case_id.as_ref().map(std::string::ToString::to_string),
                    report.job_id.as_ref().map(std::string::ToString::to_string),
                    report.kind.as_str(),
                    report.path,
                    report.sha256.to_ascii_lowercase(),
                    report.byte_size as i64,
                    now,
                ],
            )
            .map_err(|e| map_sql_err("insert report", e))?;

        Ok(id)
    }

    fn get(&self, id: &ReportId) -> Result<Option<ReportRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, case_id, job_id, kind, path, sha256, byte_size, created_at \
                 FROM reports WHERE id = ?1",
            )
            .map_err(|e| map_sql_err("prepare get report", e))?;

        let mut rows = stmt
            .query_map(rusqlite::params![id.to_string()], row_to_report)
            .map_err(|e| map_sql_err("query get report", e))?;

        match rows.next() {
            None => Ok(None),
            Some(Ok(row)) => Ok(Some(row)),
            Some(Err(e)) => Err(map_sql_err("read report row", e)),
        }
    }

    fn list_by_case(&self, case_id: &CaseId) -> Result<Vec<ReportRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, case_id, job_id, kind, path, sha256, byte_size, created_at \
                 FROM reports WHERE case_id = ?1 ORDER BY created_at DESC, id DESC",
            )
            .map_err(|e| map_sql_err("prepare list reports", e))?;

        let rows = stmt
            .query_map(rusqlite::params![case_id.to_string()], row_to_report)
            .map_err(|e| map_sql_err("query list reports", e))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| map_sql_err("read report row", e))?);
        }
        Ok(out)
    }
}

fn row_to_report(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReportRow> {
    let id_str: String = row.get(0)?;
    let case_str: Option<String> = row.get(1)?;
    let job_str: Option<String> = row.get(2)?;
    let kind_str: String = row.get(3)?;

    let id = super::parse_stored_id(&id_str)
        .map(ReportId::from_uuid)
        .ok_or_else(|| conv_err(0, "reports.id is not a valid UUID"))?;

    let case_id = match case_str.as_deref() {
        None => None,
        Some(s) => Some(
            super::parse_stored_id(s)
                .map(CaseId::from_uuid)
                .ok_or_else(|| conv_err(1, "reports.case_id is not a valid UUID"))?,
        ),
    };

    let job_id = match job_str.as_deref() {
        None => None,
        Some(s) => Some(
            super::parse_stored_id(s)
                .map(JobId::from_uuid)
                .ok_or_else(|| conv_err(2, "reports.job_id is not a valid UUID"))?,
        ),
    };

    let kind = ReportKind::parse(&kind_str)
        .ok_or_else(|| conv_err(3, "reports.kind is not a known kind"))?;

    Ok(ReportRow {
        id,
        case_id,
        job_id,
        kind,
        path: row.get(4)?,
        sha256: row.get(5)?,
        byte_size: row.get::<_, i64>(6)? as u64,
        created_at: row.get(7)?,
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
    fn report_kind_roundtrip() {
        for k in [ReportKind::Recovery, ReportKind::Sanitization] {
            assert_eq!(ReportKind::parse(k.as_str()), Some(k));
        }
    }

    #[test]
    fn report_kind_rejects_unknown() {
        assert_eq!(ReportKind::parse("bogus"), None);
    }
}
