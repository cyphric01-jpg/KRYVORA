//! The streaming block scanner.
//!
//! The scanner reads the source in fixed-size windows and maintains an
//! open-header list across windows. A header that begins in one window
//! and whose footer is in a later window is still resolved, as long as
//! the total header-to-footer distance is within the signature's
//! `max_size`.
//!
//! # Design
//!
//! * `window_size` — bytes read per iteration.
//! * `overlap` — bytes retained from the end of one buffer as the
//!   prefix of the next. Set to `max(longest_header_len,
//!   longest_footer_len)` so that both a straddling header and a
//!   straddling footer are fully visible in at least one window.
//! * `open_headers` — headers whose footers have not yet been found.
//!   Each entry records the header's absolute offset, the signature
//!   index, and the buffer position from which to resume the footer
//!   search on the next window.
//!
//! The scanner never invents a candidate without a footer.

use crate::candidate::{CandidateId, CarvedCandidate, CarvingError, CarvingReport};
use crate::signature::Signature;
use crate::signatures;
use kryvora_core::{Error, Result};
use memchr::memmem;
use std::io::Read;

pub const MAX_SCAN_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_CANDIDATES: usize = 10_000;
pub const MAX_OPEN_HEADERS: usize = 4_096;
pub const MAX_HEADER_PREVIEW_LEN: usize = 4_096;

/// Options for a scan.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Size of each read window in bytes. Defaults to 1 MiB.
    pub window_size: usize,
    /// Length of the header preview stored on each candidate. Defaults
    /// to 16 bytes.
    pub header_preview_len: usize,
    /// Known source size for meaningful job progress. Does not affect scanning.
    pub source_size_bytes: Option<u64>,
    /// Maximum number of source bytes to inspect. Defaults to 4 GiB.
    pub max_scan_bytes: u64,
    /// Maximum complete candidates retained. Defaults to 10,000.
    pub max_candidates: usize,
    /// Maximum simultaneous headers awaiting a footer. Defaults to 4,096.
    pub max_open_headers: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            window_size: 1024 * 1024,
            header_preview_len: 16,
            source_size_bytes: None,
            max_scan_bytes: 4 * 1024 * 1024 * 1024,
            max_candidates: 10_000,
            max_open_headers: 4_096,
        }
    }
}

/// Scan a source for candidates.
///
/// The source is read as a stream in `window_size` chunks. An overlap
/// of `max(longest_header, longest_footer)` bytes is carried between
/// windows. A header that is detected in one window but whose footer
/// appears in a later window is tracked across windows until the
/// footer is found or the signature's `max_size` is exceeded.
///
/// # Errors
///
/// * [`Error::Io`] — the source could not be read.
/// * [`Error::InvalidInput`] — `window_size` is too small.
pub fn scan_source<R: Read>(reader: R, options: &ScanOptions) -> Result<CarvingReport> {
    scan_source_with_checkpoint(reader, options, |_| true)
}

/// Scan a source while reporting checkpoints after each bounded read.
/// Return `false` from the callback to stop cooperatively.
pub fn scan_source_with_checkpoint<R, F>(
    reader: R,
    options: &ScanOptions,
    checkpoint: F,
) -> Result<CarvingReport>
where
    R: Read,
    F: FnMut(u64) -> bool,
{
    let signatures = signatures::all_signatures();
    let longest_header = signatures::longest_header_len();
    let longest_footer = signatures::longest_footer_len();
    let overlap = longest_header.max(longest_footer);

    if options.window_size < 2 * overlap.max(1)
        || options.window_size > 16 * 1024 * 1024
        || options.header_preview_len > MAX_HEADER_PREVIEW_LEN
        || options.max_scan_bytes == 0
        || options.max_scan_bytes > MAX_SCAN_BYTES
        || options.max_candidates == 0
        || options.max_candidates > MAX_CANDIDATES
        || options.max_open_headers == 0
        || options.max_open_headers > MAX_OPEN_HEADERS
    {
        return Err(Error::InvalidInput(
            CarvingError::InvalidOptions(options.window_size).to_string(),
        ));
    }

    Scanner::new(
        reader,
        signatures,
        options.window_size,
        overlap,
        options.header_preview_len,
        options.max_scan_bytes,
        options.max_candidates,
        options.max_open_headers,
    )
    .run_with_checkpoint(checkpoint)
}

