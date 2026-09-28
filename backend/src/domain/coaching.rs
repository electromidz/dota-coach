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
    /// What changed since the previous coaching session.
    ///
    /// The only kind that is a statement about *two* points in time. It is
    /// separated from the rest because the difference matters to a reader: a
    /// number under [`EvidenceKind::Overall`] is where the player is, and a
    /// number under this one is how far they moved.
    Progress,
}

/// One measured fact, with a stable id the model can cite.
///
/// `PartialEq` so a test can assert that identical inputs compose identical
/// evidence. That is not a convenience: the stored analysis is keyed by a hash
/// of these statements, so evidence that is not a pure function of its inputs
/// would quietly make every cache lookup a miss.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
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

/// How much a single-match mistake appears to have cost.
///
/// A judgement, and labelled as one. It exists because a list in which every
/// item is equally urgent is a list nobody acts on — the product's job is to say
/// which three things mattered, not to enumerate everything that happened.
///
/// Deliberately two values, not five. A scale finer than "this is the thing to
/// fix" versus "this is worth knowing" would be precision the evidence cannot
/// support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InsightSeverity {
    Major,
    Minor,
}

impl InsightSeverity {
    pub const ALL: [InsightSeverity; 2] = [InsightSeverity::Major, InsightSeverity::Minor];

    pub fn slug(self) -> &'static str {
        match self {
            InsightSeverity::Major => "major",
            InsightSeverity::Minor => "minor",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            InsightSeverity::Major => "Major",
            InsightSeverity::Minor => "Minor",
        }
    }

    /// `None` for anything that is not one of the two. Never coerced to a
    /// default: an unrecognised severity means the model did not answer the
    /// question, and inventing "minor" on its behalf is a claim of its own.
    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().to_lowercase();
        Self::ALL.into_iter().find(|s| s.slug() == normalized)
    }
}

/// One interpreted observation.
///
/// # The two shapes
///
/// `explanation` is the original, and is what a player-wide or role analysis
/// produces: one paragraph of interpretation.
///
/// Single-match analysis asks the same model for the same claim split into three
/// — what happened, why it mattered, what to do instead — because those are
/// three different questions and a paragraph answering all of them usually
/// answers the third one worst. The fields are optional rather than a second
/// type: an insight is an insight, and a stored analysis written before the split
/// existed must keep deserializing.
///
/// Exactly one of the two shapes has to be present; the validator drops an
/// insight carrying neither.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Insight {
    pub kind: InsightKind,
    pub kind_label: &'static str,
    pub title: String,
    /// The single-paragraph form. May be empty when the three fields below carry
    /// the interpretation instead.
    #[serde(default)]
    pub explanation: String,
    /// How much this appears to have cost. `None` when the model did not say,
    /// which is not the same as "minor".
    #[serde(default)]
    pub severity: Option<InsightSeverity>,
    /// The moment this is about, as `m:ss`.
    ///
    /// Verified, not trusted: a timestamp that does not appear verbatim in the
    /// cited evidence is stripped before the insight is stored. An invented
    /// timestamp is the single most convincing kind of fabrication this pipeline
    /// can emit, because it looks exactly like a reading from a replay.
    #[serde(default)]
    pub timestamp: Option<String>,
    /// The concrete event.
    #[serde(default)]
    pub what_happened: Option<String>,
    /// The gameplay consequence.
    #[serde(default)]
    pub why_it_matters: Option<String>,
    /// The practical alternative.
    #[serde(default)]
    pub better_action: Option<String>,
    /// Evidence ids, every one of which is guaranteed to exist in the analysis
    /// it belongs to. An insight with none is never stored.
    pub evidence: Vec<String>,
}

/// One step of a training plan.
///
/// The plan is the answer to "so what do I actually do this week", and it is
/// held to the same rule as an insight: every step points at the measured
/// weakness it exists to fix, and may state no figure the backend did not
/// compute. A step that cannot name its evidence is advice about Dota rather
/// than advice about this player, and the product already has enough of that
/// available for free elsewhere.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PlanStep {
    /// 1-based, in the order the model ranked them.
    pub position: u32,
    /// What to work on, short enough to scan.
    pub title: String,
    /// What to actually do about it in the next few games.
    pub action: String,
    /// Evidence ids, every one guaranteed to exist in the parent analysis.
    pub evidence: Vec<String>,
}

/// What an analysis is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisScope {
    /// The player's whole stored history. Only produced before role coaching
    /// existed; kept so stored analyses stay readable.
    Player,
    /// One match, read against the player's record in that match's role.
    Match,
    /// The eligible matches in the one role the player chose to work on.
    Role,
}

/// A stored analysis: the evidence that went in, and the interpretation that
/// came out.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CoachingAnalysis {
    pub id: Uuid,
    pub scope: AnalysisScope,
    /// The role it is about. `None` for a stored analysis that predates role
    /// coaching.
    pub role: Option<crate::domain::role::CoachableRole>,
    pub role_label: Option<&'static str>,
    /// Set only for a match-scoped analysis.
    pub match_id: Option<Uuid>,
    /// The model that produced it, as the provider reported itself.
    pub model: String,
    pub summary: String,
    pub insights: Vec<Insight>,
    /// The training plan, in order. Empty when the model produced none that
    /// survived validation — which is a real answer, not a rendering bug.
    pub plan: Vec<PlanStep>,
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
