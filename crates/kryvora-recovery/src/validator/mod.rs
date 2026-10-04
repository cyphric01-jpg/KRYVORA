//! Format validators.
//!
//! Each validator walks the actual bytes of a candidate and returns a
//! structured outcome. Validators do not perform a full parse; they
//! confirm the structural shape of the format and reject obvious
//! false positives. Anything ambiguous is reported as
//! `ValidationState::Partial` or `ValidationState::Inconclusive`, not
//! as `Valid`.

pub mod jpeg;
pub mod pdf;
pub mod png;

use kryvora_core::ValidationState;
use serde::{Deserialize, Serialize};

/// A single structured reason behind a validation decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReason {
    pub code: String,
    pub message: String,
    pub satisfied: bool,
}

impl ValidationReason {
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>, satisfied: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            satisfied,
        }
    }
}

/// The structured outcome of a format validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationOutcome {
    pub state: ValidationState,
    pub reasons: Vec<ValidationReason>,
    pub warnings: Vec<String>,
    pub structural_facts: serde_json::Value,
}

impl ValidationOutcome {
    #[must_use]
    pub fn valid(reasons: Vec<ValidationReason>, facts: serde_json::Value) -> Self {
        Self {
            state: ValidationState::Valid,
            reasons,
            warnings: Vec::new(),
            structural_facts: facts,
        }
    }

    #[must_use]
    pub fn partial(
        reasons: Vec<ValidationReason>,
        warnings: Vec<String>,
        facts: serde_json::Value,
    ) -> Self {
        Self {
            state: ValidationState::Partial,
            reasons,
            warnings,
            structural_facts: facts,
        }
    }

    #[must_use]
    pub fn invalid(reasons: Vec<ValidationReason>) -> Self {
        Self {
            state: ValidationState::Invalid,
            reasons,
            warnings: Vec::new(),
            structural_facts: serde_json::json!({}),
        }
    }

    #[must_use]
    pub fn inconclusive(reasons: Vec<ValidationReason>, warnings: Vec<String>) -> Self {
        Self {
            state: ValidationState::Inconclusive,
            reasons,
            warnings,
            structural_facts: serde_json::json!({}),
        }
    }
}

/// A format validator.
pub trait FormatValidator {
    /// The format name this validator handles. Must match the name
    /// used by the corresponding signature in `kryvora-carving`.
    fn format_name(&self) -> &'static str;

    /// Validate the candidate's bytes.
    fn validate(&self, candidate_data: &[u8]) -> ValidationOutcome;
}

/// Return a validator for the given format name, or `None` if no
/// validator is registered.
#[must_use]
pub fn validator_for(format: &str) -> Option<Box<dyn FormatValidator + Send + Sync>> {
    match format {
        "jpeg" => Some(Box::new(jpeg::JpegValidator)),
        "png" => Some(Box::new(png::PngValidator)),
        "pdf" => Some(Box::new(pdf::PdfValidator)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_for_known_formats() {
        assert!(validator_for("jpeg").is_some());
        assert!(validator_for("png").is_some());
        assert!(validator_for("pdf").is_some());
    }

    #[test]
    fn validator_for_unknown_format_returns_none() {
        assert!(validator_for("gzip").is_none());
    }

    #[test]
    fn outcome_constructors_set_the_right_state() {
        assert_eq!(
            ValidationOutcome::valid(vec![], serde_json::json!({})).state,
            ValidationState::Valid
        );
        assert_eq!(
            ValidationOutcome::invalid(vec![]).state,
            ValidationState::Invalid
        );
        assert_eq!(
            ValidationOutcome::partial(vec![], vec![], serde_json::json!({})).state,
            ValidationState::Partial
        );
        assert_eq!(
            ValidationOutcome::inconclusive(vec![], vec![]).state,
            ValidationState::Inconclusive
        );
    }
}
