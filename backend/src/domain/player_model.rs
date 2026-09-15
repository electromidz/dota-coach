//! The long-term model of a player.
//!
//! The distinction that matters here is between a *reading* and a *pattern*.
//! A reading is what one match says. A pattern is what many matches say, and
//! the spec is explicit that the second must never be inferred from the first:
//!
//! > Do not call something a "recurring pattern" after one match.
//!
//! So every pattern carries the number of matches it was **measurable** in
//! alongside the number it actually **occurred** in. Those are different
//! denominators — most of the laning-phase signals only exist on a parsed
//! replay — and collapsing them would let a pattern seen twice in two parsed
//! games out of forty look like a habit.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::benchmark::Confidence;
use utoipa::ToSchema;

/// A match with its derived metrics, flattened for pattern detection.
///
/// Optional fields are optional because the *input* was unavailable — an
/// unparsed replay, a match detail that never arrived — never because the
/// value was zero. Detectors treat them as "not measurable here" rather than
/// as a passing result.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AnalyzedMatch {
    pub match_id: Uuid,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub hero_id: i32,
    pub hero_name: String,
    pub role: String,
    pub won: bool,
    pub duration_seconds: i32,
    pub gpm: i32,

    pub deaths_per_10: f32,
    pub kill_participation: Option<f32>,
    pub tower_damage: Option<i32>,

    /// Parsed replays only.
    pub last_hits_at_10: Option<i32>,
    pub gold_at_10: Option<i32>,
    pub bkb_seconds: Option<i32>,
}

impl AnalyzedMatch {
    /// Whether the player was a core in this match.
    ///
    /// Several detectors only make sense for one or the other: 40 last hits at
    /// ten minutes is a poor laning stage for a mid and an irrelevant number
    /// for a hard support.
    pub fn is_core(&self) -> bool {
        matches!(self.role.as_str(), "Carry" | "Mid" | "Offlane")
    }

    pub fn minutes(&self) -> f32 {
        (self.duration_seconds as f32 / 60.0).max(1.0)
    }
}

/// Where a pattern stands now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PatternStatus {
    /// Still happening at the rate that first flagged it.
    Active,
    /// Still present across the whole history, but markedly rarer lately.
    /// The evidence for it has not vanished, so it is not called resolved.
    Improving,
    /// Detected before, and no longer meeting the threshold.
    Resolved,
}

impl PatternStatus {
    pub fn slug(self) -> &'static str {
        match self {
            PatternStatus::Active => "active",
            PatternStatus::Improving => "improving",
            PatternStatus::Resolved => "resolved",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(PatternStatus::Active),
            "improving" => Some(PatternStatus::Improving),
            "resolved" => Some(PatternStatus::Resolved),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PatternStatus::Active => "Active",
            PatternStatus::Improving => "Improving",
            PatternStatus::Resolved => "Resolved",
        }
    }
}

/// Something the player does repeatedly, with the arithmetic behind it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RecurringPattern {
    /// Stable slug — `high_death_rate`. Stored, so it must not change casually.
    pub id: String,
    pub label: String,
    /// What the detector looks for, in plain words.
    pub description: String,

    /// Matches where the condition held.
    pub occurrences: i64,
    /// Matches where it could be *checked at all*. Never the career total.
    pub measured: i64,
    /// `occurrences / measured`, 0-1.
    pub rate: f32,

    /// The same rate across the most recent measurable matches, when there
    /// are enough of them to mean anything.
    pub recent_rate: Option<f32>,
    pub recent_measured: i64,

    pub status: PatternStatus,
    pub status_label: &'static str,
    /// How much weight the rate can bear, from `measured`.
    pub confidence: Confidence,

    /// A sentence composed from the numbers above.
    pub statement: String,
    /// A few matches where it happened, newest first, so a user can go look.
    pub examples: Vec<Uuid>,

    pub first_seen_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
    /// When this backend first recorded the pattern. Survives recomputation,
    /// which is what makes "you have been doing this since March" possible.
    pub first_detected_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Whether a trait is something the player does well or badly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TraitKind {
    Strength,
    Weakness,
}

impl TraitKind {
    pub fn slug(self) -> &'static str {
        match self {
            TraitKind::Strength => "strength",
            TraitKind::Weakness => "weakness",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "strength" => Some(TraitKind::Strength),
            "weakness" => Some(TraitKind::Weakness),
            _ => None,
        }
    }
}

/// What the trait was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TraitSource {
    /// A percentile from the benchmark engine.
    Benchmark,
    /// The shape of the player's hero pool.
    HeroPool,
    /// The recent window.
    Form,
}

impl TraitSource {
    pub fn slug(self) -> &'static str {
        match self {
            TraitSource::Benchmark => "benchmark",
            TraitSource::HeroPool => "hero_pool",
            TraitSource::Form => "form",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "benchmark" => Some(TraitSource::Benchmark),
            "hero_pool" => Some(TraitSource::HeroPool),
            "form" => Some(TraitSource::Form),
            _ => None,
        }
    }
}

/// One thing the model believes about the player.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PlayerTrait {
    pub kind: TraitKind,
    pub source: TraitSource,
    /// Stable key — `benchmark.gold_per_min`, `hero_pool.signature`.
    pub key: String,
    pub label: String,
    pub statement: String,
    pub sample: i64,
    pub confidence: Confidence,
}

