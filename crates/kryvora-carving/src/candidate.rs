//! A carved candidate: a byte range with a detected header and footer.

use kryvora_core::FragmentId;
use kryvora_core::JobId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Identifier for a candidate. A type alias over `FragmentId` so the
/// candidate id namespace is the same as the fragment namespace in
/// `kryvora-core`; a candidate that is later reconstructed into
/// fragments keeps its id.
pub type CandidateId = FragmentId;

/// A byte range identified as a plausible file of a known format.
///
/// **This is not a recovered file.** It is a claim that a header and a
/// footer were both found, and that the range between them fits within
/// the signature's size bounds. Structural validation, format parsing,
/// and classification are the responsibility of `kryvora-recovery`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarvedCandidate {
    /// Fresh id assigned by the scanner.
    pub id: CandidateId,
    /// Format name from the signature that produced this candidate.
    pub format: String,
    /// Absolute byte offset of the header in the source.
    pub offset: u64,
    /// Total length in bytes, from the first byte of the header to the
    /// last byte of the footer, inclusive.
    pub length: u64,
    /// A short prefix of the candidate's bytes, for audit and display.
    pub header_preview: Vec<u8>,
    /// SHA-256 hex of the candidate's bytes at scan time. Computed
    /// lazily by the caller if needed; `None` if not computed.
    pub sha256: Option<String>,
}

impl CarvedCandidate {
    /// The byte range as a `start..end` pair.
    #[must_use]
    pub const fn range(&self) -> (u64, u64) {
        (self.offset, self.offset + self.length)
    }
}

/// A report of a single carving pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarvingReport {
    /// Persisted job that performed this scan, when run through the job API.
    pub job_id: Option<JobId>,
    /// Total bytes read from the source.
    pub bytes_scanned: u64,
    /// Number of candidate ranges emitted.
    pub candidates_found: u64,
    /// Number of header hits for which no footer was found within the
    /// signature's maximum size. These are usually truncated or
    /// overwritten files; they are recorded, not invented into
    /// candidates.
    pub truncated_headers: u64,
    /// True when the configured byte ceiling stopped the scan before EOF.
    pub scan_limit_reached: bool,
    /// True when the configured candidate ceiling stopped further work.
    pub candidate_limit_reached: bool,
    /// Number of signature headers ignored after the open-header ceiling.
    pub headers_dropped_by_limit: u64,
    /// Candidates, in source-offset order.
    pub candidates: Vec<CarvedCandidate>,
}

/// Errors from carving above and beyond [`kryvora_core::Error`].
#[derive(Debug, Error)]
pub enum CarvingError {
    #[error("signature {format} has no footer; scanner cannot bound it")]
    NoFooter { format: String },

    #[error("window size {0} is too small; must be at least 2 * longest header length")]
    WindowTooSmall(usize),

    #[error("invalid scan options: window size {0} or another resource option is outside supported bounds")]
    InvalidOptions(usize),

    #[error("io error while scanning: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_is_offset_plus_length() {
        let c = CarvedCandidate {
            id: CandidateId::new(),
            format: "jpeg".into(),
            offset: 100,
            length: 500,
            header_preview: vec![],
            sha256: None,
        };
        assert_eq!(c.range(), (100, 600));
    }
}
