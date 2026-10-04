//! Compose a provenance chain for a recovery result and persist it.

use crate::node::NodeKind;
use kryvora_core::{CaseId, Error, EvidenceId, JobId, ProvenanceId, RecoveryResultId, Result};
use kryvora_db::repo::{
    NewProvenanceNode, NewRecoveryResult, ProvenanceRepository, RecoveryResultRepository,
    SqliteProvenanceRepository, SqliteRecoveryResultRepository,
};
use kryvora_recovery::RecoveryResult;
use rusqlite::Connection;

/// Ensure that an evidence item has a root node in its case provenance graph.
///
/// The same root is reused when called repeatedly, including after recovery
/// chains have already introduced it.
///
/// # Errors
///
/// Propagates repository errors.
pub fn ensure_evidence_root(
    conn: &Connection,
    case_id: &CaseId,
    evidence_id: &EvidenceId,
) -> Result<ProvenanceId> {
    let repo = SqliteProvenanceRepository::new(conn);
    let object_id = evidence_id.to_string();

    if let Some(existing) = repo
        .find_by_object(case_id, &object_id)?
        .into_iter()
        .find(|row| row.kind == kryvora_db::repo::ProvenanceKind::Evidence)
    {
        return Ok(existing.id);
    }

    repo.insert(&NewProvenanceNode {
        case_id: *case_id,
        kind: NodeKind::Evidence.to_db_kind(),
        object_id,
        parent_id: None,
        note: None,
    })
}

/// The inputs required to build a chain and persist a recovery result.
#[derive(Debug, Clone)]
pub struct ChainInputs {
    pub case_id: CaseId,
    pub evidence_id: EvidenceId,
    pub job_id: Option<JobId>,
    pub candidate_object_id: String,
    pub actor_note: Option<String>,
    pub artifact_path: Option<String>,
}

/// Persist a [`RecoveryResult`] and its provenance chain.
///
/// The chain is:
///
/// ```text
/// evidence (root, if not already present)
///   -> candidate
///     -> artifact  (the recovery result)
/// ```
///
/// If a root node for `evidence_id` already exists in the case, it is
/// reused; otherwise one is created. The candidate node is always
/// created fresh, because a candidate is a per-run entity. The
/// artifact node is created fresh and linked to the candidate.
///
/// Returns the new provenance id for the artifact node.
///
/// # Errors
///
/// * [`Error::InvalidInput`] — the recovery result's `artifact_sha256`
///   is not a 64-char hex string.
/// * Any repository error.
pub fn persist_recovery_result(
    conn: &Connection,
    inputs: &ChainInputs,
    result: &RecoveryResult,
) -> Result<ProvenanceId> {
    const SAVEPOINT: &str = "kryvora_recovery_provenance";
    let owns_transaction = conn.is_autocommit();
    if owns_transaction {
        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(|e| Error::Database(format!("begin recovery persistence: {e}")))?;
    } else {
        conn.execute_batch(&format!("SAVEPOINT {SAVEPOINT}"))
            .map_err(|e| Error::Database(format!("begin recovery savepoint: {e}")))?;
    }

    let persisted = (|| -> Result<ProvenanceId> {
        let prov_repo = SqliteProvenanceRepository::new(conn);
        let result_repo = SqliteRecoveryResultRepository::new(conn);
        let evidence_root = ensure_evidence_root(conn, &inputs.case_id, &inputs.evidence_id)?;

        let candidate_node = prov_repo.insert(&NewProvenanceNode {
            case_id: inputs.case_id,
            kind: NodeKind::Candidate.to_db_kind(),
            object_id: inputs.candidate_object_id.clone(),
            parent_id: Some(evidence_root),
            note: None,
        })?;

        let artifact_node = prov_repo.insert(&NewProvenanceNode {
            case_id: inputs.case_id,
            kind: NodeKind::Artifact.to_db_kind(),
            object_id: result.id.to_string(),
            parent_id: Some(candidate_node),
            note: inputs.actor_note.clone(),
        })?;

        let confidence_reasons = serde_json::to_string(&result.confidence)
            .map_err(|e| Error::Internal(format!("confidence serialize: {e}")))?;
        let validation_facts = serde_json::to_string(&result.validation_facts)
            .map_err(|e| Error::Internal(format!("validation facts serialize: {e}")))?;

        result_repo.insert(&NewRecoveryResult {
            id: Some(result.id),
            evidence_id: inputs.evidence_id,
            job_id: inputs.job_id,
            provenance_id: Some(artifact_node),
            source_offset: result.source_offset,
            source_length: result.source_length,
            detected_type: result.detected_type.clone(),
            category: result.category.as_str().to_string(),
            validation_state: format!("{:?}", result.validation_state).to_lowercase(),
            confidence_level: format!("{:?}", result.confidence.level).to_lowercase(),
            confidence_reasons,
            recovery_method: result.recovery_method.as_str().to_string(),
            reconstruction_state: format!("{:?}", result.reconstruction_state).to_lowercase(),
            artifact_sha256: result.artifact_sha256.clone(),
            artifact_path: inputs.artifact_path.clone(),
            validation_facts,
        })?;

        Ok(artifact_node)
    })();

    match persisted {
        Ok(artifact_node) => {
            if owns_transaction {
                conn.execute_batch("COMMIT")
                    .map_err(|e| Error::Database(format!("commit recovery persistence: {e}")))?;
            } else {
                conn.execute_batch(&format!("RELEASE SAVEPOINT {SAVEPOINT}"))
                    .map_err(|e| Error::Database(format!("release recovery savepoint: {e}")))?;
            }
            Ok(artifact_node)
        }
        Err(error) => {
            if owns_transaction {
                let _ = conn.execute_batch("ROLLBACK");
            } else {
                let _ = conn.execute_batch(&format!(
                    "ROLLBACK TO SAVEPOINT {SAVEPOINT}; RELEASE SAVEPOINT {SAVEPOINT}"
                ));
            }
            Err(error)
        }
    }
}

/// Convenience: build the object id for a candidate node from its
/// offset and length. This is the canonical string form used by the
/// provenance layer for carved candidates that are not represented by
/// a `FragmentId` in the database.
#[must_use]
pub fn candidate_object_id(offset: u64, length: u64) -> String {
    format!("CANDIDATE-{offset}-{length}")
}

/// Convenience: return the [`RecoveryResultId`] as a string, for use in
/// callers that build the artifact node themselves.
#[must_use]
pub fn artifact_object_id(id: &RecoveryResultId) -> String {
    id.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_object_id_is_stable() {
        assert_eq!(candidate_object_id(100, 200), "CANDIDATE-100-200");
    }
}
