//! Coaching domain types.
//!
//! The split here is the whole point of the phase:
//!
//!   - [`Evidence`] is **measured**. Every statement is a sentence this
//!     backend composed from its own numbers, and the client renders those
//!     sentences verbatim.
//!   - [`Insight`] is **interpreted**. The model writes the prose, but it may
//!     only point at evidence ids — it never carries a figure of its own that
//!     the backend has not already computed.
//!
//! That is what makes "evidence-based explanation" checkable rather than a
//! description of intent: an insight citing an id that does not exist is
//! dropped before it is ever stored.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::benchmark::Confidence;
use utoipa::ToSchema;

/// What a piece of evidence describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// Career aggregates across every stored match.
    Overall,
    /// The recent window, as its own reading.
    Form,
    /// A peer comparison from the benchmark engine.
    Benchmark,
    /// The player's repertoire and hero fit.
    Hero,
    /// A recurring pattern detected across many matches.
    Pattern,
    /// The player's current training focus.
    Focus,
    /// One specific match.
    Match,
}

/// One measured fact, with a stable id the model can cite.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Evidence {
    /// Stable and human-readable — `benchmark.gold_per_min`, `match.deaths`.
    /// Stability matters: it is what a stored insight refers to.
    pub id: String,
    pub kind: EvidenceKind,
    pub label: String,
    /// The sentence, composed by the backend. This is what the UI shows and
    /// what the model is told it may rely on.
    pub statement: String,
    /// Matches behind the figure, so thin evidence is visibly thin.
    pub sample: i64,
    pub confidence: Confidence,
}

/// The kinds of insight the coaching layer may produce.
///
/// Fixed set, parsed strictly: a model that invents a seventh kind has its
/// insight dropped rather than passed through to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InsightKind {
    Strength,
    Weakness,
    RecurringPattern,
    Recommendation,
    Warning,
    Improvement,
}

impl InsightKind {
    pub const ALL: [InsightKind; 6] = [
        InsightKind::Strength,
        InsightKind::Weakness,
        InsightKind::RecurringPattern,
        InsightKind::Recommendation,
        InsightKind::Warning,
        InsightKind::Improvement,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            InsightKind::Strength => "strength",
            InsightKind::Weakness => "weakness",
            InsightKind::RecurringPattern => "recurring_pattern",
            InsightKind::Recommendation => "recommendation",
            InsightKind::Warning => "warning",
            InsightKind::Improvement => "improvement",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().to_lowercase().replace([' ', '-'], "_");
        Self::ALL.into_iter().find(|k| k.slug() == normalized)
    }

    pub fn label(self) -> &'static str {
        match self {
            InsightKind::Strength => "Strength",
            InsightKind::Weakness => "Weakness",
            InsightKind::RecurringPattern => "Recurring pattern",
            InsightKind::Recommendation => "Recommendation",
            InsightKind::Warning => "Warning",
            InsightKind::Improvement => "Improvement",
        }
    }
}

/// One interpreted observation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Insight {
    pub kind: InsightKind,
    pub kind_label: &'static str,
    pub title: String,
    pub explanation: String,
    /// Evidence ids, every one of which is guaranteed to exist in the analysis
    /// it belongs to. An insight with none is never stored.
    pub evidence: Vec<String>,
}

/// What an analysis is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisScope {
    /// The player's whole stored history.
    Player,
    /// One match, read against that history.
    Match,
}

/// A stored analysis: the evidence that went in, and the interpretation that
/// came out.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CoachingAnalysis {
    pub id: Uuid,
    pub scope: AnalysisScope,
    /// Set only for a match-scoped analysis.
    pub match_id: Option<Uuid>,
    /// The model that produced it, as the provider reported itself.
    pub model: String,
    pub summary: String,
    pub insights: Vec<Insight>,
    /// The exact evidence the model was shown, kept so an old analysis can
    /// still be read against the numbers that produced it.
    pub evidence: Vec<Evidence>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insight_kinds_round_trip_through_their_slug() {
        for kind in InsightKind::ALL {
            assert_eq!(InsightKind::parse(kind.slug()), Some(kind));
        }
    }

    #[test]
    fn insight_kinds_are_parsed_forgivingly_but_not_loosely() {
        // Shapes a model plausibly emits.
        assert_eq!(
            InsightKind::parse("Recurring Pattern"),
            Some(InsightKind::RecurringPattern)
        );
        assert_eq!(
            InsightKind::parse(" recurring-pattern "),
            Some(InsightKind::RecurringPattern)
        );
        // But an invented kind is not coerced into the nearest real one.
        assert_eq!(InsightKind::parse("observation"), None);
        assert_eq!(InsightKind::parse(""), None);
    }
}
