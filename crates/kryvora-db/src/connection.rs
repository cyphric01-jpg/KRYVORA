//! Opening and configuring a KRYVORA SQLite connection.

use kryvora_core::{Error, Result};
use rusqlite::Connection;
use std::path::Path;

/// Open a SQLite database at `path` and apply the standard KRYVORA
/// connection pragmas.
///
/// Pragmas applied on every open:
///
/// * `foreign_keys = ON` — SQLite disables FK enforcement by default.
///   KRYVORA never relies on this default.
/// * `journal_mode = WAL` — better crash resilience and read/write
///   concurrency. WAL is the correct mode for an application that will
///   eventually have concurrent readers.
/// * `synchronous = NORMAL` — a deliberate trade-off. `FULL` is safer
///   against power loss but costs throughput; `NORMAL` under WAL is
///   safe against application crashes. Documented as a limitation.
/// * `busy_timeout = 5000` — wait up to 5s on lock contention before
///   returning SQLITE_BUSY. Prevents spurious failures in tests.
///
/// # Errors
///
/// Returns [`Error::Database`] if the file cannot be opened or any
/// pragma fails.
pub fn open(path: impl AsRef<Path>) -> Result<Connection> {
    let path = path.as_ref();
    let conn = Connection::open(path)
        .map_err(|e| Error::Database(format!("open {}: {e}", path.display())))?;
    configure(&conn)?;
    Ok(conn)
}

/// Open an in-memory database. Intended for tests.
///
/// # Errors
///
/// Returns [`Error::Database`] if the pragmas fail.
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()
        .map_err(|e| Error::Database(format!("open in-memory: {e}")))?;
    configure(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    // `PRAGMA` cannot be parameterized, so we execute literal statements.
    // None of these interpolate caller input.
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;\n\
         PRAGMA journal_mode = WAL;\n\
         PRAGMA synchronous = NORMAL;\n\
         PRAGMA busy_timeout = 5000;",
    )
    .map_err(|e| Error::Database(format!("configure connection: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_pragmas_apply() {
        let conn = open_in_memory().unwrap();

        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1, "foreign_keys must be ON");

        let busy: i64 = conn
            .query_row("PRAGMA busy_timeout;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(busy, 5000);
    }
}
