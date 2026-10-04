//! The [`Hash`] value type and its supporting [`Algorithm`] enum.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use time::OffsetDateTime;

/// Hash algorithms supported by KRYVORA.
///
/// Only `Sha256` is currently implemented. The enum exists so that
/// persisted records carry the algorithm explicitly, and so that adding
/// a new algorithm later is a non-breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Algorithm {
    Sha256,
}

impl Algorithm {
    /// Lowercase, wire-stable name. Use this in reports and databases.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
        }
    }

    /// Length of the raw digest, in bytes.
    #[must_use]
    pub const fn digest_len(self) -> usize {
        match self {
            Self::Sha256 => 32,
        }
    }
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Errors that can be produced when constructing or parsing a [`Hash`].
#[derive(Debug, Error)]
pub enum HashError {
    #[error("hex digest has invalid length: got {got} chars, expected {expected}")]
    InvalidHexLength { got: usize, expected: usize },

    #[error("hex digest contains non-hex characters")]
    InvalidHexCharacter,

    #[error("hex digest does not match declared algorithm {algorithm}")]
    AlgorithmMismatch { algorithm: Algorithm },
}

/// A computed hash: the algorithm, the raw digest, the input size, and
/// the moment the digest was computed.
///
/// `Hash` is a value type. Equality compares all fields. Serde
/// serializes the digest as a lowercase hex string and the algorithm as
/// a snake_case tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hash {
    algorithm: Algorithm,
    /// Lowercase hex, no `0x` prefix, length `2 * algorithm.digest_len()`.
    digest: String,
    /// Number of bytes that were fed into the hasher.
    input_size: u64,
    /// When the digest was computed (UTC).
    #[serde(with = "time::serde::rfc3339")]
    computed_at: OffsetDateTime,
}

impl Hash {
    /// Construct a `Hash` from its parts, validating the digest length
    /// against the declared algorithm.
    pub fn new(
        algorithm: Algorithm,
        digest_hex: impl Into<String>,
        input_size: u64,
        computed_at: OffsetDateTime,
    ) -> Result<Self, HashError> {
        let digest_hex = digest_hex.into();
        let expected_chars = algorithm.digest_len() * 2;

        if digest_hex.len() != expected_chars {
            return Err(HashError::InvalidHexLength {
                got: digest_hex.len(),
                expected: expected_chars,
            });
        }

        if !digest_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(HashError::InvalidHexCharacter);
        }

        Ok(Self {
            algorithm,
            digest: digest_hex.to_ascii_lowercase(),
            input_size,
            computed_at,
        })
    }

    #[must_use]
    pub const fn algorithm(&self) -> Algorithm {
        self.algorithm
    }

    #[must_use]
    pub fn digest_hex(&self) -> &str {
        &self.digest
    }

    #[must_use]
    pub const fn input_size(&self) -> u64 {
        self.input_size
    }

    #[must_use]
    pub const fn computed_at(&self) -> OffsetDateTime {
        self.computed_at
    }

    /// Constant-time-ish equality on the digest only, ignoring
    /// `computed_at` and `input_size`.
    ///
    /// Not truly constant time because we compare `str` via `==`, but the
    /// digest is a public artifact of the input, not a secret. A future
    /// revision that handles HMACs would switch this to `subtle`.
    #[must_use]
    pub fn digest_matches(&self, other: &Self) -> bool {
        self.algorithm == other.algorithm && self.digest == other.digest
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.algorithm.as_str(), self.digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_time() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    #[test]
    fn algorithm_strings_are_stable() {
        assert_eq!(Algorithm::Sha256.as_str(), "sha256");
        assert_eq!(Algorithm::Sha256.digest_len(), 32);
        assert_eq!(Algorithm::Sha256.to_string(), "sha256");
    }

    #[test]
    fn new_rejects_wrong_length() {
        let err = Hash::new(Algorithm::Sha256, "abc", 0, fixed_time()).unwrap_err();
        assert!(matches!(
            err,
            HashError::InvalidHexLength {
                got: 3,
                expected: 64
            }
        ));
    }

    #[test]
    fn new_rejects_non_hex() {
        let bad = "z".repeat(64);
        let err = Hash::new(Algorithm::Sha256, bad, 0, fixed_time()).unwrap_err();
        assert!(matches!(err, HashError::InvalidHexCharacter));
    }

    #[test]
    fn new_lowercases_digest() {
        let upper = "AB".repeat(32);
        let h = Hash::new(Algorithm::Sha256, upper, 0, fixed_time()).unwrap();
        assert_eq!(h.digest_hex(), &"ab".repeat(32));
    }

    #[test]
    fn digest_matches_ignores_metadata() {
        let a = Hash::new(Algorithm::Sha256, "00".repeat(32), 10, fixed_time()).unwrap();
        let b = Hash::new(Algorithm::Sha256, "00".repeat(32), 999, fixed_time()).unwrap();
        assert!(a.digest_matches(&b));
        assert_ne!(a, b); // full equality differs on input_size
    }

    #[test]
    fn display_is_algorithm_colon_digest() {
        let h = Hash::new(Algorithm::Sha256, "ab".repeat(32), 0, fixed_time()).unwrap();
        assert_eq!(h.to_string(), format!("sha256:{}", "ab".repeat(32)));
    }
}
