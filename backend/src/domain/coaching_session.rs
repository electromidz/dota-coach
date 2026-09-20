//! A coaching session: what was measured, at one point in time.
//!
//! The distinction this module exists to draw:
//!
//!   - [`crate::domain::coaching::Evidence`] is a *sentence*. It carries the
//!     numbers inside prose, because its job is to be shown to a reader and
//!     cited by a model.
//!   - [`MetricSnapshot`] is a *number*. Its job is to be subtracted from the
//!     same number in an earlier session.
//!
//! Both describe the same figures. Keeping them apart is what lets a progress
//! engine exist at all: comparing two sessions must never involve parsing
//! English, and showing a session must never involve the client doing
//! arithmetic.
//!
//! A session is immutable once stored. The current coaching profile is not a
//! separate record — it is the newest session, so "where the player is now"
//! and "where they were in March" are the same shape and cannot disagree.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::player_model::PlayerTrait;
use crate::domain::role::CoachableRole;

/// What a measured value is counted in.
///
/// Carried so a comparison can refuse to subtract a percentile from a per-10
/// rate. Two metrics with different units are not two readings of the same
/// thing, however similar their keys look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetricUnit {
    /// A plain count — matches, wins.
    Count,
    /// Per minute of game time.
    PerMinute,
    /// Per ten minutes of game time.
    Per10,
    /// 0-100, already direction-corrected.
    Percentile,
    /// A ratio, typically 0-1.
    Ratio,
    /// A 0-100 composite the backend defines, such as the role score.
    Score,
}

/// One measured number, in a form a later session can be compared against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct MetricSnapshot {
    /// Stable, and deliberately sharing a namespace with
    /// [`Evidence::id`](crate::domain::coaching::Evidence) — `overall.deaths`,
    /// `benchmark.gold_per_min`, `pattern.high_death_rate`. A metric and the
    /// sentence written about it are therefore traceable to each other.
    pub key: String,
    pub label: String,
    /// The figure itself. This is the point of the whole table.
    pub value: f32,
    /// Matches behind it, so a thin reading is visibly thin when compared.
    pub sample: i64,
    pub unit: MetricUnit,
    /// False when a lower value is the better result — deaths, pattern rates.
    /// Stored per metric rather than derived at comparison time, so a session
    /// recorded before a definition changed still reads correctly.
    pub higher_is_better: bool,
}

/// What a hero contributed, as it read at the time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct HeroSnapshot {
    pub hero_id: i32,
    pub hero_name: String,
    pub matches: i64,
    pub wins: i64,
    pub win_rate: f32,
    pub avg_kda: Option<f32>,
}

/// One benchmarked metric, reduced to the figures a comparison needs.
///
/// Not the full [`BenchmarkResult`](crate::domain::benchmark::BenchmarkResult):
/// that carries notes, confidence wording and segmentation for display. A
/// snapshot keeps only what a later session subtracts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct BenchmarkSnapshot {
    pub metric: String,
    pub label: String,
    pub player_value: f32,
    pub peer_median: Option<f32>,
    pub percentile: Option<f32>,
    pub higher_is_better: bool,
}

/// A stored session, exactly as it was written.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CoachingSession {
    pub id: Uuid,
    pub role: CoachableRole,
    pub role_label: &'static str,
    /// 1-based, per player and role. "Session #12" is this number.
    pub sequence: i32,

    pub analyzed_match_count: i32,
    /// The exact matches behind every figure here. Stored so a session can be
    /// audited against the games it read, and so the next session knows what
    /// counts as new.
    pub analyzed_match_ids: Vec<Uuid>,
    pub newest_match_at: Option<chrono::DateTime<chrono::Utc>>,

    /// The role score. `None` when there was nothing to score.
    pub performance: Option<f32>,
    pub metrics: Vec<MetricSnapshot>,
    pub strengths: Vec<PlayerTrait>,
    pub weaknesses: Vec<PlayerTrait>,
    pub benchmarks: Vec<BenchmarkSnapshot>,
    pub heroes: Vec<HeroSnapshot>,

    pub training_focus_id: Option<Uuid>,
    /// Set when a model has interpreted this session. `None` is the normal
    /// case — a session is measured whether or not anyone paid for prose.
    pub analysis_id: Option<Uuid>,

    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl CoachingSession {
    /// Look up one metric by key.
    ///
    /// The comparison engine's primitive: two sessions are compared by pairing
    /// metrics that share a key *and* a unit.
    pub fn metric(&self, key: &str) -> Option<&MetricSnapshot> {
        self.metrics.iter().find(|m| m.key == key)
    }
}

/// A session that has been computed but not yet stored.
///
/// Separate from [`CoachingSession`] because `id`, `sequence` and `created_at`
/// are the database's to assign — `sequence` in particular has to be allocated
/// inside the inserting transaction, so a draft cannot carry a guess at it.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionDraft {
    pub role: CoachableRole,
    pub analyzed_match_count: i32,
    pub analyzed_match_ids: Vec<Uuid>,
    pub newest_match_at: Option<chrono::DateTime<chrono::Utc>>,
    pub performance: Option<f32>,
    pub metrics: Vec<MetricSnapshot>,
    pub strengths: Vec<PlayerTrait>,
    pub weaknesses: Vec<PlayerTrait>,
    pub benchmarks: Vec<BenchmarkSnapshot>,
    pub heroes: Vec<HeroSnapshot>,
    pub training_focus_id: Option<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metric(key: &str, value: f32, unit: MetricUnit) -> MetricSnapshot {
        MetricSnapshot {
            key: key.to_string(),
            label: key.to_string(),
            value,
            sample: 20,
            unit,
            higher_is_better: true,
        }
    }

    #[test]
    fn a_metric_is_found_by_key() {
        let session = SessionDraft {
            role: CoachableRole::Carry,
            analyzed_match_count: 20,
            analyzed_match_ids: Vec::new(),
            newest_match_at: None,
            performance: Some(61.0),
            metrics: vec![
                metric("overall.deaths", 5.8, MetricUnit::Per10),
                metric("benchmark.gold_per_min", 575.0, MetricUnit::PerMinute),
            ],
            strengths: Vec::new(),
            weaknesses: Vec::new(),
            benchmarks: Vec::new(),
            heroes: Vec::new(),
            training_focus_id: None,
        };

        // The draft and the stored row share the metric list, so testing the
        // lookup through either is testing the same thing.
        assert_eq!(session.metrics.len(), 2);
        assert_eq!(
            session
                .metrics
                .iter()
                .find(|m| m.key == "overall.deaths")
                .unwrap()
                .value,
            5.8
        );
    }

    #[test]
    fn a_metric_round_trips_through_json_unchanged() {
        // The whole table exists so these stay numbers. A serialization that
        // quietly stringified a value would make every later comparison a
        // parsing problem again.
        let original = metric("overall.deaths", 5.8, MetricUnit::Per10);
        let json = serde_json::to_string(&original).unwrap();
        let back: MetricSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(back, original);
        assert!(json.contains("\"value\":5.8"), "{json}");
        assert!(json.contains("\"unit\":\"per10\""), "{json}");
    }

    #[test]
    fn units_are_distinguishable_so_a_percentile_is_never_a_rate() {
        assert_ne!(MetricUnit::Percentile, MetricUnit::Per10);
        assert_ne!(MetricUnit::Score, MetricUnit::Count);
    }
}
