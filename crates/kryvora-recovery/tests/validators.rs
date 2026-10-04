#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Direct tests of the three format validators.

use kryvora_core::ValidationState;
use kryvora_recovery::validator::validator_for;

fn synthetic_jpeg() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&[0xFF, 0xD8]);
    v.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x64, 0x00, 0xC8, 0x03, 0x01, 0x11, 0x00, 0x02, 0x11,
        0x01, 0x03, 0x11, 0x01,
    ]);
    v.extend_from_slice(&[0xFF, 0xD9]);
    v
}

fn synthetic_png() -> Vec<u8> {
    const SIG: &[u8; 8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let mut v = Vec::new();
    v.extend_from_slice(SIG);
    let mut chunk = |name: &[u8; 4], body: &[u8]| {
        v.extend_from_slice(&(body.len() as u32).to_be_bytes());
        v.extend_from_slice(name);
        v.extend_from_slice(body);
        v.extend_from_slice(&[0u8; 4]);
    };
    chunk(b"IHDR", &[0, 0, 0, 100, 0, 0, 0, 200, 8, 6, 0, 0, 0]);
    chunk(b"IEND", &[]);
    v
}

fn synthetic_pdf() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"%PDF-1.7\n");
    v.extend_from_slice(b"1 0 obj\n<< /Type /Catalog >>\nendobj\n");
    v.extend_from_slice(b"xref\n0 2\n");
    v.extend_from_slice(b"trailer\n<< /Size 2 >>\n");
    v.extend_from_slice(b"startxref\n0\n");
    v.extend_from_slice(b"%%EOF\n");
    v
}

#[test]
fn jpeg_validator_accepts_synthetic() {
    let v = validator_for("jpeg").unwrap();
    assert_eq!(v.format_name(), "jpeg");
    assert_eq!(v.validate(&synthetic_jpeg()).state, ValidationState::Valid);
}

#[test]
fn png_validator_accepts_synthetic() {
    let v = validator_for("png").unwrap();
    assert_eq!(v.format_name(), "png");
    assert_eq!(v.validate(&synthetic_png()).state, ValidationState::Valid);
}

#[test]
fn pdf_validator_accepts_synthetic() {
    let v = validator_for("pdf").unwrap();
    assert_eq!(v.format_name(), "pdf");
    assert_eq!(v.validate(&synthetic_pdf()).state, ValidationState::Valid);
}

#[test]
fn all_validators_reject_garbage() {
    for format in ["jpeg", "png", "pdf"] {
        let v = validator_for(format).unwrap();
        let outcome = v.validate(b"this is not a file of any known format");
        assert_ne!(
            outcome.state,
            ValidationState::Valid,
            "format {format} should not validate garbage"
        );
    }
}