/// An open header whose footer has not yet been located.
#[derive(Debug)]
struct OpenHeader {
    /// Absolute byte offset of the header in the source.
    absolute_offset: u64,
    /// Index into `signatures`.
    signature_index: usize,
    /// Buffer position at which to resume the footer search on the
    /// next window. Measured relative to the start of the current
    /// buffer.
    resume_at: usize,
    /// Length of the header preview captured at detection time.
    preview: Vec<u8>,
}

struct Scanner<'a, R: Read> {
    reader: R,
    signatures: &'a [Signature],
    window_size: usize,
    overlap: usize,
    header_preview_len: usize,
    max_scan_bytes: u64,
    max_candidates: usize,
    max_open_headers: usize,
    /// Absolute offset of `buffer[0]` in the source.
    buffer_start_offset: u64,
    /// Rolling buffer. Grows to `window_size + overlap`, then stays
    /// there: after each scan the last `overlap` bytes are retained.
    buffer: Vec<u8>,
    open_headers: Vec<OpenHeader>,
    candidates: Vec<CarvedCandidate>,
    truncated_headers: u64,
    scan_limit_reached: bool,
    candidate_limit_reached: bool,
    headers_dropped_by_limit: u64,
}

impl<'a, R: Read> Scanner<'a, R> {
    fn new(
        reader: R,
        signatures: &'a [Signature],
        window_size: usize,
        overlap: usize,
        header_preview_len: usize,
        max_scan_bytes: u64,
        max_candidates: usize,
        max_open_headers: usize,
    ) -> Self {
        Self {
            reader,
            signatures,
            window_size,
            overlap,
            header_preview_len,
            max_scan_bytes,
            max_candidates,
            max_open_headers,
            buffer_start_offset: 0,
            buffer: Vec::with_capacity(window_size + overlap),
            open_headers: Vec::new(),
            candidates: Vec::new(),
            truncated_headers: 0,
            scan_limit_reached: false,
            candidate_limit_reached: false,
            headers_dropped_by_limit: 0,
        }
    }

    fn run_with_checkpoint<F>(mut self, mut checkpoint: F) -> Result<CarvingReport>
    where
        F: FnMut(u64) -> bool,
    {
        let mut chunk = vec![0u8; self.window_size];
        let mut total_read: u64 = 0;

        loop {
            if !checkpoint(total_read) {
                return Err(Error::JobCancelled);
            }
            let remaining = self.max_scan_bytes.saturating_sub(total_read);
            if remaining == 0 {
                let mut probe = [0u8; 1];
                self.scan_limit_reached = self.reader.read(&mut probe).map_err(Error::Io)? != 0;
                self.scan_buffer(true)?;
                break;
            }
            let read_capacity = usize::try_from(remaining.min(self.window_size as u64))
                .unwrap_or(self.window_size);
            let n = self
                .reader
                .read(&mut chunk[..read_capacity])
                .map_err(Error::Io)?;
            let eof = n == 0;

            if !eof {
                self.buffer.extend_from_slice(&chunk[..n]);
                total_read = total_read
                    .checked_add(n as u64)
                    .ok_or_else(|| Error::InvalidInput("scan byte count overflow".into()))?;
            }

            self.scan_buffer(eof)?;
            if !checkpoint(total_read) {
                return Err(Error::JobCancelled);
            }

            if self.candidate_limit_reached {
                break;
            }

            if eof {
                break;
            }

            // Retain the last `overlap` bytes for the next iteration.
            // Advance `buffer_start_offset` by the number of bytes
            // discarded.
            let keep = self.overlap.min(self.buffer.len());
            let drop = self.buffer.len() - keep;
            self.buffer.drain(..drop);
            self.buffer_start_offset += drop as u64;

            // Any open header whose `resume_at` was inside the dropped
            // region is now orphaned: its remaining search region is
            // gone. Since we always retain at least `overlap` bytes,
            // and `overlap >= longest_footer`, a footer that starts in
            // the retained region is still findable. But a header whose
            // `resume_at` is in the dropped region must have its
            // resume_at clamped to 0.
            for oh in &mut self.open_headers {
                oh.resume_at = oh.resume_at.saturating_sub(drop);
            }
        }

        self.candidates.sort_by_key(|candidate| candidate.offset);
        Ok(CarvingReport {
            job_id: None,
            bytes_scanned: total_read,
            candidates_found: self.candidates.len() as u64,
            truncated_headers: self.truncated_headers,
            scan_limit_reached: self.scan_limit_reached,
            candidate_limit_reached: self.candidate_limit_reached,
            headers_dropped_by_limit: self.headers_dropped_by_limit,
            candidates: self.candidates,
        })
    }

