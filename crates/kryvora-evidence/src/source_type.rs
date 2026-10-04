//! The category of an evidence source.
//!
//! Stored in the `evidence.source_type` column as a lowercase string.
//! The enum exists so that callers cannot pass arbitrary strings for a
//! field the rest of the system relies on to make decisions (for
//! example, whether an evidence source can be hashed as a stream).

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// The category of an evidence source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    /// A single file on a filesystem.
    File,
    /// A directory tree. Registration stores the directory's identity;
    /// per-file hashing is a later, separate operation.
    Directory,
    /// A raw block device (for example, a disk image or a physical
    /// drive). Reserved for the storage-inspection phase.
    BlockDevice,
    /// A memory image captured from a running system.
    MemoryImage,
    /// A network stream captured to a container.
    NetworkCapture,
    /// Anything not yet classified. Explicit, not a null fallback.
    Unknown,
}

impl SourceType {
    /// Stable, lowercase, snake_case name as stored in the database and
    /// transmitted over the wire.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::BlockDevice => "block_device",
            Self::MemoryImage => "memory_image",
            Self::NetworkCapture => "network_capture",
            Self::Unknown => "unknown",
        }
    }

    /// Parse from the stored string form. Case-insensitive.
    ///
    /// # Errors
    ///
    /// Returns [`SourceTypeParseError`] if the string does not match a
    /// known variant.
    pub fn parse(s: &str) -> Result<Self, SourceTypeParseError> {
        match s.to_ascii_lowercase().as_str() {
            "file" => Ok(Self::File),
            "directory" => Ok(Self::Directory),
            "block_device" => Ok(Self::BlockDevice),
            "memory_image" => Ok(Self::MemoryImage),
            "network_capture" => Ok(Self::NetworkCapture),
            "unknown" => Ok(Self::Unknown),
            other => Err(SourceTypeParseError {
                value: other.to_string(),
            }),
        }
    }
}

impl fmt::Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Returned when [`SourceType::parse`] sees a string that does not match
/// any known variant.
#[derive(Debug, Error)]
#[error("unknown source type: {value:?}")]
pub struct SourceTypeParseError {
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_variants() {
        for v in [
            SourceType::File,
            SourceType::Directory,
            SourceType::BlockDevice,
            SourceType::MemoryImage,
            SourceType::NetworkCapture,
            SourceType::Unknown,
        ] {
            assert_eq!(SourceType::parse(v.as_str()).unwrap(), v);
        }
    }

    #[test]
    fn parse_is_case_insensitive() {
        assert_eq!(SourceType::parse("FILE").unwrap(), SourceType::File);
        assert_eq!(
            SourceType::parse("Block_Device").unwrap(),
            SourceType::BlockDevice
        );
    }

    #[test]
    fn parse_rejects_unknown() {
        let err = SourceType::parse("floppy").unwrap_err();
        assert_eq!(err.value, "floppy");
    }

    #[test]
    fn serde_is_snake_case() {
        assert_eq!(
            serde_json::to_string(&SourceType::BlockDevice).unwrap(),
            "\"block_device\""
        );
    }
}
