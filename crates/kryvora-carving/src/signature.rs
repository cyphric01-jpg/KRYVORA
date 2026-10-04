//! The `Signature` trait and its supporting types.

/// A human-readable format name, e.g. `"jpeg"`, `"png"`, `"pdf"`.
pub type FormatName = &'static str;

/// A signature for a file format.
///
/// A signature knows:
///
/// * Its format name.
/// * The magic bytes that mark the start of a file.
/// * The magic bytes that mark the end, if the format has one. Formats
///   without a fixed footer (e.g. some container formats) return `None`
///   and are **not** supported by this crate's candidate generator
///   until a length-determination strategy is added.
/// * A minimum plausible size (to reject false positives that are too
///   short to be real).
/// * A maximum practical size (to bound the forward search and to
///   discard candidates that exceed any plausible real file).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature {
    pub format: FormatName,
    pub header: &'static [u8],
    pub footer: Option<&'static [u8]>,
    pub min_size: u64,
    pub max_size: u64,
}

impl Signature {
    /// A name suitable for logs, reports, and audit events.
    #[must_use]
    pub const fn format(&self) -> FormatName {
        self.format
    }

    /// The header byte sequence.
    #[must_use]
    pub const fn header(&self) -> &'static [u8] {
        self.header
    }

    /// The footer byte sequence, if this format has one.
    #[must_use]
    pub const fn footer(&self) -> Option<&'static [u8]> {
        self.footer
    }

    /// The minimum plausible size for a file of this format.
    #[must_use]
    pub const fn min_size(&self) -> u64 {
        self.min_size
    }

    /// The maximum practical size for a file of this format.
    #[must_use]
    pub const fn max_size(&self) -> u64 {
        self.max_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE: Signature = Signature {
        format: "fake",
        header: b"\xAB\xCD",
        footer: Some(b"\xEF\xFE"),
        min_size: 4,
        max_size: 1024,
    };

    #[test]
    fn accessors_return_construction_values() {
        assert_eq!(FAKE.format(), "fake");
        assert_eq!(FAKE.header(), b"\xAB\xCD");
        assert_eq!(FAKE.footer(), Some(&b"\xEF\xFE"[..]));
        assert_eq!(FAKE.min_size(), 4);
        assert_eq!(FAKE.max_size(), 1024);
    }
}