    fn scan_buffer(&mut self, eof: bool) -> Result<()> {
        // 1. Detect new headers in the buffer that are not already
        //    open. We search the entire buffer, then filter out hits
        //    that match an existing open header's offset.
        let buffer_len = self.buffer.len();
        let remaining_header_slots = self
            .max_open_headers
            .saturating_sub(self.open_headers.len());
        let mut new_headers: Vec<(u64, usize)> = Vec::with_capacity(remaining_header_slots);

        for (sig_index, sig) in self.signatures.iter().enumerate() {
            let finder = memmem::Finder::new(sig.header);
            for pos in finder.find_iter(&self.buffer) {
                let abs = self.buffer_start_offset + pos as u64;
                // Skip if already open.
                if self.open_headers.iter().any(|h| h.absolute_offset == abs) {
                    continue;
                }
                if new_headers.len() >= remaining_header_slots {
                    self.headers_dropped_by_limit = self.headers_dropped_by_limit.saturating_add(1);
                    continue;
                }
                new_headers.push((abs, sig_index));
            }
        }

        // 2. Add them to the open list, with the search resuming just
        //    past the end of the header. `resume_at` is relative to
        //    the start of the current buffer.
        for (abs, sig_index) in new_headers {
            let sig = &self.signatures[sig_index];
            let header_len = sig.header.len();
            let pos_in_buffer = (abs - self.buffer_start_offset) as usize;
            let resume_at = pos_in_buffer + header_len;

            let preview_end = pos_in_buffer
                .saturating_add(self.header_preview_len)
                .min(buffer_len);
            let preview = self.buffer[pos_in_buffer..preview_end].to_vec();

            self.open_headers.push(OpenHeader {
                absolute_offset: abs,
                signature_index: sig_index,
                resume_at,
                preview,
            });
        }

        // 3. For each open header, try to close it against the current
        //    buffer, starting from `resume_at`.
        let mut still_open: Vec<OpenHeader> = Vec::with_capacity(self.open_headers.len());
        let mut closed: Vec<(usize, u64, u64, Vec<u8>)> = Vec::new(); // (sig_idx, abs_off, length, preview)

        for oh in self.open_headers.drain(..) {
            let sig = &self.signatures[oh.signature_index];
            let Some(footer) = sig.footer else {
                // Signature without a footer cannot be closed. Record
                // as truncated and drop.
                self.truncated_headers += 1;
                continue;
            };

            // Bound the search: the footer must begin no later than
            // `header_offset + max_size - footer_len`.
            let max_abs_end = oh
                .absolute_offset
                .saturating_add(sig.max_size)
                .saturating_sub(footer.len() as u64);
            let max_pos_in_buffer = max_abs_end.saturating_sub(self.buffer_start_offset) as usize;
            let search_end = max_pos_in_buffer.min(buffer_len);

            if oh.resume_at >= search_end {
                // No room left in this buffer for the footer. Check
                // whether the header has exceeded max_size.
                let bytes_consumed = self
                    .buffer_start_offset
                    .saturating_add(buffer_len as u64)
                    .saturating_sub(oh.absolute_offset);
                if bytes_consumed >= sig.max_size {
                    self.truncated_headers += 1;
                    // Drop it.
                } else {
                    still_open.push(oh);
                }
                continue;
            }

            let search_region = &self.buffer[oh.resume_at..search_end];
            let footer_finder = memmem::Finder::new(footer);
            match footer_finder.find(search_region) {
                Some(rel) => {
                    let footer_pos = oh.resume_at + rel;
                    let end = footer_pos + footer.len();
                    let length =
                        (self.buffer_start_offset + end as u64).saturating_sub(oh.absolute_offset);

                    if length < sig.min_size {
                        // Found footer but total length is below the
                        // floor; treat as truncated, not a candidate.
                        self.truncated_headers += 1;
                    } else {
                        closed.push((oh.signature_index, oh.absolute_offset, length, oh.preview));
                    }
                }
                None => {
                    // Footer not yet in the buffer. If we have not hit
                    // max_size, keep the header open and advance
                    // resume_at to the end of the current buffer so the
                    // next window continues from there.
                    let bytes_consumed = self
                        .buffer_start_offset
                        .saturating_add(buffer_len as u64)
                        .saturating_sub(oh.absolute_offset);
                    if bytes_consumed >= sig.max_size {
                        self.truncated_headers += 1;
                    } else {
                        still_open.push(OpenHeader {
                            absolute_offset: oh.absolute_offset,
                            signature_index: oh.signature_index,
                            resume_at: buffer_len,
                            preview: oh.preview,
                        });
                    }
                }
            }
        }

        // 4. Emit closed candidates.
        for (sig_index, abs_off, length, preview) in closed {
            if self.candidates.len() >= self.max_candidates {
                self.candidate_limit_reached = true;
                continue;
            }
            let sig = &self.signatures[sig_index];
            self.candidates.push(CarvedCandidate {
                id: CandidateId::new(),
                format: sig.format.to_string(),
                offset: abs_off,
                length,
                header_preview: preview,
                sha256: None,
            });
        }

        self.open_headers = still_open;

        // 5. On EOF, any remaining open headers are truncated.
        if eof {
            self.truncated_headers += self.open_headers.len() as u64;
            self.open_headers.clear();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn empty_source_yields_no_candidates() {
        let report = scan_source(Cursor::new(b""), &ScanOptions::default()).unwrap();
        assert_eq!(report.candidates_found, 0);
        assert_eq!(report.bytes_scanned, 0);
    }

    #[test]
    fn source_with_no_signatures_yields_no_candidates() {
        let data = vec![0u8; 10_000];
        let report = scan_source(Cursor::new(data), &ScanOptions::default()).unwrap();
        assert_eq!(report.candidates_found, 0);
    }

    #[test]
    fn window_too_small_is_rejected() {
        let opts = ScanOptions {
            window_size: 2,
            header_preview_len: 16,
            ..ScanOptions::default()
        };
        let err = scan_source(Cursor::new(b"hello"), &opts).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    }

    #[test]
    fn finds_a_jpeg_header_and_footer() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
        data.extend_from_slice(&[0x00; 100]);
        data.extend_from_slice(&[0xFF, 0xD9]);

        let report = scan_source(Cursor::new(data), &ScanOptions::default()).unwrap();
        assert_eq!(report.candidates_found, 1);
        assert_eq!(report.candidates[0].format, "jpeg");
        assert_eq!(report.candidates[0].offset, 0);
        assert_eq!(report.candidates[0].length, 105);
    }

    #[test]
    fn header_without_footer_is_truncated_not_a_candidate() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
        data.extend_from_slice(&[0x00; 100]);

        let report = scan_source(Cursor::new(data), &ScanOptions::default()).unwrap();
        assert_eq!(report.candidates_found, 0);
        assert_eq!(report.truncated_headers, 1);
    }

