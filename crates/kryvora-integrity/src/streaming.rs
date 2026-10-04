//! Streaming SHA-256 over any [`std::io::Read`].

use crate::hash::{Algorithm, Hash};
use kryvora_core::{Error, IntegrityState, Result};
use sha2::{Digest, Sha256};
use std::io::Read;
use time::OffsetDateTime;

/// Default buffer size for streaming hashes.
///
/// 1 MiB is large enough to saturate disk I/O on typical hardware and
/// small enough that hundreds of concurrent hashes remain memory-safe.
pub const DEFAULT_BUFFER_SIZE: usize = 1024 * 1024;

/// Compute the SHA-256 digest of everything `reader` yields.
///
/// The reader is consumed to end-of-stream. Memory usage is bounded by
/// `buffer_size` bytes plus the hasher state, regardless of input size.
///
/// # Errors
///
/// Returns [`Error::Io`] if the underlying reader fails. Returns
/// [`Error::Internal`] if the hasher somehow produces a digest of the
/// wrong length (this is unreachable with `sha2`, but we do not
/// `unwrap`).
pub fn calculate_hash<R: Read>(reader: R) -> Result<Hash> {
    calculate_hash_with_buffer(reader, DEFAULT_BUFFER_SIZE)
}

/// Same as [`calculate_hash`], but with a caller-chosen buffer size.
///
/// `buffer_size` must be non-zero; if zero is supplied, the default is
/// used. This avoids a `panic` in `vec![0; 0]` corner cases while
/// keeping the API total.
pub fn calculate_hash_with_buffer<R: Read>(mut reader: R, buffer_size: usize) -> Result<Hash> {
    let buffer_size = if buffer_size == 0 {
        DEFAULT_BUFFER_SIZE
    } else {
        buffer_size
    };

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; buffer_size];
    let mut total: u64 = 0;

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        total = total
            .checked_add(n as u64)
            .ok_or_else(|| Error::Internal("input size overflowed u64".into()))?;
    }

    let digest = hasher.finalize();
    let digest_hex = hex::encode(digest);

    let hash = Hash::new(
        Algorithm::Sha256,
        digest_hex,
        total,
        OffsetDateTime::now_utc(),
    )
    .map_err(|e| Error::Internal(format!("hasher produced invalid digest: {e}")))?;

    Ok(hash)
}

/// Recompute the hash of `reader` and compare it against `expected`.
///
/// This is the primary forensic verification entry point. It answers the
/// question "does this input still match the recorded hash?" with an
/// explicit [`IntegrityState`]:
///
/// * [`IntegrityState::Verified`] — digest and input size both match.
/// * [`IntegrityState::Mismatch`] — digest differs, or input size differs.
/// * [`IntegrityState::Failed`] — the input could not be read to
///   completion (I/O error partway through).
///
/// Note that [`IntegrityState::Unknown`] and [`IntegrityState::Pending`]
/// are never returned by this function. They belong to the *record* of
/// an evidence item before verification has been attempted.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if `expected` is zero-length. All
/// other failure modes are folded into the returned [`IntegrityState`].
pub fn verify_hash<R: Read>(reader: R, expected: &Hash) -> Result<IntegrityState> {
    match calculate_hash(reader) {
        Ok(actual) => {
            if actual.digest_matches(expected) && actual.input_size() == expected.input_size() {
                Ok(IntegrityState::Verified)
            } else {
                Ok(IntegrityState::Mismatch)
            }
        }
        Err(Error::Io(_)) => Ok(IntegrityState::Failed),
        Err(other) => Err(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn hash_of(bytes: &[u8]) -> Hash {
        calculate_hash(Cursor::new(bytes)).unwrap()
    }

    #[test]
    fn empty_input_matches_known_vector() {
        // SHA-256("") =
        // e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let h = hash_of(b"");
        assert_eq!(
            h.digest_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(h.input_size(), 0);
    }

    #[test]
    fn abc_matches_known_vector() {
        // SHA-256("abc") =
        // ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let h = hash_of(b"abc");
        assert_eq!(
            h.digest_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(h.input_size(), 3);
    }

    #[test]
    fn buffer_size_does_not_change_digest() {
        let data = vec![0xAB_u8; 3 * 1024 * 1024 + 7];
        let a = calculate_hash_with_buffer(Cursor::new(&data), 1).unwrap();
        let b = calculate_hash_with_buffer(Cursor::new(&data), 4096).unwrap();
        let c = calculate_hash_with_buffer(Cursor::new(&data), DEFAULT_BUFFER_SIZE).unwrap();
        assert_eq!(a.digest_hex(), b.digest_hex());
        assert_eq!(b.digest_hex(), c.digest_hex());
        assert_eq!(a.input_size(), data.len() as u64);
    }

    #[test]
    fn verify_returns_verified_on_match() {
        let data = b"forensic payload";
        let expected = hash_of(data);
        let state = verify_hash(Cursor::new(data), &expected).unwrap();
        assert_eq!(state, IntegrityState::Verified);
    }

    #[test]
    fn verify_returns_mismatch_on_different_content() {
        let expected = hash_of(b"original");
        let state = verify_hash(Cursor::new(b"tampered"), &expected).unwrap();
        assert_eq!(state, IntegrityState::Mismatch);
    }

    #[test]
    fn verify_returns_mismatch_on_size_change_even_if_digest_matches() {
        // Construct a Hash whose digest matches the data but whose
        // input_size is wrong. Verification must fail.
        let data = b"abc";
        let real = hash_of(data);
        let tampered_record = Hash::new(
            real.algorithm(),
            real.digest_hex(),
            real.input_size() + 1,
            real.computed_at(),
        )
        .unwrap();
        let state = verify_hash(Cursor::new(data), &tampered_record).unwrap();
        assert_eq!(state, IntegrityState::Mismatch);
    }
}
