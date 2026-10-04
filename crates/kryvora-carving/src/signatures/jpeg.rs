//! JPEG signature.

use crate::signature::Signature;

/// JPEG files begin with `FF D8 FF` (SOI marker followed by the first
/// marker's lead byte) and end with `FF D9` (EOI marker).
///
/// The `min_size` floor of 16 bytes is a sanity check, not a format
/// validation. Real JPEGs are much larger; the floor exists only to
/// reject the degenerate "header immediately followed by footer" case.
/// Structural validation of JPEG internals is the responsibility of
/// `kryvora-recovery`.
pub const SIGNATURE: Signature = Signature {
    format: "jpeg",
    header: &[0xFF, 0xD8, 0xFF],
    footer: Some(&[0xFF, 0xD9]),
    min_size: 16,
    max_size: 64 * 1024 * 1024,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_three_bytes_starting_ffd8ff() {
        assert_eq!(SIGNATURE.header, &[0xFF, 0xD8, 0xFF]);
    }

    #[test]
    fn footer_is_ffd9() {
        assert_eq!(SIGNATURE.footer, Some(&[0xFF, 0xD9][..]));
    }
}