/// How much the player leans on a role.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RoleAffinity {
    pub role: String,
    pub matches: i64,
    /// Share of the player's matches, 0-1.
    pub share: f32,
    pub win_rate: f32,
}

/// The recent window, as its own reading.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RecentForm {
    pub matches: i64,
    pub wins: i64,
    pub win_rate: Option<f32>,
    /// Positive for a winning streak, negative for a losing one.
    pub streak: i32,
}

/// How well this backend knows the player.
///
/// The spec's own framing: a new user gets generic analysis, and a player with
/// a hundred matches gets something personal. This is the dial that says which
/// one is currently honest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelConfidence {
    /// Too little history for anything but generic advice.
    Sparse,
    /// Enough for trends, not enough to be sure of them.
    Developing,
    /// Enough history for the model's claims to carry weight.
    Established,
}

impl ModelConfidence {
    /// Thresholds in matches. Deliberately generous at the low end: ten games
    /// is where a win rate stops being a coin flip, not where it becomes true.
    pub fn for_matches(matches: i64) -> Self {
        if matches < 10 {
            ModelConfidence::Sparse
        } else if matches < 30 {
            ModelConfidence::Developing
        } else {
            ModelConfidence::Established
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            ModelConfidence::Sparse => "sparse",
            ModelConfidence::Developing => "developing",
            ModelConfidence::Established => "established",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "sparse" => Some(ModelConfidence::Sparse),
            "developing" => Some(ModelConfidence::Developing),
            "established" => Some(ModelConfidence::Established),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ModelConfidence::Sparse => "Sparse",
            ModelConfidence::Developing => "Developing",
            ModelConfidence::Established => "Established",
        }
    }

    /// What this means for the advice built on it.
    pub fn caveat(self) -> &'static str {
        match self {
            ModelConfidence::Sparse => {
                "Fewer than 10 stored matches, so this is a first impression rather than a model."
            }
            ModelConfidence::Developing => {
                "Enough matches to see trends, not enough to be confident in them."
            }
            ModelConfidence::Established => "Enough history for these claims to carry weight.",
        }
    }
}

/// Everything the backend believes about one player.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PlayerModel {
    /// Bumped when a detector or a trait rule changes, so stored rows are
    /// identifiable as having come from an older definition.
    pub model_version: i32,
    pub matches_analyzed: i64,
    pub confidence: ModelConfidence,
    pub confidence_label: &'static str,
    pub confidence_caveat: &'static str,

    pub strengths: Vec<PlayerTrait>,
    pub weaknesses: Vec<PlayerTrait>,
    pub preferred_roles: Vec<RoleAffinity>,
    /// Patterns currently meeting the threshold, worst first.
    pub patterns: Vec<RecurringPattern>,
    /// Patterns that used to, kept because "you fixed this" is worth knowing.
    pub resolved_patterns: Vec<RecurringPattern>,
    pub recent_form: RecentForm,

    pub computed_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_confidence_follows_the_stored_history() {
        assert_eq!(ModelConfidence::for_matches(0), ModelConfidence::Sparse);
        assert_eq!(ModelConfidence::for_matches(9), ModelConfidence::Sparse);
        assert_eq!(
            ModelConfidence::for_matches(10),
            ModelConfidence::Developing
        );
        assert_eq!(
            ModelConfidence::for_matches(29),
            ModelConfidence::Developing
        );
        assert_eq!(
            ModelConfidence::for_matches(100),
            ModelConfidence::Established
        );
    }

    #[test]
    fn every_stored_enum_round_trips_through_its_slug() {
        for status in [
            PatternStatus::Active,
            PatternStatus::Improving,
            PatternStatus::Resolved,
        ] {
            assert_eq!(PatternStatus::parse(status.slug()), Some(status));
        }
        for kind in [TraitKind::Strength, TraitKind::Weakness] {
            assert_eq!(TraitKind::parse(kind.slug()), Some(kind));
        }
        for source in [
            TraitSource::Benchmark,
            TraitSource::HeroPool,
            TraitSource::Form,
        ] {
            assert_eq!(TraitSource::parse(source.slug()), Some(source));
        }
        for confidence in [
            ModelConfidence::Sparse,
            ModelConfidence::Developing,
            ModelConfidence::Established,
        ] {
            assert_eq!(ModelConfidence::parse(confidence.slug()), Some(confidence));
        }
    }

    #[test]
    fn core_roles_are_distinguished_from_supports() {
        let core = |role: &str| {
            AnalyzedMatch {
                match_id: Uuid::nil(),
                started_at: chrono::Utc::now(),
                hero_id: 35,
                hero_name: "Luna".into(),
                role: role.into(),
                won: true,
                duration_seconds: 2_400,
                gpm: 500,
                deaths_per_10: 1.0,
                kill_participation: None,
                tower_damage: None,
                last_hits_at_10: None,
                gold_at_10: None,
                bkb_seconds: None,
            }
            .is_core()
        };

        assert!(core("Carry"));
        assert!(core("Mid"));
        assert!(core("Offlane"));
        assert!(!core("Support"));
        assert!(!core("Hard Support"));
    }
}
