//! JPEG structural validator.

use super::{FormatValidator, ValidationOutcome, ValidationReason};

#[derive(Debug)]
pub struct JpegValidator;

impl FormatValidator for JpegValidator {
    fn format_name(&self) -> &'static str {
        "jpeg"
    }

    fn validate(&self, data: &[u8]) -> ValidationOutcome {
        // 1. SOI marker.
        if data.len() < 2 || data[0] != 0xFF || data[1] != 0xD8 {
            return ValidationOutcome::invalid(vec![ValidationReason::new(
                "soi_missing",
                "JPEG must begin with SOI marker FF D8",
                false,
            )]);
        }

        // 2. Walk markers. A JPEG is a sequence of markers; each
        //    marker is FF followed by a non-zero, non-FF byte. Marker
        //    FF D9 (EOI) terminates the stream.
        let mut i = 2usize;
        let mut saw_sof = false;
        let mut sof_dimensions: Option<(u16, u16)> = None;
        let mut marker_count = 0u32;

        while i + 1 < data.len() {
            if data[i] != 0xFF {
                // Not aligned on a marker; skip forward one byte.
                i += 1;
                continue;
            }
            let marker = data[i + 1];
            if marker == 0x00 || marker == 0xFF {
                i += 1;
                continue;
            }

            marker_count += 1;

            if marker == 0xD9 {
                // EOI.
                let reasons = vec![
                    ValidationReason::new("soi_present", "SOI marker present", true),
                    ValidationReason::new("eoi_present", "EOI marker present", true),
                    ValidationReason::new(
                        "sof_present",
                        "SOF (start of frame) marker present",
                        saw_sof,
                    ),
                ];

                if !saw_sof {
                    return ValidationOutcome::partial(
                        reasons,
                        vec!["no SOF marker found before EOI".into()],
                        serde_json::json!({
                            "marker_count": marker_count,
                        }),
                    );
                }

                let facts = match sof_dimensions {
                    Some((w, h)) => serde_json::json!({
                        "marker_count": marker_count,
                        "width": w,
                        "height": h,
                    }),
                    None => serde_json::json!({ "marker_count": marker_count }),
                };

                return ValidationOutcome::valid(reasons, facts);
            }

            // SOF markers: C0..CF, excluding C4 (DHT), C8 (JPG),
            // CC (DAC).
            if (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC
            {
                saw_sof = true;
                // SOF segment: FF Cn, length (2 bytes), precision
                // (1 byte), height (2 bytes), width (2 bytes).
                if i + 9 <= data.len() {
                    let h = u16::from_be_bytes([data[i + 5], data[i + 6]]);
                    let w = u16::from_be_bytes([data[i + 7], data[i + 8]]);
                    sof_dimensions = Some((w, h));
                }
                // Skip the SOF segment body.
                if i + 4 <= data.len() {
                    let seg_len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
                    i = i.saturating_add(2).saturating_add(seg_len);
                } else {
                    i += 2;
                }
                continue;
            }

            // Standalone markers: D0..D7 (RSTn), 01 (TEM). No length.
            if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
                i += 2;
                continue;
            }

            // Markers with a length field.
            if i + 4 <= data.len() {
                let seg_len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
                i = i.saturating_add(2).saturating_add(seg_len);
            } else {
                break;
            }
        }

        // Reached end of data without EOI.
        ValidationOutcome::partial(
            vec![
                ValidationReason::new("soi_present", "SOI marker present", true),
                ValidationReason::new("eoi_present", "EOI marker present", false),
                ValidationReason::new("sof_present", "SOF marker present", saw_sof),
            ],
            vec!["no EOI marker found before end of candidate".into()],
            serde_json::json!({ "marker_count": marker_count }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kryvora_core::ValidationState;

    fn synthetic_jpeg() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&[0xFF, 0xD8]);
        v.extend_from_slice(&[
            0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x64, 0x00, 0xC8, 0x03, 0x01, 0x11, 0x00, 0x02,
            0x11, 0x01, 0x03, 0x11, 0x01,
        ]);
        v.extend_from_slice(&[0xFF, 0xD9]);
        v
    }

    #[test]
    fn valid_synthetic_jpeg() {
        let out = JpegValidator.validate(&synthetic_jpeg());
        assert_eq!(out.state, ValidationState::Valid);
    }

    #[test]
    fn rejects_missing_soi() {
        let out = JpegValidator.validate(b"not a jpeg");
        assert_eq!(out.state, ValidationState::Invalid);
    }

    #[test]
    fn reports_dimensions_from_sof() {
        let out = JpegValidator.validate(&synthetic_jpeg());
        assert_eq!(out.structural_facts["width"], 200);
        assert_eq!(out.structural_facts["height"], 100);
    }

    #[test]
    fn no_eoi_is_partial() {
        let mut v = synthetic_jpeg();
        v.truncate(v.len() - 2);
        let out = JpegValidator.validate(&v);
        assert_eq!(out.state, ValidationState::Partial);
    }
}