    #[test]
    fn byte_limit_is_reported_and_exact_eof_is_not_a_limit() {
        let options = ScanOptions {
            max_scan_bytes: 16,
            ..ScanOptions::default()
        };
        let report = scan_source(Cursor::new(vec![0xAA; 32]), &options).unwrap();
        assert_eq!(report.bytes_scanned, 16);
        assert!(report.scan_limit_reached);

        let exact = scan_source(Cursor::new(vec![0xAA; 16]), &options).unwrap();
        assert_eq!(exact.bytes_scanned, 16);
        assert!(!exact.scan_limit_reached);
    }

    #[test]
    fn candidate_and_open_header_limits_are_reported() {
        let mut data = Vec::new();
        for _ in 0..2 {
            data.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
            data.extend_from_slice(&[0x00; 20]);
            data.extend_from_slice(&[0xFF, 0xD9]);
        }
        let options = ScanOptions {
            max_candidates: 1,
            ..ScanOptions::default()
        };
        let report = scan_source(Cursor::new(data), &options).unwrap();
        assert_eq!(report.candidates_found, 1);
        assert!(report.candidate_limit_reached);

        let mut headers = Vec::new();
        for _ in 0..4 {
            headers.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
            headers.extend_from_slice(&[0x00; 8]);
        }
        let options = ScanOptions {
            max_open_headers: 1,
            ..ScanOptions::default()
        };
        let report = scan_source(Cursor::new(headers), &options).unwrap();
        assert!(report.headers_dropped_by_limit > 0);
    }

