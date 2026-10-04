//! Node kinds and typed references.

use kryvora_core::ProvenanceId;
use kryvora_db::repo::ProvenanceKind;
use serde::{Deserialize, Serialize};

/// The kind of a provenance node, mirroring the values allowed by the
/// `provenance_nodes.kind` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Evidence,
    Candidate,
    Job,
    Artifact,
    Report,
}

impl NodeKind {
    #[must_use]
    pub const fn to_db_kind(self) -> ProvenanceKind {
        match self {
            Self::Evidence => ProvenanceKind::Evidence,
            Self::Candidate => ProvenanceKind::Candidate,
            Self::Job => ProvenanceKind::Job,
            Self::Artifact => ProvenanceKind::Artifact,
            Self::Report => ProvenanceKind::Report,
        }
    }

    #[must_use]
    pub const fn from_db_kind(kind: ProvenanceKind) -> Self {
        match kind {
            ProvenanceKind::Evidence => Self::Evidence,
            ProvenanceKind::Candidate => Self::Candidate,
            ProvenanceKind::Job => Self::Job,
            ProvenanceKind::Artifact => Self::Artifact,
            ProvenanceKind::Report => Self::Report,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Evidence => "evidence",
            Self::Candidate => "candidate",
            Self::Job => "job",
            Self::Artifact => "artifact",
            Self::Report => "report",
        }
    }
}

/// A reference to a provenance node: its kind and its object id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceNodeRef {
    pub id: ProvenanceId,
    pub kind: NodeKind,
    pub object_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_roundtrips_through_db_representation() {
        for k in [
            NodeKind::Evidence,
            NodeKind::Candidate,
            NodeKind::Job,
            NodeKind::Artifact,
            NodeKind::Report,
        ] {
            let db = k.to_db_kind();
            assert_eq!(NodeKind::from_db_kind(db), k);
        }
    }

    #[test]
    fn kind_as_str_matches_db_kind_as_str() {
        for k in [
            NodeKind::Evidence,
            NodeKind::Candidate,
            NodeKind::Job,
            NodeKind::Artifact,
            NodeKind::Report,
        ] {
            assert_eq!(k.as_str(), k.to_db_kind().as_str());
        }
    }
}
