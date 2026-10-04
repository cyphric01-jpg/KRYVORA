//! PDF signature.

use crate::signature::Signature;

/// PDF files begin with `%PDF-` (the version digits follow) and end
/// with `%%EOF`.
pub const SIGNATURE: Signature = Signature {
    format: "pdf",
    header: b"%PDF-",
    footer: Some(b"%%EOF"),
    min_size: 16,
    max_size: 512 * 1024 * 1024,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_percent_pdf_dash() {
        assert_eq!(SIGNATURE.header, b"%PDF-");
    }

    #[test]
    fn footer_is_percent_percent_eof() {
        assert_eq!(SIGNATURE.footer, Some(&b"%%EOF"[..]));
    }
}
