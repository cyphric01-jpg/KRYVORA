//! Typed identifiers.
//!
//! All KRYVORA identifiers are UUID-v4 under the hood, but they are
//! distinct types so the compiler prevents passing an `EvidenceId`
//! where a `JobId` is expected.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! typed_id {
    (
        $(#[$meta:meta])*
        $name:ident, $prefix:literal
    ) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generate a fresh random identifier.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Construct from an existing UUID (e.g. when loading from a database).
            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Borrow the underlying UUID.
            #[must_use]
            pub const fn as_uuid(&self) -> Uuid {
                self.0
            }

            /// Stable, human-readable prefix (e.g. `CASE`).
            #[must_use]
            pub const fn prefix(&self) -> &'static str {
                $prefix
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}-{}", $prefix, self.0.simple())
            }
        }
    };
}

typed_id!(
    /// Identifies a forensic case.
    CaseId, "CASE"
);
typed_id!(
    /// Identifies a piece of registered evidence.
    EvidenceId, "EVID"
);
typed_id!(
    /// Identifies a long-running job.
    JobId, "JOB"
);
typed_id!(
    /// Identifies an audit-chain event.
    AuditEventId, "AUDIT"
);
typed_id!(
    /// Identifies a provenance node.
    ProvenanceId, "PROV"
);
typed_id!(
    /// Identifies a recovery result.
    RecoveryResultId, "REC"
);
typed_id!(
    /// Identifies a fragment within a reconstruction.
    FragmentId, "FRAG"
);
typed_id!(
    /// Identifies a generated report.
    ReportId, "REPORT"
);
typed_id!(
    /// Identifies a sanitization operation.
    SanitizationOperationId, "SAN"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        assert_ne!(CaseId::new(), CaseId::new());
        assert_ne!(EvidenceId::new(), EvidenceId::new());
        assert_ne!(JobId::new(), JobId::new());
    }

    #[test]
    fn display_includes_prefix() {
        let id = CaseId::new();
        let s = id.to_string();
        assert!(s.starts_with("CASE-"), "got {s}");
    }

    #[test]
    fn roundtrip_through_uuid() {
        let a = EvidenceId::new();
        let b = EvidenceId::from_uuid(a.as_uuid());
        assert_eq!(a, b);
    }

    #[test]
    fn serde_is_transparent_string() {
        let id = JobId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert!(json.starts_with('"') && json.ends_with('"'));
        let back: JobId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }
}
