//! One match, placed against same-rank peers on the same hero.
//!
//! The distinction that shapes every type here: a **percentile of one match**
//! and a **percentile of a career average** are different claims. The first is
//! a fact about a game that was played — "this game's gold per minute beat 78%
//! of peer games on this hero" — and needs no sample floor, because there is
//! nothing being estimated. The second is a claim about the player, and the
//! benchmark engine's [`Confidence`](super::benchmark::Confidence) floor
//! applies to it in full.
//!
//! Both appear side by side, so both are labelled for what they are.

use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::benchmark::{
    BenchmarkContextInfo, BenchmarkMetric, Confidence, ResolvedBracket,
};

/// One figure and where it sits in the peer distribution.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct Reading {
    pub value: f32,
    /// 0-100, direction-corrected, so 90 always means "better than 90% of
    /// peers" — including for deaths, where the raw position is inverted.
    /// `None` when the provider has no distribution for this metric.
    pub percentile: Option<f32>,
}

/// The same, for an average, which carries the sample it rests on.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct AverageReading {
    pub value: f32,
    pub percentile: Option<f32>,
    /// Matches behind `value`.
    pub sample: i64,
    /// How much weight that sample can bear. Applies to this reading only —
    /// the single-match one beside it is not an estimate and is not floored.
    pub confidence: Confidence,
}

/// One metric, as this match and as the player's average, against one peer
/// distribution.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MetricComparison {
    pub metric: BenchmarkMetric,
    pub label: &'static str,
    pub higher_is_better: bool,

    /// `None` when this match has no figure for the metric — an unparsed
    /// replay has no hero-damage number, which is not zero hero damage.
    pub this_match: Option<Reading>,
    /// `None` when the player has no other eligible games on this hero.
    pub hero_average: Option<AverageReading>,

    pub peer_median: Option<f32>,
    /// The 80th percentile: the "top 20%" line.
    pub top_20_value: Option<f32>,
}

/// The one-number summary, and what it is a summary *of*.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct Standing {
    /// Median of this match's per-metric percentiles.
    pub this_match: Option<f32>,
    /// Median of the hero average's per-metric percentiles.
    pub hero_average: Option<f32>,
    /// How many metrics went into `this_match`, or into `hero_average` when
    /// this match could not be compared. Below a handful the median moves a
    /// long way on one metric, and a reader deserves to know that.
    pub metrics_counted: usize,
    /// The peer cohort's size, when the provider reports one. It does not.
    pub peer_sample_size: Option<i64>,
}

/// A metric worth naming, good or bad.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Highlight {
    pub metric: BenchmarkMetric,
    pub label: &'static str,
    pub value: f32,
    pub percentile: f32,
    /// The comparison in words, already carrying its numbers so the client
    /// never recomputes one.
    pub detail: String,
}

/// One past game on this hero, reduced to its standing.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrendPoint {
    pub match_id: Uuid,
    pub dota_match_id: i64,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub won: bool,
    pub standing: f32,
    /// True for the match being viewed, so the chart can mark it without the
    /// client matching ids.
    pub is_current: bool,
}

/// What to do about the weakest metric, with the arithmetic already done.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Suggestion {
    pub metric: BenchmarkMetric,
    pub label: &'static str,
    pub percentile: f32,
    pub player_value: f32,
    pub peer_median: f32,
    /// The gap expressed over this match's duration, for metrics where a
    /// whole-game count is more actionable than a rate. `None` for gold, XP
    /// and damage, which stay rates.
    pub whole_game_delta: Option<f32>,
    pub whole_game_unit: Option<&'static str>,
    pub text: String,
}

/// Everything the match-detail comparison needs, in one response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MatchComparison {
    pub hero_id: i32,
    pub hero_name: String,

    /// Which peer group these numbers actually describe.
    pub bracket: ResolvedBracket,
    /// False when this match's own figures must not be compared at all —
    /// a Turbo game against a distribution drawn from ranked pubs. The raw
    /// values are still served; the percentiles are withheld.
    pub comparable: bool,

    pub standing: Standing,
    pub metrics: Vec<MetricComparison>,

    /// Newest first, the same order the match list uses.
    pub trend: Vec<TrendPoint>,
    /// Standing minus the standing of the previous game on this hero. `None`
    /// when there is no previous game, or this match is not in the trend.
    pub delta_vs_previous: Option<f32>,

    pub pros: Vec<Highlight>,
    pub cons: Vec<Highlight>,
    pub suggestion: Option<Suggestion>,

    /// The shared "what was compared against what" block, identical in shape
    /// to the one `/api/benchmark` returns.
    pub context: BenchmarkContextInfo,
    /// Set when the comparison as a whole is degraded rather than any one
    /// metric — a provider outage, or an ineligible game mode.
    pub note: Option<String>,
}
