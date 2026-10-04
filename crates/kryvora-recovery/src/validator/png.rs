//! PNG structural validator.

use super::{FormatValidator, ValidationOutcome, ValidationReason};

const PNG_SIGNATURE: &[u8; 8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

#[derive(Debug)]
pub struct PngValidator;

impl FormatValidator for PngValidator {
    fn format_name(&self) -> &'static str {
        "png"
    }

    fn validate(&self, data: &[u8]) -> ValidationOutcome {
        if data.len() < 8 || &data[..8] != PNG_SIGNATURE {
            return ValidationOutcome::invalid(vec![ValidationReason::new(
                "signature_missing",
                "PNG signature bytes are not present at offset 0",
                false,
            )]);
        }

        let mut pos = 8usize;
        let mut chunk_count = 0u32;
        let mut saw_ihdr = false;
        let mut saw_iend = false;
        let mut width: Option<u32> = None;
        let mut height: Option<u32> = None;
        let mut bit_depth: Option<u8> = None;
        let mut color_type: Option<u8> = None;
        let mut warnings: Vec<String> = Vec::new();

        while pos + 12 <= data.len() {
            let length =
                u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                    as usize;
            let chunk_type = &data[pos + 4..pos + 8];

            // Bounds check on the chunk body + CRC.
            let chunk_end = pos + 8 + length + 4;
            if chunk_end > data.len() {
                warnings.push(format!(
                    "chunk {chunk_type:?} extends past end of data; truncated"
                ));
                break;
            }

            chunk_count += 1;

            if !saw_ihdr && chunk_type != b"IHDR" {
                return ValidationOutcome::invalid(vec![ValidationReason::new(
                    "ihdr_missing",
                    "first chunk must be IHDR",
                    false,
                )]);
            }

            if chunk_type == b"IHDR" {
                if length != 13 {
                    return ValidationOutcome::invalid(vec![ValidationReason::new(
                        "ihdr_length",
                        "IHDR chunk length must be 13",
                        false,
                    )]);
                }
                saw_ihdr = true;
                width = Some(u32::from_be_bytes([
                    data[pos + 8],
                    data[pos + 9],
                    data[pos + 10],
                    data[pos + 11],
                ]));
                height = Some(u32::from_be_bytes([
                    data[pos + 12],
                    data[pos + 13],
                    data[pos + 14],
                    data[pos + 15],
                ]));
                bit_depth = Some(data[pos + 16]);
                color_type = Some(data[pos + 17]);
            }

            if chunk_type == b"IEND" {
                if length != 0 {
                    return ValidationOutcome::invalid(vec![ValidationReason::new(
                        "iend_length",
                        "IEND chunk length must be 0",
                        false,
                    )]);
                }
                saw_iend = true;
                break;
            }

            pos = chunk_end;
        }

        let mut reasons = vec![
            ValidationReason::new("signature_present", "PNG signature present", true),
            ValidationReason::new("ihdr_present", "IHDR chunk present", saw_ihdr),
            ValidationReason::new("iend_present", "IEND chunk present", saw_iend),
        ];

        if !saw_ihdr {
            reasons.push(ValidationReason::new(
                "structure",
                "required chunks not found",
                false,
            ));
            return ValidationOutcome::invalid(reasons);
        }

        let facts = serde_json::json!({
            "width": width,
            "height": height,
            "bit_depth": bit_depth,
            "color_type": color_type,
            "chunk_count": chunk_count,
        });

        if saw_iend {
            ValidationOutcome::valid(reasons, facts)
        } else {
            warnings.push("IEND chunk not found; PNG is truncated".into());
            ValidationOutcome::partial(reasons, warnings, facts)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kryvora_core::ValidationState;

    fn chunk(chunk_type: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&(body.len() as u32).to_be_bytes());
        v.extend_from_slice(chunk_type);
        v.extend_from_slice(body);
        // CRC bytes: we do not validate CRC in this validator, so a
        // placeholder is fine.
        v.extend_from_slice(&[0u8; 4]);
        v
    }

    fn synthetic_png() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(PNG_SIGNATURE);
        // IHDR: 13 bytes = width(4) height(4) bit_depth(1) color(1)
        // compression(1) filter(1) interlace(1)
        let ihdr_body = [
            0x00, 0x00, 0x00, 0x64, // width 100
            0x00, 0x00, 0x00, 0xC8, // height 200
            0x08, 0x06, 0x00, 0x00, 0x00,
        ];
        v.extend_from_slice(&chunk(b"IHDR", &ihdr_body));
        v.extend_from_slice(&chunk(b"IEND", &[]));
        v
    }

    #[test]
    fn valid_synthetic_png() {
        let out = PngValidator.validate(&synthetic_png());
        assert_eq!(out.state, ValidationState::Valid);
    }

    #[test]
    fn rejects_missing_signature() {
        let out = PngValidator.validate(b"not a png at all");
        assert_eq!(out.state, ValidationState::Invalid);
    }

    #[test]
    fn reports_dimensions() {
        let out = PngValidator.validate(&synthetic_png());
        assert_eq!(out.structural_facts["width"], 100);
        assert_eq!(out.structural_facts["height"], 200);
    }

    #[test]
    fn missing_iend_is_partial() {
        let mut v = Vec::new();
        v.extend_from_slice(PNG_SIGNATURE);
        let ihdr_body = [
            0x00, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00, 0xC8, 0x08, 0x06, 0x00, 0x00, 0x00,
        ];
        v.extend_from_slice(&chunk(b"IHDR", &ihdr_body));
        // No IEND.
        let out = PngValidator.validate(&v);
        assert_eq!(out.state, ValidationState::Partial);
    }
}