    #[test]
    fn dense_signatures_are_capped_during_detection() {
        let mut data = Vec::with_capacity(64 * 1024);
        for _ in 0..3_000 {
            data.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
            data.extend_from_slice(&[0; 16]);
            data.extend_from_slice(&[0xFF, 0xD9]);
        }
        let options = ScanOptions {
            window_size: 64 * 1024,
            max_open_headers: 32,
            max_candidates: 64,
            ..ScanOptions::default()
        };
        let first = scan_source(Cursor::new(&data), &options).unwrap();
        let second = scan_source(Cursor::new(&data), &options).unwrap();
        assert_eq!(first.candidates_found, 32);
        assert_eq!(first.headers_dropped_by_limit, 3_000 - 32);
        assert_eq!(
            first.candidates.iter().map(|candidate| candidate.offset).collect::<Vec<_>>(),
            second.candidates.iter().map(|candidate| candidate.offset).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn options_cannot_raise_absolute_resource_ceilings() {
        let options = ScanOptions {
            max_open_headers: super::MAX_OPEN_HEADERS + 1,
            ..ScanOptions::default()
        };
        assert!(scan_source(Cursor::new(b""), &options).is_err());
        let options = ScanOptions {
            header_preview_len: super::MAX_HEADER_PREVIEW_LEN + 1,
            ..ScanOptions::default()
        };
        assert!(scan_source(Cursor::new(b""), &options).is_err());
        let options = ScanOptions {
            max_scan_bytes: super::MAX_SCAN_BYTES + 1,
            ..ScanOptions::default()
        };
        assert!(scan_source(Cursor::new(b""), &options).is_err());
        let options = ScanOptions {
            max_candidates: super::MAX_CANDIDATES + 1,
            ..ScanOptions::default()
        };
        assert!(scan_source(Cursor::new(b""), &options).is_err());
    }

    #[test]
    fn checkpoint_reports_monotonic_read_progress() {
        let bytes = vec![0u8; 256];
        let mut checkpoints = Vec::new();
        scan_source_with_checkpoint(
            Cursor::new(bytes),
            &ScanOptions {
                window_size: 64,
                ..ScanOptions::default()
            },
            |count| {
                checkpoints.push(count);
                true
            },
        )
        .unwrap();

        assert!(checkpoints.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(checkpoints.last(), Some(&256));
    }
}
