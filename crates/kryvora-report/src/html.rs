// file: crates/kryvora-report/src/html.rs
//! HTML rendering.

use kryvora_recovery::RecoveryReport;
use kryvora_sanitize::{SanitizationResult, SanitizeOutcome};
use std::fmt::Write as _;

/// Escape a string for safe inclusion in HTML text or attribute
/// context.
#[must_use]
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Render an HTML report for a recovery result set.
#[must_use]
pub fn render_recovery_html(title: &str, case_id: Option<&str>, report: &RecoveryReport) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "<!DOCTYPE html>\n\
         <html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>{title}</title>\
         <style>{style}</style></head><body>\
         <h1>{title}</h1>",
        title = escape_html(title),
        style = STYLE,
    );

    if let Some(c) = case_id {
        let _ = write!(s, "<p><strong>Case:</strong> {}</p>", escape_html(c));
    }

    let _ = write!(
        s,
        "<h2>Summary</h2>\
         <table>\
         <tr><th>Candidates considered</th><td>{}</td></tr>\
         <tr><th>Candidates validated</th><td>{}</td></tr>\
         <tr><th>Candidates rejected</th><td>{}</td></tr>\
         </table>",
        report.candidates_considered, report.candidates_validated, report.candidates_rejected,
    );

    if report.results.is_empty() {
        let _ = write!(s, "<p><em>No validated artifacts.</em></p>");
    } else {
        let _ = write!(
            s,
            "<h2>Recovered artifacts</h2><table>\
            <tr><th>#</th><th>Format</th><th>Category</th>\
            <th>Offset</th><th>Length</th><th>SHA-256</th>\
            <th>Validation</th><th>Confidence</th></tr>"
        );
        for (i, r) in report.results.iter().enumerate() {
            let _ = write!(
                s,
                "<tr>\
                 <td>{i}</td>\
                 <td>{}</td>\
                 <td>{}</td>\
                 <td>{}</td>\
                 <td>{}</td>\
                 <td><code>{}</code></td>\
                 <td>{:?}</td>\
                 <td>{:?}</td>\
                 </tr>",
                escape_html(&r.detected_type),
                escape_html(r.category.as_str()),
                r.source_offset,
                r.source_length,
                escape_html(&r.artifact_sha256),
                r.validation_state,
                r.confidence.level,
            );
        }
        let _ = write!(s, "</table>");
    }

    let _ = write!(s, "</body></html>");
    s
}

/// Render an HTML report for a sanitization result.
#[must_use]
pub fn render_sanitization_html(title: &str, target: &str, result: &SanitizationResult) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "<!DOCTYPE html>\n\
         <html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>{title}</title>\
         <style>{style}</style></head><body>\
         <h1>{title}</h1>\
         <p><strong>Target:</strong> <code>{target}</code></p>",
        title = escape_html(title),
        target = escape_html(target),
        style = STYLE,
    );

    let _ = write!(s, "<h2>Outcome</h2><table>");
    let _ = write!(
        s,
        "<tr><th>Outcome</th><td>{}</td></tr>",
        outcome_str(result.outcome)
    );
    let _ = write!(
        s,
        "<tr><th>Bytes overwritten</th><td>{}</td></tr>",
        result.bytes_overwritten,
    );
    let _ = write!(
        s,
        "<tr><th>Elapsed</th><td>{:.3} s</td></tr>",
        result.elapsed.as_secs_f64(),
    );
    if let Some(reason) = &result.reason {
        let _ = write!(
            s,
            "<tr><th>Reason</th><td>{}</td></tr>",
            escape_html(reason),
        );
    }
    let _ = write!(s, "</table></body></html>");
    s
}

fn outcome_str(o: SanitizeOutcome) -> &'static str {
    match o {
        SanitizeOutcome::Success => "Success",
        SanitizeOutcome::Partial => "Partial",
        SanitizeOutcome::Failed => "Failed",
        SanitizeOutcome::NotVerified => "Not verified",
        SanitizeOutcome::Unsupported => "Unsupported",
    }
}

const STYLE: &str = "\
body { font-family: system-ui, sans-serif; max-width: 960px; margin: 2em auto; padding: 0 1em; }\n\
table { border-collapse: collapse; margin: 1em 0; width: 100%; }\n\
th, td { border: 1px solid #ccc; padding: 0.4em 0.6em; text-align: left; }\n\
th { background: #f0f0f0; }\n\
code { background: #f6f6f6; padding: 0.1em 0.3em; }\n\
";

