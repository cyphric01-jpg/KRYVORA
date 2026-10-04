//! Repository for planned and completed destructive sanitization operations.

use super::{map_sql_err, now_rfc3339, parse_stored_id};
use kryvora_core::{CaseId, Error, Result, SanitizationOperationId, SanitizationOperationState};
use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone)]
pub struct NewSanitizationOperation {
    pub case_id: Option<CaseId>,
    pub target_path: String,
    pub target_kind: String,
    pub method: String,
    pub target_identity_verified: bool,
    pub confirmation_kind: String,
    pub confirmation_validated: bool,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizationOperationRow {
    pub id: SanitizationOperationId,
    pub case_id: Option<CaseId>,
    pub target_path: String,
    pub target_kind: String,
    pub method: String,
    pub target_identity_verified: bool,
    pub confirmation_kind: String,
    pub confirmation_validated: bool,
    pub state: SanitizationOperationState,
    pub outcome: Option<String>,
    pub result_json: Option<String>,
    pub error_message: Option<String>,
    pub actor: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

pub trait SanitizationOperationRepository {
    fn insert(&self, operation: &NewSanitizationOperation) -> Result<SanitizationOperationId>;
    fn get(&self, id: &SanitizationOperationId) -> Result<Option<SanitizationOperationRow>>;
    fn list_recent(&self, limit: u32) -> Result<Vec<SanitizationOperationRow>>;
    fn transition(
        &self,
        id: &SanitizationOperationId,
        next: SanitizationOperationState,
        outcome: Option<&str>,
        result_json: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<()>;
}

#[derive(Debug)]
pub struct SqliteSanitizationOperationRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteSanitizationOperationRepository<'a> {
    #[must_use]
    pub const fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl SanitizationOperationRepository for SqliteSanitizationOperationRepository<'_> {
    fn insert(&self, operation: &NewSanitizationOperation) -> Result<SanitizationOperationId> {
        if operation.target_path.trim().is_empty() {
            return Err(Error::InvalidInput("sanitization target path is empty".into()));
        }
        if !matches!(operation.target_kind.as_str(), "file" | "directory") {
            return Err(Error::InvalidInput("invalid sanitization target kind".into()));
        }
        if operation.method != "random_overwrite" {
            return Err(Error::InvalidInput("unsupported sanitization method".into()));
        }
        if !matches!(operation.confirmation_kind.as_str(), "basic" | "typed_phrase") {
            return Err(Error::InvalidInput("invalid sanitization confirmation kind".into()));
        }

        let id = SanitizationOperationId::new();
        let created_at = now_rfc3339();
        self.conn
            .execute(
                "INSERT INTO sanitization_operations (
                    id, case_id, target_path, target_kind, method,
                    target_identity_verified, confirmation_kind, confirmation_validated,
                    state, outcome, result_json, error_message, actor,
                    created_at, started_at, completed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'planned', NULL, NULL, NULL, ?9, ?10, NULL, NULL)",
                rusqlite::params![
                    id.to_string(),
                    operation.case_id.map(|case_id| case_id.to_string()),
                    operation.target_path,
                    operation.target_kind,
                    operation.method,
                    operation.target_identity_verified,
                    operation.confirmation_kind,
                    operation.confirmation_validated,
                    operation.actor,
                    created_at,
                ],
            )
            .map_err(|error| map_sql_err("insert sanitization operation", error))?;
        Ok(id)
    }

    fn get(&self, id: &SanitizationOperationId) -> Result<Option<SanitizationOperationRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, case_id, target_path, target_kind, method,
                        target_identity_verified, confirmation_kind, confirmation_validated,
                        state, outcome, result_json, error_message, actor,
                        created_at, started_at, completed_at
                 FROM sanitization_operations WHERE id = ?1",
                rusqlite::params![id.to_string()],
                row_to_operation,
            )
            .optional()
            .map_err(|error| map_sql_err("get sanitization operation", error))?)
    }

    fn list_recent(&self, limit: u32) -> Result<Vec<SanitizationOperationRow>> {
        if limit == 0 {
            return Err(Error::InvalidInput("list limit must be > 0".into()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT id, case_id, target_path, target_kind, method,
                        target_identity_verified, confirmation_kind, confirmation_validated,
                        state, outcome, result_json, error_message, actor,
                        created_at, started_at, completed_at
                 FROM sanitization_operations ORDER BY created_at DESC, id DESC LIMIT ?1",
            )
            .map_err(|error| map_sql_err("prepare list sanitization operations", error))?;
        let rows = statement
            .query_map(rusqlite::params![limit], row_to_operation)
            .map_err(|error| map_sql_err("query sanitization operations", error))?;
        let mut operations = Vec::new();
        for row in rows {
            operations.push(
                row.map_err(|error| map_sql_err("read sanitization operation", error))?,
            );
        }
        Ok(operations)
    }

    fn transition(
        &self,
        id: &SanitizationOperationId,
        next: SanitizationOperationState,
        outcome: Option<&str>,
        result_json: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<()> {
        let current = self
            .get(id)?
            .ok_or_else(|| Error::NotFound(format!("sanitization operation {id}")))?;
        let valid = matches!(
            (current.state, next),
            (SanitizationOperationState::Planned, SanitizationOperationState::Running)
                | (SanitizationOperationState::Planned, SanitizationOperationState::Refused)
                | (SanitizationOperationState::Running, SanitizationOperationState::Completed)
                | (SanitizationOperationState::Running, SanitizationOperationState::Failed)
        );
        if !valid {
            return Err(Error::InvalidInput(format!(
                "invalid sanitization operation transition: {} -> {}",
                current.state.as_str(),
                next.as_str()
            )));
        }
        if let Some(value) = outcome {
            if !matches!(value, "success" | "partial" | "failed" | "not_verified" | "unsupported") {
                return Err(Error::InvalidInput("invalid sanitization outcome".into()));
            }
        }

        let now = now_rfc3339();
        let started_at = (next == SanitizationOperationState::Running).then_some(now.as_str());
        let completed_at = matches!(
            next,
            SanitizationOperationState::Completed
                | SanitizationOperationState::Failed
                | SanitizationOperationState::Refused
        )
        .then_some(now.as_str());
        let changed = self
            .conn
            .execute(
                "UPDATE sanitization_operations
                 SET state = ?1, outcome = ?2, result_json = ?3, error_message = ?4,
                     started_at = COALESCE(started_at, ?5),
                     completed_at = COALESCE(?6, completed_at),
                     target_identity_verified = CASE WHEN ?1 = 'running' THEN 1 ELSE target_identity_verified END,
                     confirmation_validated = CASE WHEN ?1 = 'running' THEN 1 ELSE confirmation_validated END
                 WHERE id = ?7 AND state = ?8",
                rusqlite::params![
                    next.as_str(), outcome, result_json, error_message,
                    started_at, completed_at, id.to_string(), current.state.as_str()
                ],
            )
            .map_err(|error| map_sql_err("transition sanitization operation", error))?;
        if changed != 1 {
            return Err(Error::Database(
                "sanitization operation changed during transition".into(),
            ));
        }
        Ok(())
    }
}

fn row_to_operation(row: &rusqlite::Row<'_>) -> rusqlite::Result<SanitizationOperationRow> {
    let id_text: String = row.get(0)?;
    let case_text: Option<String> = row.get(1)?;
    let state_text: String = row.get(8)?;
    let id = parse_stored_id(&id_text)
        .map(SanitizationOperationId::from_uuid)
        .ok_or_else(|| conversion_error(0, "sanitization operation id is invalid"))?;
    let case_id = match case_text.as_deref() {
        None => None,
        Some(value) => Some(
            parse_stored_id(value)
                .map(CaseId::from_uuid)
                .ok_or_else(|| conversion_error(1, "sanitization operation case id is invalid"))?,
        ),
    };
    let state = SanitizationOperationState::parse(&state_text)
        .ok_or_else(|| conversion_error(8, "sanitization operation state is invalid"))?;

    Ok(SanitizationOperationRow {
        id,
        case_id,
        target_path: row.get(2)?,
        target_kind: row.get(3)?,
        method: row.get(4)?,
        target_identity_verified: row.get(5)?,
        confirmation_kind: row.get(6)?,
        confirmation_validated: row.get(7)?,
        state,
        outcome: row.get(9)?,
        result_json: row.get(10)?,
        error_message: row.get(11)?,
        actor: row.get(12)?,
        created_at: row.get(13)?,
        started_at: row.get(14)?,
        completed_at: row.get(15)?,
    })
}

fn conversion_error(index: usize, message: &'static str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, message)),
    )
}