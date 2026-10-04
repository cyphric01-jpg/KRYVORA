//! Artifact categories and recovery methods.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Broad category of a recovered artifact. Matches Section 24 of the
/// master build prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactCategory {
    Image,
    Video,
    Audio,
    Document,
    Archive,
    Database,
    ApplicationArtifact,
    Unknown,
}

impl ArtifactCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Document => "document",
            Self::Archive => "archive",
            Self::Database => "database",
            Self::ApplicationArtifact => "application_artifact",
            Self::Unknown => "unknown",
        }
    }

    /// Classify a format name into a category.
    ///
    /// Format names are lowercase strings from `kryvora-carving`'s
    /// signature registry. Unknown formats map to
    /// [`ArtifactCategory::Unknown`]; they are not guessed at.
    #[must_use]
    pub fn from_format(format: &str) -> Self {
        match format {
            "jpeg" | "png" => Self::Image,
            "pdf" => Self::Document,
            _ => Self::Unknown,
        }
    }
}

impl fmt::Display for ArtifactCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How an artifact was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryMethod {
    /// Signature-based carving: header and footer were located in a
    /// single contiguous byte range.
    SignatureCarving,
    /// Fragment reassembly across multiple disjoint ranges. Not used
    /// in Batch 5; reserved for a later batch.
    FragmentReassembly,
    /// Header-only carving where the footer was inferred from
    /// structure rather than located. Not used in Batch 5.
    HeaderOnly,
}

impl RecoveryMethod {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SignatureCarving => "signature_carving",
            Self::FragmentReassembly => "fragment_reassembly",
            Self::HeaderOnly => "header_only",
        }
    }
}

impl fmt::Display for RecoveryMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_map_from_known_formats() {
        assert_eq!(
            ArtifactCategory::from_format("jpeg"),
            ArtifactCategory::Image
        );
        assert_eq!(
            ArtifactCategory::from_format("png"),
            ArtifactCategory::Image
        );
        assert_eq!(
            ArtifactCategory::from_format("pdf"),
            ArtifactCategory::Document
        );
    }

    #[test]
    fn unknown_format_maps_to_unknown_category() {
        assert_eq!(
            ArtifactCategory::from_format("gzip"),
            ArtifactCategory::Unknown
        );
    }

    #[test]
    fn serde_is_snake_case() {
        assert_eq!(
            serde_json::to_string(&ArtifactCategory::ApplicationArtifact).unwrap(),
            "\"application_artifact\""
        );
        assert_eq!(
            serde_json::to_string(&RecoveryMethod::SignatureCarving).unwrap(),
            "\"signature_carving\""
        );
    }

    #[test]
    fn display_matches_as_str() {
        assert_eq!(ArtifactCategory::Image.to_string(), "image");
        assert_eq!(
            RecoveryMethod::SignatureCarving.to_string(),
            "signature_carving"
        );
    }
}
