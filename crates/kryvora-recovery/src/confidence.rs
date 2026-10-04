//! Deterministic, explainable confidence assessment.

use kryvora_core::Confidence;
use serde::{Deserialize, Serialize};

/// A signal that contributes to the confidence assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceSignal {
    /// The candidate's byte range begins with a valid signature header.
    SignatureValid,
    /// The validator confirmed the structural shape of the format.
    StructuralValid,
    /// Fragment ordering was assessed. The current engine does not
    /// reconstruct fragments, so this signal must not be claimed.
    FragmentConsistent,
    /// The candidate's size is within the format's plausible bounds.
    SizeConsistent,
    /// The format validator accepted the artifact as a valid instance.
    FormatValidated,
    /// Every byte in the artifact traces to a known offset in a known
    /// source.
    SourceTraceable,
    /// The reconstruction is contiguous; no reassembly was inferred.
    ReconstructionCertain,
}

impl ConfidenceSignal {
    /// Human-readable description of the signal.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::SignatureValid => "signature detected",
            Self::StructuralValid => "structure valid",
            Self::FragmentConsistent => "fragments consistent",
            Self::SizeConsistent => "size within bounds",
            Self::FormatValidated => "format validation passed",
            Self::SourceTraceable => "source traceable",
            Self::ReconstructionCertain => "reconstruction certain",
        }
    }
}

/// A single reason for the current confidence level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfidenceReason {
    pub signal: ConfidenceSignal,
    pub satisfied: bool,
    pub note: Option<String>,
}

impl ConfidenceReason {
    #[must_use]
    pub const fn new(signal: ConfidenceSignal, satisfied: bool) -> Self {
        Self {
            signal,
            satisfied,
            note: None,
        }
    }

    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// The full confidence assessment for a recovery result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfidenceAssessment {
    pub level: Confidence,
    pub reasons: Vec<ConfidenceReason>,
}

impl ConfidenceAssessment {
    /// Derive confidence from required evidence gates rather than a
    /// numeric score. Fragment consistency is deliberately not a gate:
    /// this engine handles only contiguous ranges and does not infer
    /// fragmented-file relationships.
    #[must_use]
    pub fn from_reasons(reasons: Vec<ConfidenceReason>) -> Self {
        let is_satisfied = |signal| {
            reasons
                .iter()
                .find(|reason| reason.signal == signal)
                .is_some_and(|reason| reason.satisfied)
        };
        let signature = is_satisfied(ConfidenceSignal::SignatureValid);
        let structure = is_satisfied(ConfidenceSignal::StructuralValid);
        let format = is_satisfied(ConfidenceSignal::FormatValidated);
        let size = is_satisfied(ConfidenceSignal::SizeConsistent);
        let source = is_satisfied(ConfidenceSignal::SourceTraceable);
        let reconstruction = is_satisfied(ConfidenceSignal::ReconstructionCertain);
        let level = if signature && structure && format && size && source && reconstruction {
            Confidence::High
        } else if signature && structure && size && source {
            Confidence::Moderate
        } else if signature && source {
            Confidence::Low
        } else {
            Confidence::Uncertain
        };
        Self { level, reasons }
    }

    /// Build a maximum-confidence assessment: every signal is
    /// satisfied. Used by tests and for the trivially perfect case of
    /// a fully validated, contiguous, single-range artifact.
    #[must_use]
    pub fn all_satisfied() -> Self {
        let reasons = ALL_SIGNALS
            .iter()
            .map(|s| ConfidenceReason::new(*s, true))
            .collect();
        Self::from_reasons(reasons)
    }
}

/// Every signal, in a stable order.
pub const ALL_SIGNALS: &[ConfidenceSignal] = &[
    ConfidenceSignal::SignatureValid,
    ConfidenceSignal::StructuralValid,
    ConfidenceSignal::FragmentConsistent,
    ConfidenceSignal::SizeConsistent,
    ConfidenceSignal::FormatValidated,
    ConfidenceSignal::SourceTraceable,
    ConfidenceSignal::ReconstructionCertain,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn reasons(satisfied: &[ConfidenceSignal]) -> Vec<ConfidenceReason> {
        ALL_SIGNALS
            .iter()
            .map(|signal| ConfidenceReason::new(*signal, satisfied.contains(signal)))
            .collect()
    }

    #[test]
    fn high_requires_all_format_and_source_gates() {
        let a = ConfidenceAssessment::from_reasons(reasons(&[
            ConfidenceSignal::SignatureValid,
            ConfidenceSignal::StructuralValid,
            ConfidenceSignal::SizeConsistent,
            ConfidenceSignal::FormatValidated,
            ConfidenceSignal::SourceTraceable,
            ConfidenceSignal::ReconstructionCertain,
        ]));
        assert_eq!(a.level, Confidence::High);
    }

    #[test]
    fn structurally_sound_but_not_fully_validated_is_moderate() {
        let a = ConfidenceAssessment::from_reasons(reasons(&[
            ConfidenceSignal::SignatureValid,
            ConfidenceSignal::StructuralValid,
            ConfidenceSignal::SizeConsistent,
            ConfidenceSignal::SourceTraceable,
        ]));
        assert_eq!(a.level, Confidence::Moderate);
    }

    #[test]
    fn signature_and_traceability_without_structure_is_low() {
        let a = ConfidenceAssessment::from_reasons(reasons(&[
            ConfidenceSignal::SignatureValid,
            ConfidenceSignal::SourceTraceable,
        ]));
        assert_eq!(a.level, Confidence::Low);
    }

    #[test]
    fn insufficient_signature_or_source_evidence_is_uncertain() {
        let a = ConfidenceAssessment::from_reasons(reasons(&[
            ConfidenceSignal::SignatureValid,
        ]));
        assert_eq!(a.level, Confidence::Uncertain);
    }

    #[test]
    fn all_satisfied_helper_produces_high() {
        assert_eq!(
            ConfidenceAssessment::all_satisfied().level,
            Confidence::High
        );
    }

    #[test]
    fn reasons_are_preserved_in_assessment() {
        let a = ConfidenceAssessment::from_reasons(reasons(&[]));
        assert_eq!(a.reasons.len(), 7);
    }

    #[test]
    fn signal_description_is_stable() {
        assert_eq!(
            ConfidenceSignal::FormatValidated.description(),
            "format validation passed"
        );
    }
}
