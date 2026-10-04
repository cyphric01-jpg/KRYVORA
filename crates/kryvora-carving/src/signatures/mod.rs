//! The signature registry.
//!
//! New formats are added by creating a `pub const SIGNATURE` in a new
//! submodule and appending it to [`REGISTRY`]. Do not hard-code format
//! logic elsewhere in the crate.

pub mod jpeg;
pub mod pdf;
pub mod png;

use crate::signature::Signature;

/// The complete set of signatures this build of KRYVORA understands.
pub const REGISTRY: &[Signature] = &[jpeg::SIGNATURE, png::SIGNATURE, pdf::SIGNATURE];

/// Return the complete set of signatures.
#[must_use]
pub fn all_signatures() -> &'static [Signature] {
    REGISTRY
}

/// The length in bytes of the longest header in the registry. Used to
/// size the window overlap in the scanner so that a header straddling
/// a window boundary is not missed.
#[must_use]
pub fn longest_header_len() -> usize {
    REGISTRY.iter().map(|s| s.header.len()).max().unwrap_or(0)
}

/// The length in bytes of the longest footer in the registry.
#[must_use]
pub fn longest_footer_len() -> usize {
    REGISTRY
        .iter()
        .filter_map(|s| s.footer.map(|f| f.len()))
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_every_supported_format() {
        let names: Vec<_> = REGISTRY.iter().map(|s| s.format).collect();
        assert!(names.contains(&"jpeg"));
        assert!(names.contains(&"png"));
        assert!(names.contains(&"pdf"));
    }

    #[test]
    fn every_signature_has_a_footer_or_is_explicitly_rejected() {
        // Batch 4 supports only formats with a footer. If a future
        // format without a footer is added, this test must be updated
        // deliberately, not silently.
        for s in REGISTRY {
            assert!(
                s.footer.is_some(),
                "format {} has no footer; scanner does not support it yet",
                s.format
            );
        }
    }

    #[test]
    fn every_signature_has_a_nonempty_header() {
        for s in REGISTRY {
            assert!(
                !s.header.is_empty(),
                "format {} has an empty header",
                s.format
            );
        }
    }

    #[test]
    fn min_size_is_at_least_header_plus_footer() {
        for s in REGISTRY {
            let footer_len = s.footer.map(|f| f.len()).unwrap_or(0);
            let floor = (s.header.len() + footer_len) as u64;
            assert!(
                s.min_size >= floor,
                "format {}: min_size {} < header+footer {}",
                s.format,
                s.min_size,
                floor
            );
        }
    }

    #[test]
    fn min_size_less_than_max_size() {
        for s in REGISTRY {
            assert!(
                s.min_size < s.max_size,
                "format {}: min_size {} >= max_size {}",
                s.format,
                s.min_size,
                s.max_size
            );
        }
    }

    #[test]
    fn longest_header_len_is_at_least_eight() {
        // PNG's header is 8 bytes; that is the current maximum.
        assert!(longest_header_len() >= 8);
    }
}
