//! Rendering and writing a report assembled from persisted case records.

use crate::builder::{write_and_hash, GeneratedReport};
use kryvora_core::Result;
use std::fmt::Write as _;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedCaseReport {
    pub case_id: String,
    pub case_title: String,
    pub examiner: Option<String>,
    pub case_created_at: String,
    pub evidence: Vec<PersistedEvidence>,
    pub jobs: Vec<PersistedJob>,
    pub artifacts: Vec<PersistedArtifact>,
    pub audit_chain: PersistedAuditStatus,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedEvidence {
    pub id: String,
    pub source_path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub integrity_status: String,
    pub integrity_details: String,
    pub registered_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedJob {
    pub id: String,
    pub evidence_id: Option<String>,
    pub kind: String,
    pub state: String,
    pub progress: String,
    pub configuration: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedArtifact {
    pub id: String,
    pub evidence_id: String,
    pub job_id: Option<String>,
    pub format: String,
    pub offset: u64,
    pub length: u64,
    pub validation_status: String,
    pub recovery_status: String,
    pub linkage_status: String,
    pub confidence: String,
    pub confidence_rationale: String,
    pub sha256: String,
    pub output_path: Option<String>,
    pub output_status: String,
    pub validation_facts: String,
    pub provenance: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedAuditStatus {
    pub state: String,
    pub verified_length: u64,
    pub first_bad_sequence: Option<u64>,
    pub reason: Option<String>,
}

pub fn render_persisted_case_report(report: &PersistedCaseReport) -> String {
    let mut html = String::new();
    let _ = write!(
        html,
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Case report {}</title><style>{STYLE}</style></head><body><h1>{}</h1>",
        escape(&report.case_id),
        escape(&report.case_title),
    );
    let _ = write!(
        html,
        "<h2>Case</h2><table><tr><th>Case ID</th><td>{}</td></tr><tr><th>Examiner</th><td>{}</td></tr><tr><th>Created</th><td>{}</td></tr></table>",
        escape(&report.case_id),
        escape(report.examiner.as_deref().unwrap_or("Unavailable")),
        escape(&report.case_created_at),
    );
    let _ = write!(
        html,
        "<h2>Audit-chain verification</h2><table><tr><th>Status</th><td>{}</td></tr><tr><th>Verified events</th><td>{}</td></tr><tr><th>First invalid sequence</th><td>{}</td></tr><tr><th>Reason</th><td>{}</td></tr></table><p>This is a local hash-chain check. It has no external signature or independent anchor and is not proof against an attacker able to rewrite the database and chain.</p>",
        escape(&report.audit_chain.state),
        report.audit_chain.verified_length,
        report.audit_chain.first_bad_sequence.map_or_else(|| "Unavailable".to_string(), |value| value.to_string()),
        escape(report.audit_chain.reason.as_deref().unwrap_or("None reported")),
    );

    let _ = write!(html, "<h2>Evidence ({})</h2>", report.evidence.len());
    if report.evidence.is_empty() {
        let _ = write!(html, "<p>No evidence records are linked to this case.</p>");
    }
    for evidence in &report.evidence {
        let _ = write!(
            html,
            "<h3>Evidence {}</h3><table><tr><th>Source path</th><td>{}</td></tr><tr><th>Size</th><td>{} bytes</td></tr><tr><th>Registered SHA-256</th><td><code>{}</code></td></tr><tr><th>Current integrity</th><td>{}</td></tr><tr><th>Integrity detail</th><td>{}</td></tr><tr><th>Registered at</th><td>{}</td></tr></table>",
            escape(&evidence.id),
            escape(&evidence.source_path),
            evidence.size_bytes,
            escape(&evidence.sha256),
            escape(&evidence.integrity_status),
            escape(&evidence.integrity_details),
            escape(&evidence.registered_at),
        );
    }

    let _ = write!(html, "<h2>Jobs ({})</h2>", report.jobs.len());
    if report.jobs.is_empty() {
        let _ = write!(html, "<p>No jobs are linked to this case.</p>");
    } else {
        let _ = write!(html, "<table><tr><th>Job ID</th><th>Evidence</th><th>Type</th><th>Status</th><th>Progress</th><th>Configuration</th><th>Error</th><th>Created</th><th>Started</th><th>Completed</th></tr>");
        for job in &report.jobs {
            let _ = write!(
                html,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(&job.id),
                escape(job.evidence_id.as_deref().unwrap_or("Unavailable")),
                escape(&job.kind),
                escape(&job.state),
                escape(&job.progress),
                escape(job.configuration.as_deref().unwrap_or("Unavailable")),
                escape(job.error.as_deref().unwrap_or("None")),
                escape(&job.created_at),
                escape(job.started_at.as_deref().unwrap_or("Unavailable")),
                escape(job.completed_at.as_deref().unwrap_or("Unavailable")),
            );
        }
        let _ = write!(html, "</table>");
    }

    let _ = write!(html, "<h2>Persisted candidate results ({})</h2>", report.artifacts.len());
    if report.artifacts.is_empty() {
        let _ = write!(html, "<p>No candidate results are persisted for this case.</p>");
    }
    for artifact in &report.artifacts {
        let _ = write!(
            html,
            "<h3>{} · {}</h3><table><tr><th>Result ID</th><td>{}</td></tr><tr><th>Evidence ID</th><td>{}</td></tr><tr><th>Job ID</th><td>{}</td></tr><tr><th>Source range</th><td>offset {} · {} bytes</td></tr><tr><th>Validation</th><td>{}</td></tr><tr><th>Recovery status</th><td>{}</td></tr><tr><th>Case/evidence/job/provenance linkage</th><td>{}</td></tr><tr><th>Confidence</th><td>{} · {}</td></tr><tr><th>Artifact SHA-256</th><td><code>{}</code></td></tr><tr><th>Recovered output</th><td>{} · {}</td></tr><tr><th>Validation facts</th><td><code>{}</code></td></tr><tr><th>Provenance path</th><td>{}</td></tr><tr><th>Recorded at</th><td>{}</td></tr></table>",
            escape(&artifact.format),
            escape(&artifact.id),
            escape(&artifact.id),
            escape(&artifact.evidence_id),
            escape(artifact.job_id.as_deref().unwrap_or("Unavailable")),
            artifact.offset,
            artifact.length,
            escape(&artifact.validation_status),
            escape(&artifact.recovery_status),
            escape(&artifact.linkage_status),
            escape(&artifact.confidence),
            escape(&artifact.confidence_rationale),
            escape(&artifact.sha256),
            escape(artifact.output_path.as_deref().unwrap_or("Unavailable")),
            escape(&artifact.output_status),
            escape(&artifact.validation_facts),
            escape(&artifact.provenance.join(" → ")),
            escape(&artifact.created_at),
        );
    }

    let _ = write!(html, "<h2>Limitations</h2><ul>");
    for limitation in &report.limitations {
        let _ = write!(html, "<li>{}</li>", escape(limitation));
    }
    let _ = write!(html, "</ul></body></html>");
    html
}

pub fn generate_persisted_case_report(
    path: impl AsRef<Path>,
    report: &PersistedCaseReport,
) -> Result<GeneratedReport> {
    let html = render_persisted_case_report(report);
    write_and_hash(path.as_ref(), &html)
}

fn escape(value: &str) -> String {
    crate::html::escape_html(value)
}

const STYLE: &str = "body{font-family:system-ui,sans-serif;max-width:1100px;margin:2em auto;padding:0 1em;color:#16324a}table{border-collapse:collapse;margin:1em 0 2em;width:100%}th,td{border:1px solid #c8d8e5;padding:.45em .6em;text-align:left;vertical-align:top}th{background:#eaf4fb}code{overflow-wrap:anywhere}p{line-height:1.5}";

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PersistedCaseReport {
        PersistedCaseReport {
            case_id: "case-1".into(),
            case_title: "Test case".into(),
            examiner: None,
            case_created_at: "2026-01-01T00:00:00Z".into(),
            evidence: Vec::new(),
            jobs: Vec::new(),
            artifacts: vec![PersistedArtifact {
                id: "result-1".into(),
                evidence_id: "evidence-1".into(),
                job_id: Some("job-1".into()),
                format: "png".into(),
                offset: 8,
                length: 32,
                validation_status: "partial".into(),
                recovery_status: "Partial; not reported as recovered".into(),
                linkage_status: "not verified".into(),
                confidence: "uncertain".into(),
                confidence_rationale: "structural checks incomplete".into(),
                sha256: "a".repeat(64),
                output_path: None,
                output_status: "not created for partial result".into(),
                validation_facts: "{}".into(),
                provenance: vec!["artifact → candidate → evidence".into()],
                created_at: "2026-01-02T00:00:00Z".into(),
            }],
            audit_chain: PersistedAuditStatus {
                state: "broken".into(),
                verified_length: 2,
                first_bad_sequence: Some(2),
                reason: Some("current hash mismatch".into()),
            },
            limitations: vec!["No filesystem-aware recovery.".into()],
        }
    }

    #[test]
    fn report_renders_persisted_failure_states_without_success_claims() {
        let html = render_persisted_case_report(&sample());
        assert!(html.contains("current hash mismatch"));
        assert!(html.contains("Partial; not reported as recovered"));
        assert!(html.contains("not created for partial result"));
        assert!(html.contains("No filesystem-aware recovery."));
    }

    #[test]
    fn report_output_is_deterministic_and_escapes_untrusted_values() {
        let mut report = sample();
        report.case_title = "<script>bad</script>".into();
        let first = render_persisted_case_report(&report);
        let second = render_persisted_case_report(&report);
        assert_eq!(first, second);
        assert!(!first.contains("<script>bad</script>"));
        assert!(first.contains("&lt;script&gt;bad&lt;/script&gt;"));
    }
}
