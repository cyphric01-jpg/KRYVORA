//! Repository for the `cases` table.

use super::{map_sql_err, now_rfc3339};
use kryvora_core::{CaseId, Error, Result};
use rusqlite::Connection;

/// Fields a caller must supply to insert a case.
///
/// `examiner` and `notes` are optional; `title` is required and must be
/// non-empty. `created_at` and `updated_at` are set by the repository.
#[derive(Debug, Clone)]
pub struct NewCase {
    pub title: String,
    pub examiner: Option<String>,
    pub notes: Option<String>,
}

/// A row from the `cases` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseRow {
    pub id: CaseId,
    pub title: String,
    pub examiner: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Contract for case persistence.
pub trait CaseRepository {
    /// Insert a new case and return its freshly generated identifier.
    fn insert(&self, new_case: &NewCase) -> Result<CaseId>;

    /// Fetch a case by id.
    fn get(&self, id: &CaseId) -> Result<Option<CaseRow>>;

    /// List cases, newest first. `limit` must be > 0.
    fn list(&self, limit: u32, offset: u32) -> Result<Vec<CaseRow>>;

    /// Update title and notes together. `title` must be non-empty.
    fn update_title_and_notes(&self, id: &CaseId, title: &str, notes: Option<&str>) -> Result<()>;
}

/// `rusqlite`-backed implementation. Borrows the connection; the caller
/// owns its lifetime.
#[derive(Debug)]
pub struct SqliteCaseRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteCaseRepository<'a> {
    #[must_use]
    pub const fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl CaseRepository for SqliteCaseRepository<'_> {
    fn insert(&self, new_case: &NewCase) -> Result<CaseId> {
        if new_case.title.trim().is_empty() {
            return Err(Error::InvalidInput("case title must not be empty".into()));
        }

        let id = CaseId::new();
        let now = now_rfc3339();

        self.conn
            .execute(
                "INSERT INTO cases (id, title, examiner, notes, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                rusqlite::params![
                    id.to_string(),
                    new_case.title,
                    new_case.examiner,
                    new_case.notes,
                    now,
                ],
            )
            .map_err(|e| map_sql_err("insert case", e))?;

        Ok(id)
    }

    fn get(&self, id: &CaseId) -> Result<Option<CaseRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, examiner, notes, created_at, updated_at \
                 FROM cases WHERE id = ?1",
            )
            .map_err(|e| map_sql_err("prepare get case", e))?;

        let mut rows = stmt
            .query_map(rusqlite::params![id.to_string()], row_to_case)
            .map_err(|e| map_sql_err("query get case", e))?;

        match rows.next() {
            None => Ok(None),
            Some(Ok(row)) => Ok(Some(row)),
            Some(Err(e)) => Err(map_sql_err("read case row", e)),
        }
    }

    fn list(&self, limit: u32, offset: u32) -> Result<Vec<CaseRow>> {
        if limit == 0 {
            return Err(Error::InvalidInput("list limit must be > 0".into()));
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, examiner, notes, created_at, updated_at \
                 FROM cases ORDER BY created_at DESC, id DESC \
                 LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| map_sql_err("prepare list cases", e))?;

        let rows = stmt
            .query_map(rusqlite::params![limit, offset], row_to_case)
            .map_err(|e| map_sql_err("query list cases", e))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| map_sql_err("read case row", e))?);
        }
        Ok(out)
    }

    fn update_title_and_notes(&self, id: &CaseId, title: &str, notes: Option<&str>) -> Result<()> {
        if title.trim().is_empty() {
            return Err(Error::InvalidInput("case title must not be empty".into()));
        }

        let now = now_rfc3339();
        let changed = self
            .conn
            .execute(
                "UPDATE cases SET title = ?1, notes = ?2, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![title, notes, now, id.to_string()],
            )
            .map_err(|e| map_sql_err("update case", e))?;

        if changed == 0 {
            return Err(Error::NotFound(format!("case {id}")));
        }
        Ok(())
    }
}

fn row_to_case(row: &rusqlite::Row<'_>) -> rusqlite::Result<CaseRow> {
    let id_str: String = row.get(0)?;
    let id = uuid_parse_case(&id_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "cases.id is not a valid UUID",
            )),
        )
    })?;

    Ok(CaseRow {
        id,
        title: row.get(1)?,
        examiner: row.get(2)?,
        notes: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

/// Parse a UUID string into a [`CaseId`]. Handles both the KRYVORA
/// prefixed storage form (`CASE-<32 hex>`) and a bare canonical UUID.
/// Returns `None` on malformed input.
fn uuid_parse_case(s: &str) -> Option<CaseId> {
    super::parse_stored_id(s).map(CaseId::from_uuid)
}