#[cfg(test)]
mod tests {
    use super::*;
    use kryvora_core::{Confidence, ReconstructionState, RecoveryResultId, ValidationState};
    use kryvora_recovery::{
        ArtifactCategory, ConfidenceAssessment, ConfidenceReason, ConfidenceSignal, RecoveryMethod,
        RecoveryReport, RecoveryResult,
    };
    use std::time::Duration;
    use time::OffsetDateTime;

    #[test]
    fn escape_handles_angle_brackets_and_ampersand() {
        assert_eq!(escape_html("<x>&\"'"), "&lt;x&gt;&amp;&quot;&#39;");
    }

    #[test]
    fn escape_leaves_normal_text_unchanged() {
        assert_eq!(escape_html("hello world"), "hello world");
    }

    fn dummy_result() -> RecoveryResult {
        RecoveryResult {
            id: RecoveryResultId::new(),
            evidence_id: None,
            source_offset: 100,
            source_length: 200,
            detected_type: "jpeg".into(),
            category: ArtifactCategory::Image,
            validation_state: ValidationState::Valid,
            confidence: ConfidenceAssessment::from_reasons(vec![
                ConfidenceReason::new(ConfidenceSignal::SignatureValid, true),
                ConfidenceReason::new(ConfidenceSignal::StructuralValid, true),
                ConfidenceReason::new(ConfidenceSignal::FormatValidated, true),
                ConfidenceReason::new(ConfidenceSignal::SizeConsistent, true),
                ConfidenceReason::new(ConfidenceSignal::FragmentConsistent, true),
                ConfidenceReason::new(ConfidenceSignal::SourceTraceable, true),
                ConfidenceReason::new(ConfidenceSignal::ReconstructionCertain, true),
            ]),
            recovery_method: RecoveryMethod::SignatureCarving,
            reconstruction_state: ReconstructionState::Contiguous,
            artifact_sha256: "a".repeat(64),
            created_at: OffsetDateTime::now_utc(),
            job_id: None,
            provenance_id: None,
            report_id: None,
            validation_facts: serde_json::json!({}),
        }
    }

    #[test]
    fn recovery_html_contains_summary_and_artifact_row() {
        let report = RecoveryReport {
            case_id: None,
            job_id: None,
            candidates_considered: 3,
            candidates_validated: 2,
            candidates_rejected: 1,
            results: vec![dummy_result(), dummy_result()],
        };
        let html = render_recovery_html("Test", Some("CASE-1"), &report);
        assert!(html.contains("Test"));
        assert!(html.contains("CASE-1"));
        assert!(html.contains("jpeg"));
        assert!(html.contains("<table>"));
    }

    #[test]
    fn recovery_html_escapes_untrusted_strings() {
        let mut r = dummy_result();
        r.detected_type = "<script>alert(1)</script>".into();
        let report = RecoveryReport {
            case_id: None,
            job_id: None,
            candidates_considered: 1,
            candidates_validated: 1,
            candidates_rejected: 0,
            results: vec![r],
        };
        let html = render_recovery_html("T", None, &report);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn sanitization_html_reflects_outcome_and_bytes() {
        let r = SanitizationResult::success(1234, Duration::from_millis(500));
        let html = render_sanitization_html("San", "/tmp/x", &r);
        assert!(html.contains("Success"));
        assert!(html.contains("1234"));
        assert!(html.contains("/tmp/x"));
    }

    #[test]
    fn sanitization_html_reflects_not_verified_outcome() {
        let r = SanitizationResult::not_verified(
            500,
            Duration::from_millis(100),
            "solid state media".into(),
        );
        let html = render_sanitization_html("San", "/tmp/x", &r);
        assert!(html.contains("Not verified"));
        assert!(html.contains("solid state media"));
    }

    #[test]
    fn confidence_level_is_mentioned() {
        let report = RecoveryReport {
            case_id: None,
            job_id: None,
            candidates_considered: 1,
            candidates_validated: 1,
            candidates_rejected: 0,
            results: vec![dummy_result()],
        };
        let html = render_recovery_html("T", None, &report);
        assert!(html.contains("High"));
        assert_eq!(Confidence::High, Confidence::High);
    }
}
