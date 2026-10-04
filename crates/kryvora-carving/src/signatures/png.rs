//! PNG signature.

use crate::signature::Signature;

/// PNG files begin with the 8-byte PNG magic and end with the IEND
/// chunk. The IEND chunk's fixed bytes are `49 45 4E 44 AE 42 60 82`
/// (length `00 00 00 00`, type `IEND`, CRC `AE 42 60 82`). We match
/// the type-plus-CRC sequence, which is sufficient to identify the end
/// of a well-formed PNG.
pub const SIGNATURE: Signature = Signature {
    format: "png",
    header: &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
    footer: Some(&[0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82]),
    min_size: 20,
    max_size: 256 * 1024 * 1024,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_eight_bytes_png_magic() {
        assert_eq!(
            SIGNATURE.header,
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }

    #[test]
    fn footer_is_iend_chunk_marker() {
        assert_eq!(
            SIGNATURE.footer,
            Some(&[0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82][..])
        );
    }
}
