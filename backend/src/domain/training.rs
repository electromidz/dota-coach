//! Training focus and progress.
//!
//! One focus at a time. The spec is blunt about why — a coach who hands a
//! player ten weaknesses has given them nothing to do on Tuesday night — and
//! equally blunt that the one is not simply the lowest statistic: it is chosen
//! from benchmark gap, pattern history, recent performance, impact, confidence
//! and recency together.
//!
//! The other half of the phase is that a focus has to be *checkable*. Every
//! focus here carries a measure, a direction, a starting value and a target,
//! so "is this actually improving?" is arithmetic rather than an opinion.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::benchmark::Confidence;
use utoipa::ToSchema;

/// What a focus is measured by.
///
/// Deliberately small: every variant has to be computable per match from
/// stored data, because that is what makes a progress series possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FocusMeasure {
    DeathsPer10,
    KillParticipation,
    GoldPerMin,
    LastHitsAt10,
    /// The share of matches a named recurring pattern occurs in. The pattern
    /// id travels alongside in [`TrainingFocus::pattern_id`].
    PatternRate,
}

impl FocusMeasure {
    pub const ALL: [FocusMeasure; 5] = [
        FocusMeasure::DeathsPer10,
        FocusMeasure::KillParticipation,
        FocusMeasure::GoldPerMin,
        FocusMeasure::LastHitsAt10,
        FocusMeasure::PatternRate,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            FocusMeasure::DeathsPer10 => "deaths_per_10",
            FocusMeasure::KillParticipation => "kill_participation",
            FocusMeasure::GoldPerMin => "gold_per_min",
            FocusMeasure::LastHitsAt10 => "last_hits_at_10",
            FocusMeasure::PatternRate => "pattern_rate",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.slug() == value)
    }

    pub fn label(self) -> &'static str {
        match self {
            FocusMeasure::DeathsPer10 => "Deaths per 10 minutes",
            FocusMeasure::KillParticipation => "Kill participation",
            FocusMeasure::GoldPerMin => "Gold per minute",
            FocusMeasure::LastHitsAt10 => "Last hits at 10 minutes",
            FocusMeasure::PatternRate => "How often this happens",
        }
    }

    /// False when a lower value is the better result.
    pub fn higher_is_better(self) -> bool {
        !matches!(self, FocusMeasure::DeathsPer10 | FocusMeasure::PatternRate)
    }

    /// How much moving this measure is worth, 0-100.
    ///
    /// A judgement, stated once and openly rather than buried in a scoring
    /// expression: dying less changes more games than last-hitting slightly
    /// faster, and a recurring habit changes more than either.
    pub fn impact(self) -> f32 {
        match self {
            FocusMeasure::PatternRate => 95.0,
            FocusMeasure::DeathsPer10 => 90.0,
            FocusMeasure::KillParticipation => 75.0,
            FocusMeasure::GoldPerMin => 65.0,
            FocusMeasure::LastHitsAt10 => 60.0,
        }
    }

    /// How the value reads to a person.
    pub fn format(self, value: f32) -> String {
        match self {
            FocusMeasure::KillParticipation | FocusMeasure::PatternRate => {
                format!("{:.0}%", value * 100.0)
            }
            FocusMeasure::DeathsPer10 => format!("{value:.1}"),
            _ => format!("{value:.0}"),
        }
    }
}

/// Where a focus came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FocusSource {
    /// A gap against the peer distribution.
    Benchmark,
    /// A recurring pattern detected across the history.
    Pattern,
}

impl FocusSource {
    pub fn slug(self) -> &'static str {
        match self {
            FocusSource::Benchmark => "benchmark",
            FocusSource::Pattern => "pattern",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "benchmark" => Some(FocusSource::Benchmark),
            "pattern" => Some(FocusSource::Pattern),
            _ => None,
        }
    }
}

/// Where a focus stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FocusStatus {
    Active,
    /// The target was reached across a full recent window.
    Achieved,
    /// Dropped without being reached — the evidence behind it went away.
    Retired,
}

impl FocusStatus {
    pub fn slug(self) -> &'static str {
        match self {
            FocusStatus::Active => "active",
            FocusStatus::Achieved => "achieved",
            FocusStatus::Retired => "retired",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(FocusStatus::Active),
            "achieved" => Some(FocusStatus::Achieved),
            "retired" => Some(FocusStatus::Retired),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FocusStatus::Active => "Active",
            FocusStatus::Achieved => "Achieved",
            FocusStatus::Retired => "Retired",
        }
    }
}

