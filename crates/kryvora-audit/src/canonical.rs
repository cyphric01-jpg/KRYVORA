//! Canonical byte serialization of an audit event.
//!
//! The bytes produced by [`canonical_bytes`] are the sole input to the
//! SHA-256 that produces `current_hash`. Determinism is the entire
//! point: the same logical event must always produce the same bytes,
//! on every platform, in every run, regardless of `HashMap` iteration
//! order or JSON key order.
//!
//! Format:
//!
//! ```text
//! previous_hash   (64 ASCII hex bytes)   0x00
//! sequence        (big-endian u64, 8 bytes) 0x00
//! event_type      (ASCII snake_case)     0x00
//! created_at      (RFC 3339 UTC)         0x00
//! actor           (UTF-8 or empty)       0x00
//! object_id       (UTF-8 or empty)       0x00
//! job_id          (UTF-8 or empty)       0x00
//! details         (canonical JSON)
//! ```
//!
//! There is no trailing separator. `0x00` cannot appear in any field:
//! hex is `[0-9a-f]`, timestamps are `[0-9T:-Z]`, event types are
//! `[a-z_]`, actor/object_id/job_id are ASCII identifier strings, and
//! canonical JSON never contains a raw NUL byte.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// A snapshot of the fields that are hashed, in their hashed form.
///
/// This struct is deliberately separate from the persisted row. It
/// exists so that `canonical_bytes` can be unit-tested without a
/// database, and so that `verify_chain` can recompute the hash of a
/// row without needing to reconstruct every column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalInput {
    pub previous_hash: String,
    pub sequence: u64,
    pub event_type: String,
    pub created_at: String,
    pub actor: Option<String>,
    pub object_id: Option<String>,
    pub job_id: Option<String>,
    pub details: Value,
}

/// Produce the canonical byte sequence for a hash input.
#[must_use]
pub fn canonical_bytes(input: &CanonicalInput) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);

    out.extend_from_slice(input.previous_hash.as_bytes());
    out.push(0x00);

    out.extend_from_slice(&input.sequence.to_be_bytes());
    out.push(0x00);

    out.extend_from_slice(input.event_type.as_bytes());
    out.push(0x00);

    out.extend_from_slice(input.created_at.as_bytes());
    out.push(0x00);

    out.extend_from_slice(input.actor.as_deref().unwrap_or("").as_bytes());
    out.push(0x00);

    out.extend_from_slice(input.object_id.as_deref().unwrap_or("").as_bytes());
    out.push(0x00);

    out.extend_from_slice(input.job_id.as_deref().unwrap_or("").as_bytes());
    out.push(0x00);

    let canonical = canonical_json(&input.details);
    out.extend_from_slice(canonical.as_bytes());

    out
}

/// Compute the current_hash of a canonical input: lowercase hex SHA-256
/// of [`canonical_bytes`].
#[must_use]
pub fn compute_hash(input: &CanonicalInput) -> String {
    let bytes = canonical_bytes(input);
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    hex::encode(hasher.finalize())
}

/// Canonicalize a `serde_json::Value` to a deterministic string.
///
/// Objects have their keys sorted lexicographically. Arrays preserve
/// order (order is meaningful in JSON). Scalars are serialized as-is.
///
/// This is not RFC 8785. It is a smaller, sufficient canonical form for
/// KRYVORA's purposes: no number-formatting corner cases, no Unicode
/// normalization. It guarantees only that two `Value`s that are equal
/// under `serde_json::Value`'s `Eq` impl produce identical strings.
#[must_use]
pub fn canonical_json(value: &Value) -> String {
    let canonical = canonicalize(value);
    // `serde_json::to_string` on a Value built from BTreeMap-backed
    // objects produces stable output. The only failure mode is a
    // hypothetical serializer error, which cannot occur for a Value
    // that is already in memory.
    serde_json::to_string(&canonical).unwrap_or_else(|_| "null".to_string())
}

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = std::collections::BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k.clone(), canonicalize(v));
            }
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(arr) => Value::Array(arr.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base_input() -> CanonicalInput {
        CanonicalInput {
            previous_hash: kryvora_db::repo::GENESIS_PREVIOUS_HASH.to_string(),
            sequence: 0,
            event_type: "case_created".to_string(),
            created_at: "2026-09-28T12:55:00Z".to_string(),
            actor: Some("examiner".to_string()),
            object_id: Some("CASE-00000000000000000000000000000000".to_string()),
            job_id: None,
            details: json!({"title": "Case Alpha"}),
        }
    }

    #[test]
    fn same_input_produces_same_bytes() {
        let a = canonical_bytes(&base_input());
        let b = canonical_bytes(&base_input());
        assert_eq!(a, b);
    }

    #[test]
    fn same_input_produces_same_hash() {
        let a = compute_hash(&base_input());
        let b = compute_hash(&base_input());
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn object_key_order_does_not_change_hash() {
        let mut a = base_input();
        let mut b = base_input();
        a.details = json!({"alpha": 1, "beta": 2, "gamma": 3});
        b.details = json!({"gamma": 3, "alpha": 1, "beta": 2});
        assert_eq!(compute_hash(&a), compute_hash(&b));
    }

    #[test]
    fn nested_object_key_order_does_not_change_hash() {
        let mut a = base_input();
        let mut b = base_input();
        a.details = json!({"outer": {"alpha": 1, "beta": 2}});
        b.details = json!({"outer": {"beta": 2, "alpha": 1}});
        assert_eq!(compute_hash(&a), compute_hash(&b));
    }

    #[test]
    fn changing_actor_changes_hash() {
        let mut a = base_input();
        let mut b = base_input();
        a.actor = Some("alice".into());
        b.actor = Some("bob".into());
        assert_ne!(compute_hash(&a), compute_hash(&b));
    }

    #[test]
    fn changing_details_changes_hash() {
        let mut a = base_input();
        let mut b = base_input();
        a.details = json!({"x": 1});
        b.details = json!({"x": 2});
        assert_ne!(compute_hash(&a), compute_hash(&b));
    }

    #[test]
    fn none_and_empty_string_actor_are_distinguishable_from_a_real_actor() {
        let mut a = base_input();
        let mut b = base_input();
        let mut c = base_input();
        a.actor = None;
        b.actor = Some(String::new());
        c.actor = Some("x".into());

        // None and empty string both serialize as "" in the canonical
        // form, so they hash the same. This is intentional: the field's
        // absence is not semantically meaningful to the chain, only its
        // content is. A real actor always differs.
        assert_eq!(compute_hash(&a), compute_hash(&b));
        assert_ne!(compute_hash(&a), compute_hash(&c));
    }

    #[test]
    fn changing_sequence_changes_hash() {
        let mut a = base_input();
        let mut b = base_input();
        a.sequence = 0;
        b.sequence = 1;
        assert_ne!(compute_hash(&a), compute_hash(&b));
    }

    #[test]
    fn changing_previous_hash_changes_hash() {
        let a = base_input();
        let mut b = base_input();
        b.previous_hash = "f".repeat(64);
        assert_ne!(compute_hash(&a), compute_hash(&b));
    }

    #[test]
    fn canonical_json_of_empty_object_is_stable() {
        assert_eq!(canonical_json(&json!({})), "{}");
    }

    #[test]
    fn canonical_json_of_nested_structures() {
        let v = json!({"b": [3, {"y": 1, "x": 2}], "a": 1});
        let s = canonical_json(&v);
        assert_eq!(s, r#"{"a":1,"b":[3,{"x":2,"y":1}]}"#);
    }
}
