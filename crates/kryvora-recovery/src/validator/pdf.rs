//! PDF structural validator.

use super::{FormatValidator, ValidationOutcome, ValidationReason};

#[derive(Debug)]
pub struct PdfValidator;

impl FormatValidator for PdfValidator {
    fn format_name(&self) -> &'static str {
        "pdf"
    }

    fn validate(&self, data: &[u8]) -> ValidationOutcome {
        // 1. Header: %PDF-x.y, where x and y are digits.
        if data.len() < 8 || &data[..5] != b"%PDF-" {
            return ValidationOutcome::invalid(vec![ValidationReason::new(
                "header_missing",
                "PDF must begin with %PDF-",
                false,
            )]);
        }

        let version_digits: Vec<u8> = data[5..]
            .iter()
            .take_while(|b| b.is_ascii_digit() || **b == b'.')
            .copied()
            .collect();
        let version_str = String::from_utf8_lossy(&version_digits).into_owned();

        if version_str.len() < 3 || !version_str.contains('.') {
            return ValidationOutcome::invalid(vec![ValidationReason::new(
                "version_invalid",
                "PDF version must be of the form X.Y",
                false,
            )]);
        }

        // 2. Body: at least one `obj` token.
        let has_obj = find_subsequence(data, b" obj") || find_subsequence(data, b"\nobj");

        // 3. Trailer: `%%EOF` within the last 1024 bytes.
        let tail_start = data.len().saturating_sub(1024);
        let has_eof = find_subsequence(&data[tail_start..], b"%%EOF");

        // 4. Cross-reference: `xref` or `/Type /XRef`.
        let has_xref = find_subsequence(data, b"xref")
            || find_subsequence(data, b"/Type /XRef")
            || find_subsequence(data, b"/Type/XRef");

        let mut reasons = vec![
            ValidationReason::new("header_present", "PDF header present", true),
            ValidationReason::new("version_present", "PDF version present", true),
            ValidationReason::new("obj_present", "at least one object token present", has_obj),
            ValidationReason::new("xref_present", "cross-reference table present", has_xref),
            ValidationReason::new("eof_present", "%%EOF marker present", has_eof),
        ];

        let facts = serde_json::json!({
            "version": version_str,
            "has_obj": has_obj,
            "has_xref": has_xref,
            "has_eof": has_eof,
        });

        match (has_obj, has_xref, has_eof) {
            (true, true, true) => ValidationOutcome::valid(reasons, facts),
            _ => {
                reasons.push(ValidationReason::new(
                    "structure",
                    "one or more required structural elements missing",
                    false,
                ));
                let mut warnings = Vec::new();
                if !has_obj {
                    warnings.push("no PDF object token found".into());
                }
                if !has_xref {
                    warnings.push("no cross-reference table found".into());
                }
                if !has_eof {
                    warnings.push("no %%EOF found within last 1024 bytes".into());
                }
                ValidationOutcome::partial(reasons, warnings, facts)
            }
        }
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kryvora_core::ValidationState;

    fn synthetic_pdf() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"%PDF-1.7\n");
        v.extend_from_slice(b"1 0 obj\n<< /Type /Catalog >>\nendobj\n");
        v.extend_from_slice(b"xref\n0 2\n");
        v.extend_from_slice(b"trailer\n<< /Size 2 >>\n");
        v.extend_from_slice(b"startxref\n0\n");
        v.extend_from_slice(b"%%EOF\n");
        v
    }

    #[test]
    fn valid_synthetic_pdf() {
        let out = PdfValidator.validate(&synthetic_pdf());
        assert_eq!(out.state, ValidationState::Valid);
    }

    #[test]
    fn rejects_missing_header() {
        let out = PdfValidator.validate(b"not a pdf");
        assert_eq!(out.state, ValidationState::Invalid);
    }

    #[test]
    fn missing_xref_is_partial() {
        let mut v = Vec::new();
        v.extend_from_slice(b"%PDF-1.7\n");
        v.extend_from_slice(b"1 0 obj\n<< /Type /Catalog >>\nendobj\n");
        v.extend_from_slice(b"%%EOF\n");
        let out = PdfValidator.validate(&v);
        assert_eq!(out.state, ValidationState::Partial);
    }

    #[test]
    fn reports_version() {
        let out = PdfValidator.validate(&synthetic_pdf());
        assert_eq!(out.structural_facts["version"], "1.7");
    }
}