/// One thing to work on, with the arithmetic that makes it checkable.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrainingFocus {
    pub id: Option<Uuid>,
    /// Stable key — `pattern.high_death_rate`, `benchmark.gold_per_min`.
    pub key: String,
    pub title: String,
    /// Why this one and not another, in the player's terms.
    pub why: String,

    pub source: FocusSource,
    pub measure: FocusMeasure,
    pub measure_label: &'static str,
    /// Set when `measure` is [`FocusMeasure::PatternRate`].
    pub pattern_id: Option<String>,
    pub higher_is_better: bool,

    /// Where the player stood when the focus was set.
    pub baseline_value: f32,
    /// What "done" is. Chosen to be reachable, not aspirational.
    pub target_value: f32,
    /// Where they stand now, over the recent window.
    pub current_value: Option<f32>,
    /// 0-1 along the road from baseline to target. Clamped, and `None` when
    /// there is nothing recent to measure.
    pub progress: Option<f32>,
    pub target_met: bool,

    pub status: FocusStatus,
    pub status_label: &'static str,
    /// Deterministic 0-100, and the parts that produced it.
    pub score: f32,
    pub score_parts: Vec<FocusScorePart>,
    pub confidence: Confidence,
    /// Matches behind the baseline.
    pub sample: i64,

    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// One weighted input to the selection score.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FocusScorePart {
    pub key: &'static str,
    pub label: &'static str,
    /// 0-100.
    pub score: f32,
    pub weight: f32,
    pub detail: String,
}

/// One bucket of the progress series.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProgressPoint {
    /// Matches in this bucket that the measure could be read from.
    pub matches: i64,
    pub value: f32,
    /// The newest match in the bucket, so a point can be placed in time.
    pub at: chrono::DateTime<chrono::Utc>,
}

/// A measure over time, oldest first, so it reads left to right.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProgressSeries {
    pub measure: FocusMeasure,
    pub label: &'static str,
    pub higher_is_better: bool,
    pub points: Vec<ProgressPoint>,
    /// Matches per bucket. Reported so a flat line is readable as "ten-match
    /// averages" rather than "ten matches".
    pub window: i64,
    pub target_value: Option<f32>,
}

/// Relative weights of the selection inputs, from `PRODUCT_SPEC.md` §28.
///
/// Configuration, not structure: nothing downstream assumes a particular
/// weighting, and the list of inputs is the spec's, not an invention here.
#[derive(Debug, Clone, Copy)]
pub struct FocusWeights {
    pub gap: f32,
    pub pattern: f32,
    pub recent: f32,
    pub impact: f32,
    pub confidence: f32,
    pub recency: f32,
}

impl Default for FocusWeights {
    fn default() -> Self {
        Self {
            gap: 0.25,
            pattern: 0.20,
            recent: 0.15,
            impact: 0.20,
            confidence: 0.10,
            recency: 0.10,
        }
    }
}

impl FocusWeights {
    pub fn total(&self) -> f32 {
        self.gap + self.pattern + self.recent + self.impact + self.confidence + self.recency
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_round_trip_and_know_their_direction() {
        for measure in FocusMeasure::ALL {
            assert_eq!(FocusMeasure::parse(measure.slug()), Some(measure));
        }

        // The two where less is more.
        assert!(!FocusMeasure::DeathsPer10.higher_is_better());
        assert!(!FocusMeasure::PatternRate.higher_is_better());
        assert!(FocusMeasure::GoldPerMin.higher_is_better());
    }

    #[test]
    fn rates_are_formatted_as_percentages_and_counts_are_not() {
        assert_eq!(FocusMeasure::PatternRate.format(0.42), "42%");
        assert_eq!(FocusMeasure::KillParticipation.format(0.6), "60%");
        assert_eq!(FocusMeasure::DeathsPer10.format(2.44), "2.4");
        assert_eq!(FocusMeasure::GoldPerMin.format(512.6), "513");
    }

    #[test]
    fn every_stored_enum_round_trips() {
        for source in [FocusSource::Benchmark, FocusSource::Pattern] {
            assert_eq!(FocusSource::parse(source.slug()), Some(source));
        }
        for status in [
            FocusStatus::Active,
            FocusStatus::Achieved,
            FocusStatus::Retired,
        ] {
            assert_eq!(FocusStatus::parse(status.slug()), Some(status));
        }
    }

    #[test]
    fn the_default_weights_are_the_spec_inputs_and_sum_to_one() {
        assert!((FocusWeights::default().total() - 1.0).abs() < f32::EPSILON * 4.0);
    }
}
