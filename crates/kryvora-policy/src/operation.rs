//! Operation requests.

use kryvora_core::Error;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The kinds of operations KRYVORA can perform on a target.
///
/// Not every operation is implemented yet. The policy layer knows the
/// full set so that its decisions are stable; the jobs layer only
/// executes the subset that has a real implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    /// Read-only inspection of a target.
    Inspect,
    /// Compute the SHA-256 digest of a source.
    Hash,
    /// Sanitize a single file.
    SanitizeFile,
    /// Sanitize an entire directory.
    SanitizeDirectory,
    /// Sanitize an entire device or partition.
    SanitizeDevice,
    /// Scan a source for recoverable artifacts.
    Carve,
    /// Reconstruct and validate candidates into artifacts.
    Recover,
    /// Generate a report.
    Report,
}

impl OperationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inspect => "inspect",
            Self::Hash => "hash",
            Self::SanitizeFile => "sanitize_file",
            Self::SanitizeDirectory => "sanitize_directory",
            Self::SanitizeDevice => "sanitize_device",
            Self::Carve => "carve",
            Self::Recover => "recover",
            Self::Report => "report",
        }
    }

    /// Is this operation destructive to the target's data?
    #[must_use]
    pub const fn is_destructive(self) -> bool {
        matches!(
            self,
            Self::SanitizeFile | Self::SanitizeDirectory | Self::SanitizeDevice
        )
    }

    /// Does this operation require administrator / root privileges on
    /// typical platforms?
    #[must_use]
    pub const fn typically_requires_elevation(self) -> bool {
        matches!(self, Self::SanitizeDevice)
    }
}

impl fmt::Display for OperationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A request to perform an operation on a target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationRequest {
    pub kind: OperationKind,
    /// Free-text description shown to the user, e.g. `"erase C:\\temp\\x.bin"`.
    pub description: String,
    /// Whether the caller has already received and acknowledged an
    /// explicit confirmation. The policy layer decides whether this is
    /// sufficient; it does not trust the caller's claim.
    pub user_confirmed: bool,
}

impl OperationRequest {
    /// Construct a request. `user_confirmed` is false; the caller is
    /// expected to set it after receiving user confirmation.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidInput`] if `description` is empty.
    pub fn new(kind: OperationKind, description: impl Into<String>) -> kryvora_core::Result<Self> {
        let description = description.into();
        if description.trim().is_empty() {
            return Err(Error::InvalidInput(
                "operation description must not be empty".into(),
            ));
        }
        Ok(Self {
            kind,
            description,
            user_confirmed: false,
        })
    }

    #[must_use]
    pub fn confirmed(mut self) -> Self {
        self.user_confirmed = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_kinds_are_flagged() {
        assert!(OperationKind::SanitizeFile.is_destructive());
        assert!(OperationKind::SanitizeDirectory.is_destructive());
        assert!(OperationKind::SanitizeDevice.is_destructive());
        assert!(!OperationKind::Inspect.is_destructive());
        assert!(!OperationKind::Hash.is_destructive());
        assert!(!OperationKind::Carve.is_destructive());
    }

    #[test]
    fn only_device_sanitization_requires_elevation() {
        assert!(OperationKind::SanitizeDevice.typically_requires_elevation());
        assert!(!OperationKind::SanitizeFile.typically_requires_elevation());
    }

    #[test]
    fn new_rejects_empty_description() {
        assert!(OperationRequest::new(OperationKind::Inspect, "  ").is_err());
    }

    #[test]
    fn serde_is_snake_case() {
        assert_eq!(
            serde_json::to_string(&OperationKind::SanitizeDevice).unwrap(),
            "\"sanitize_device\""
        );
    }
}
