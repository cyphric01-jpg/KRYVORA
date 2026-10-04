// file: crates/kryvora-cli/src/commands/recover.rs
//! `kryvora recover` — re-run recovery on candidates stored for an
//! evidence id, using an existing case.

use kryvora_carving::{scan_source, ScanOptions};
use kryvora_core::Error;
use kryvora_db::repo::{EvidenceRepository, SqliteEvidenceRepository};
use kryvora_db::{apply_migrations, open};
use kryvora_recovery::run_recovery_job;
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;

/// Execute a standalone recovery pass: carve the source fresh, then
/// recover. This exists as a separate entry point for cases where the
/// source is available but the carve was not persisted.
///
/// # Errors
///
/// Same as [`crate::commands::carve::execute`].
pub fn execute(
    db_path: &Path,
    source: &Path,
    evidence_id_str: &str,
    examiner: Option<&str>,
) -> kryvora_core::Result<()> {
    if !source.is_file() {
        return Err(Error::InvalidInput(format!(
            "source is not a regular file: {}",
            source.display()
        )));
    }

    let mut conn = open(db_path)?;
    apply_migrations(&mut conn)?;

    // Parse the evidence id from the string. Accept the prefixed form
    // (EVID-...) or a bare UUID.
    let uuid = parse_evidence_id(evidence_id_str).ok_or_else(|| {
        Error::InvalidInput(format!(
            "evidence id is not a valid KRYVORA evidence id: {evidence_id_str}"
        ))
    })?;
    let evidence_id = kryvora_core::EvidenceId::from_uuid(uuid);

    // Confirm the evidence exists.
    let ev_repo = SqliteEvidenceRepository::new(&conn);
    let _ = ev_repo
        .get(&evidence_id)?
        .ok_or_else(|| Error::NotFound(format!("evidence {evidence_id}")))?;

    // Carve.
    let carve_file = File::open(source).map_err(Error::Io)?;
    let carve_reader = BufReader::new(carve_file);
    let carve_report = scan_source(carve_reader, &ScanOptions::default())?;

    println!("=== KRYVORA recover ===");
    println!("evidence:            {evidence_id}");
    println!("candidates found:    {}", carve_report.candidates_found);

    if carve_report.candidates.is_empty() {
        println!("No candidates found. Nothing to recover.");
        return Ok(());
    }

    // Recover.
    let recovery_reader = Cursor::new(std::fs::read(source).map_err(Error::Io)?);
    let candidates = carve_report.candidates;

    let report = match run_recovery_job(
        &conn,
        recovery_reader,
        candidates,
        Some(evidence_id),
        examiner.map(str::to_string),
    )? {
        Some(r) => r,
        None => return Err(Error::JobCancelled),
    };

    println!("candidates validated: {}", report.candidates_validated);
    println!("candidates rejected:  {}", report.candidates_rejected);
    println!();

    Ok(())
}

fn parse_evidence_id(s: &str) -> Option<uuid::Uuid> {
    if let Ok(u) = uuid::Uuid::parse_str(s) {
        return Some(u);
    }
    let (_, tail) = s.rsplit_once('-')?;
    if tail.len() == 32 {
        uuid::Uuid::parse_str(tail).ok()
    } else {
        None
    }
}
