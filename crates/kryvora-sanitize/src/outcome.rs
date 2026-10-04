// file: crates/kryvora-sanitize/src/outcome.rs
//! The closed set of sanitization outcomes.

use serde::{Deserialize, Serialize};

/// The final state of a sanitization operation.
///
/// This enum is the only truth value the crate produces. Nothing else
/// in the crate returns a value that means "it worked".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SanitizeOutcome {
    /// Every step of the chosen method completed and its verification
    /// criteria passed. For file overwrite, this requires that a
    /// re-read confirms the original digest is no longer present and
    /// the file has been unlinked.
    Success,

    /// The operation ran but some files were not fully sanitized, or
    /// verification could not be applied to all files.
    Partial,

    /// The operation could not proceed. Nothing was sanitized.
    Failed,

    /// The operation ran but the chosen method cannot guarantee
    /// sanitization on the underlying media. The bytes were written,
    /// but the crate does not claim the original content is
    /// unrecoverable.
    NotVerified,

    /// The target is not supported by the current implementation.
    Unsupported,
}

impl SanitizeOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::NotVerified => "not_verified",
            Self::Unsupported => "unsupported",
        }
    }
}

impl std::fmt::Display for SanitizeOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_is_snake_case() {
        assert_eq!(
            serde_json::to_string(&SanitizeOutcome::NotVerified).unwrap(),
            "\"not_verified\""
        );
        assert_eq!(
            serde_json::to_string(&SanitizeOutcome::Unsupported).unwrap(),
            "\"unsupported\""
        );
    }

    #[test]
    fn display_matches_as_str() {
        assert_eq!(SanitizeOutcome::Success.to_string(), "success");
        assert_eq!(SanitizeOutcome::Unsupported.to_string(), "unsupported");
    }
}
