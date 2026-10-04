//! Structured error taxonomy.
//!
//! Errors are explicitly categorised so callers, audit, and UI can branch
//! on meaning rather than on string content.

use thiserror::Error;

/// Convenience alias used throughout KRYVORA.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// High-level categorisation. Stable, serializable, UI-friendly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    InvalidInput,
    NotFound,
    PermissionDenied,
    UnsupportedTarget,
    UnsafeTarget,
    IntegrityMismatch,
    IoError,
    DatabaseError,
    ValidationFailed,
    ReconstructionFailed,
    PolicyViolation,
    JobCancelled,
    NotImplemented,
    InternalError,
}

/// The KRYVORA error type.
///
/// Library crates return this. The `kryvora-app` binary layer may wrap it
/// in `anyhow` for convenience, but the structured kind is always preserved.
#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("unsupported target: {0}")]
    UnsupportedTarget(String),

    #[error("unsafe target: {0}")]
    UnsafeTarget(String),

    #[error("integrity mismatch (expected {expected}, got {actual})")]
    IntegrityMismatch { expected: String, actual: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Database(String),

    #[error("validation failed: {0}")]
    ValidationFailed(String),

    #[error("reconstruction failed: {0}")]
    ReconstructionFailed(String),

    #[error("policy violation: {0}")]
    PolicyViolation(String),

    #[error("job cancelled")]
    JobCancelled,

    #[error("not implemented: {0}")]
    NotImplemented(&'static str),

    #[error("internal error: {0}")]
    Internal(String),
}

impl Error {
    /// Map any error to its stable category.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            Self::InvalidInput(_) => ErrorKind::InvalidInput,
            Self::NotFound(_) => ErrorKind::NotFound,
            Self::PermissionDenied(_) => ErrorKind::PermissionDenied,
            Self::UnsupportedTarget(_) => ErrorKind::UnsupportedTarget,
            Self::UnsafeTarget(_) => ErrorKind::UnsafeTarget,
            Self::IntegrityMismatch { .. } => ErrorKind::IntegrityMismatch,
            Self::Io(_) => ErrorKind::IoError,
            Self::Database(_) => ErrorKind::DatabaseError,
            Self::ValidationFailed(_) => ErrorKind::ValidationFailed,
            Self::ReconstructionFailed(_) => ErrorKind::ReconstructionFailed,
            Self::PolicyViolation(_) => ErrorKind::PolicyViolation,
            Self::JobCancelled => ErrorKind::JobCancelled,
            Self::NotImplemented(_) => ErrorKind::NotImplemented,
            Self::Internal(_) => ErrorKind::InternalError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_mapping_is_stable() {
        assert_eq!(
            Error::InvalidInput("x".into()).kind(),
            ErrorKind::InvalidInput
        );
        assert_eq!(Error::JobCancelled.kind(), ErrorKind::JobCancelled);
        assert_eq!(
            Error::IntegrityMismatch {
                expected: "a".into(),
                actual: "b".into()
            }
            .kind(),
            ErrorKind::IntegrityMismatch
        );
    }

    #[test]
    fn io_error_converts() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: Error = io.into();
        assert_eq!(err.kind(), ErrorKind::IoError);
    }
}
