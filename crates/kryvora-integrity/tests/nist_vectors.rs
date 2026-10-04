#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! NIST FIPS 180-4 test vectors for SHA-256.
//!
//! Source: FIPS PUB 180-4, Appendix B, plus the well-known `abc` vector.
//! These are the canonical short-message vectors. The `abc` vector is
//! repeated here as an integration test to guarantee the public API
//! surface (not just the internal function) produces them.

use kryvora_integrity::{calculate_hash, Algorithm, Hash};
use std::io::Cursor;
use time::OffsetDateTime;

fn h(input: &[u8]) -> String {
    calculate_hash(Cursor::new(input))
        .unwrap()
        .digest_hex()
        .to_owned()
}

#[test]
fn fips_180_4_empty() {
    assert_eq!(
        h(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn fips_180_4_abc() {
    assert_eq!(
        h(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn fips_180_4_two_block_message() {
    let msg = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    assert_eq!(
        h(msg),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn fips_180_4_million_a() {
    let million_a = vec![b'a'; 1_000_000];
    assert_eq!(
        h(&million_a),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}

#[test]
fn hash_carries_algorithm_and_size() {
    let h = calculate_hash(Cursor::new(b"abc")).unwrap();
    assert_eq!(h.algorithm(), Algorithm::Sha256);
    assert_eq!(h.input_size(), 3);
}

#[test]
fn hash_serde_roundtrip() {
    let h = calculate_hash(Cursor::new(b"abc")).unwrap();
    let json = serde_json::to_string(&h).unwrap();
    let back: Hash = serde_json::from_str(&json).unwrap();
    assert_eq!(h.digest_hex(), back.digest_hex());
    assert_eq!(h.input_size(), back.input_size());
    assert_eq!(h.algorithm(), back.algorithm());
    assert_eq!(h, back);
}

#[test]
fn hash_rejects_invalid_hex_via_public_api() {
    let err = Hash::new(
        Algorithm::Sha256,
        "not-a-hex-string",
        0,
        OffsetDateTime::now_utc(),
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("invalid length") || msg.contains("non-hex"));
}
