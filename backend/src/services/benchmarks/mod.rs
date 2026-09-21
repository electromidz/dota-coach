//! Benchmark engine: how does this player compare to appropriate players?
//!
//! The engine owns the arithmetic and the honesty rules; a [`BenchmarkProvider`]
//! only supplies a distribution. That split is what lets STRATZ replace
//! OpenDota later without the percentile, confidence or gap logic moving.

pub mod opendota;
pub mod percentile;

use std::collections::HashMap;

use async_trait::async_trait;

use crate::domain::benchmark::{
    BenchmarkContext, BenchmarkMetric, BenchmarkResult, Bucket, Confidence, ResolvedBracket,
    Segment,
};

/// The reference distribution for one context, keyed by metric.
pub struct Distribution {
    pub buckets: HashMap<BenchmarkMetric, Vec<Bucket>>,
    /// Dimensions the provider genuinely segmented on.
    pub segmented_by: Vec<Segment>,
    /// `None` when the provider does not report one. Never guessed.
    pub sample_size: Option<i64>,
    /// Which rank bracket these buckets actually describe, and whether that is
    /// the one that was asked for.
    pub bracket: ResolvedBracket,
}

#[derive(Debug, thiserror::Error)]
pub enum BenchmarkError {
    #[error("benchmark provider unavailable: {0}")]
    Unavailable(String),
    #[error("benchmark provider rate limited")]
    RateLimited,
    #[error("unexpected benchmark response: {0}")]
    InvalidResponse(String),
    #[error("no benchmark data for this context")]
    NotFound,
}

#[async_trait]
pub trait BenchmarkProvider: Send + Sync {
    async fn get_distribution(
        &self,
        context: &BenchmarkContext,
    ) -> Result<Distribution, BenchmarkError>;
}

/// The player's own figures for one hero, as computed by the metrics engine.
pub struct PlayerValues {
    pub values: HashMap<BenchmarkMetric, f32>,
    /// Matches behind those figures.
    pub sample: i64,
}

/// Compare a player against a distribution.
///
/// Pure: everything network-shaped already happened. The rules it enforces:
///
///   - No percentile below the sample floor. A figure from three games is
///     reported with its sample size and an explanation, never a rank.
///   - Direction is honoured, so 90 always means "better than 90% of players".
///   - A metric the provider does not cover is omitted, not defaulted.
pub fn compare(values: &PlayerValues, distribution: &Distribution) -> Vec<BenchmarkResult> {
    let confidence = percentile::confidence_for(values.sample);

    BenchmarkMetric::ALL
        .into_iter()
        .filter_map(|metric| {
            let player_value = *values.values.get(&metric)?;
            let buckets = distribution.buckets.get(&metric);

            let (peer_median, top_20_value) = match buckets {
                Some(b) => (percentile::value_at(0.5, b), percentile::value_at(0.8, b)),
                None => (None, None),
            };

            // Two independent reasons to withhold a percentile: too few of the
            // player's matches, or no distribution to place them in.
            let (percentile_value, note) = match (confidence, buckets) {
                (Confidence::Insufficient, _) => (
                    None,
                    Some(format!(
                        "Not enough matches yet — {} needed for a percentile, you have {}.",
                        percentile::MIN_SAMPLE,
                        values.sample
                    )),
                ),
                (_, None) => (
                    None,
                    Some("The provider has no distribution for this metric.".to_string()),
                ),
                (_, Some(b)) => (
                    percentile::percentile_of(player_value, b, metric.higher_is_better()),
                    None,
                ),
            };

            let gap = top_20_value
                .map(|top| percentile::gap_to_top(player_value, top, metric.higher_is_better()));

            Some(BenchmarkResult {
                metric,
                label: metric.label(),
                higher_is_better: metric.higher_is_better(),
                player_value,
                player_sample: values.sample,
                peer_median,
                top_20_value,
                percentile: percentile_value,
                gap_to_top_20: gap,
                peer_sample_size: distribution.sample_size,
                confidence,
                segmented_by: distribution.segmented_by.clone(),
                note,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buckets() -> Vec<Bucket> {
        [(0.1, 100.0), (0.5, 500.0), (0.8, 800.0), (0.9, 900.0)]
            .into_iter()
            .map(|(percentile, value)| Bucket { percentile, value })
            .collect()
    }

    fn distribution() -> Distribution {
        Distribution {
            buckets: HashMap::from([(BenchmarkMetric::GoldPerMin, buckets())]),
            segmented_by: vec![Segment::Hero],
            sample_size: None,
            bracket: ResolvedBracket::all_ranks(),
        }
    }

    fn values(gpm: f32, sample: i64) -> PlayerValues {
        PlayerValues {
            values: HashMap::from([(BenchmarkMetric::GoldPerMin, gpm)]),
            sample,
        }
    }

    #[test]
    fn a_solid_sample_gets_a_percentile_and_a_gap() {
        let results = compare(&values(500.0, 30), &distribution());
        let gpm = &results[0];

        assert_eq!(gpm.metric, BenchmarkMetric::GoldPerMin);
        assert_eq!(gpm.peer_median, Some(500.0));
        assert_eq!(gpm.top_20_value, Some(800.0));
        assert!((gpm.percentile.unwrap() - 50.0).abs() < 0.5);
        assert_eq!(gpm.gap_to_top_20, Some(300.0));
        assert_eq!(gpm.confidence, Confidence::Adequate);
        assert!(gpm.note.is_none());
    }

    #[test]
    fn a_thin_sample_gets_no_percentile_and_an_explanation() {
        let results = compare(&values(500.0, 2), &distribution());
        let gpm = &results[0];

        assert_eq!(gpm.confidence, Confidence::Insufficient);
        assert_eq!(gpm.percentile, None, "must not rank a two-game average");
        assert!(gpm.note.as_deref().unwrap().contains("Not enough matches"));
        // The player's own value is still theirs to see.
        assert_eq!(gpm.player_value, 500.0);
        // And the reference distribution is not a secret either.
        assert_eq!(gpm.peer_median, Some(500.0));
    }

    #[test]
    fn a_borderline_sample_is_reported_but_flagged() {
        let results = compare(&values(500.0, percentile::MIN_SAMPLE), &distribution());
        assert_eq!(results[0].confidence, Confidence::Low);
        assert!(results[0].percentile.is_some());
    }

    #[test]
    fn a_metric_the_provider_does_not_cover_is_omitted_not_defaulted() {
        let results = compare(&values(500.0, 30), &distribution());
        // Only gold-per-minute was supplied on either side.
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn a_metric_with_no_distribution_keeps_the_value_and_explains_the_absence() {
        let mut empty = distribution();
        empty.buckets.clear();

        let results = compare(&values(500.0, 30), &empty);
        let gpm = &results[0];

        assert_eq!(gpm.player_value, 500.0);
        assert_eq!(gpm.percentile, None);
        assert_eq!(gpm.peer_median, None);
        assert!(gpm.note.as_deref().unwrap().contains("no distribution"));
    }

    #[test]
    fn results_report_what_was_actually_segmented() {
        let results = compare(&values(500.0, 30), &distribution());
        // Hero only: this provider cannot segment by rank, and must not imply it.
        assert_eq!(results[0].segmented_by, vec![Segment::Hero]);
        assert_eq!(results[0].peer_sample_size, None);
    }
}
