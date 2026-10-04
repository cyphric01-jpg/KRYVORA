//! `kryvora verify` — walk and report the audit chain of an existing
//! database.

use crate::output;
use kryvora_audit::{count, verify_chain, ChainStatus};
use kryvora_core::Error;
use kryvora_db::open;
use std::path::Path;

/// Open the database, verify the audit chain, and print the result.
///
/// # Errors
///
/// * [`Error::NotFound`] — the database file does not exist.
/// * Any error from opening the database or reading the chain.
pub fn execute(db_path: &Path) -> kryvora_core::Result<()> {
    if !db_path.is_file() {
        return Err(Error::NotFound(format!(
            "database does not exist: {}",
            db_path.display()
        )));
    }

    let conn = open(db_path)?;
    let chain_status = verify_chain(&conn)?;
    let total = count(&conn)?;

    output::print_verify_report(db_path, total, &chain_status);

    // A broken chain is a finding, not an error. Return Ok so the
    // caller can script on stdout; the process exit code is still
    // success because verification completed without infrastructure
    // failure.
    let _ = matches!(chain_status, ChainStatus::Intact { .. });
    Ok(())
}
