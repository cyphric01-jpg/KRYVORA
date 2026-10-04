// file: crates/kryvora-cli/src/main.rs
//! KRYVORA command-line driver.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod commands;
mod output;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

/// KRYVORA — unified forensic utility.
#[derive(Debug, Parser)]
#[command(name = "kryvora", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the v0.1 vertical slice on a source file.
    Run {
        #[arg(long, env = "KRYVORA_DB", default_value = "kryvora.db")]
        db: PathBuf,
        #[arg(long)]
        case_title: String,
        #[arg(long)]
        examiner: Option<String>,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        notes: Option<String>,
    },

    /// Verify the audit chain of an existing database.
    Verify {
        #[arg(long, env = "KRYVORA_DB", default_value = "kryvora.db")]
        db: PathBuf,
    },

    /// Sanitize a single file.
    SanitizeFile {
        #[arg(long, env = "KRYVORA_DB", default_value = "kryvora.db")]
        db: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        actor: Option<String>,
    },

    /// Sanitize a directory recursively.
    SanitizeFolder {
        #[arg(long, env = "KRYVORA_DB", default_value = "kryvora.db")]
        db: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        actor: Option<String>,
    },

    /// Carve a source into candidates, validate them, and persist
    /// recovery results with provenance.
    Carve {
        #[arg(long, env = "KRYVORA_DB", default_value = "kryvora.db")]
        db: PathBuf,
        #[arg(long)]
        source: PathBuf,
        #[arg(long, default_value = "Carve Case")]
        case_title: String,
        #[arg(long)]
        examiner: Option<String>,
        #[arg(long)]
        window_size: Option<usize>,
    },

    /// Run recovery against an existing evidence record.
    Recover {
        #[arg(long, env = "KRYVORA_DB", default_value = "kryvora.db")]
        db: PathBuf,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        evidence_id: String,
        #[arg(long)]
        examiner: Option<String>,
    },

    /// Generate a recovery report for a case.
    Report {
        #[arg(long, env = "KRYVORA_DB", default_value = "kryvora.db")]
        db: PathBuf,
        #[arg(long)]
        case_id: String,
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Run {
            db,
            case_title,
            examiner,
            source,
            notes,
        } => commands::run::execute(
            &db,
            &case_title,
            examiner.as_deref(),
            &source,
            notes.as_deref(),
        ),
        Command::Verify { db } => commands::verify::execute(&db),
        Command::SanitizeFile {
            db,
            target,
            yes,
            actor,
        } => commands::sanitize_file::execute(&db, &target, yes, actor.as_deref()),
        Command::SanitizeFolder {
            db,
            target,
            yes,
            actor,
        } => commands::sanitize_folder::execute(&db, &target, yes, actor.as_deref()),
        Command::Carve {
            db,
            source,
            case_title,
            examiner,
            window_size,
        } => commands::carve::execute(&db, &source, &case_title, examiner.as_deref(), window_size),
        Command::Recover {
            db,
            source,
            evidence_id,
            examiner,
        } => commands::recover::execute(&db, &source, &evidence_id, examiner.as_deref()),
        Command::Report { db, case_id, out } => commands::report::execute(&db, &case_id, &out),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
