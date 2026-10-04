// file: crates/kryvora-cli/src/output.rs
//! Console output formatting for the KRYVORA CLI.

use kryvora_audit::ChainStatus;
use kryvora_evidence::Evidence;
use std::path::Path;

/// Print the report for a completed `run`.
pub fn print_run_report(
    case_id: &str,
    evidence_id: &str,
    evidence: &Evidence,
    chain_status: &ChainStatus,
    db_path: &Path,
) {
    let integrity = format!("{:?}", evidence.integrity_state);
    let chain_line = match chain_status {
        ChainStatus::Empty => "Empty".to_string(),
        ChainStatus::Intact { length } => format!("Intact (length {length})"),
        ChainStatus::Broken {
            first_bad_sequence,
            reason,
        } => format!("Broken at sequence {first_bad_sequence}: {reason:?}"),
    };

    println!("=== KRYVORA v0.1 ===");
    println!("database:     {}", db_path.display());
    println!("case:         {case_id}");
    println!("evidence:     {evidence_id}");
    println!("source:       {}", evidence.source_path);
    println!(
        "size:         {} bytes ({})",
        evidence.size_bytes,
        format_bytes(evidence.size_bytes)
    );
    println!("algorithm:    {}", evidence.hash.algorithm().as_str());
    println!("digest:       {}", evidence.hash.digest_hex());
    println!("integrity:    {integrity}");
    println!("tool_version: {}", evidence.tool_version);
    println!("chain:        {chain_line}");
    println!();
}

/// Print the report for a `verify` invocation.
pub fn print_verify_report(db_path: &Path, total: u64, chain_status: &ChainStatus) {
    let chain_line = match chain_status {
        ChainStatus::Empty => "Empty".to_string(),
        ChainStatus::Intact { length } => format!("Intact (length {length})"),
        ChainStatus::Broken {
            first_bad_sequence,
            reason,
        } => format!("BROKEN at sequence {first_bad_sequence}: {reason:?}"),
    };

    println!("=== KRYVORA verify ===");
    println!("database:     {}", db_path.display());
    println!("events:       {total}");
    println!("chain:        {chain_line}");

    if let ChainStatus::Broken {
        first_bad_sequence,
        reason,
    } = chain_status
    {
        println!();
        println!("CHAIN INTEGRITY FAILURE");
        println!("first_bad_sequence: {first_bad_sequence}");
        println!("reason: {reason:?}");
    }
    println!();
}

/// Format a byte count as a human-friendly string.
fn format_bytes(n: u64) -> String {
    const KI: u64 = 1024;
    const MI: u64 = KI * 1024;
    const GI: u64 = MI * 1024;

    if n >= GI {
        format!("{:.2} GiB", n as f64 / GI as f64)
    } else if n >= MI {
        format!("{:.2} MiB", n as f64 / MI as f64)
    } else if n >= KI {
        format!("{:.2} KiB", n as f64 / KI as f64)
    } else {
        format!("{n} B")
    }
}
