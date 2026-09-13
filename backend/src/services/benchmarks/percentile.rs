//! Where a value falls in a distribution.
//!
//! Pure arithmetic over provider buckets. No network, no database — so the one
//! part of benchmarking that is easy to get subtly wrong is also the part that
//! is exhaustively testable.

use crate::domain::benchmark::{Bucket, Confidence};

/// Below this many of the player's own matches, no percentile is claimed.
///
/// The spec is explicit that a percentile must not be asserted on an
/// insufficient sample, and this is the player's side of that: an average over
/// three games says more about variance than about skill.
pub const MIN_SAMPLE: i64 = 5;
/// Below this, the figure is shown but flagged.
pub const LOW_SAMPLE: i64 = 15;

pub fn confidence_for(player_sample: i64) -> Confidence {
    if player_sample < MIN_SAMPLE {
        Confidence::Insufficient
    } else if player_sample < LOW_SAMPLE {
        Confidence::Low
    } else {
        Confidence::Adequate
    }
}

/// Linearly interpolate a value's percentile within a bucket list.
///
/// Buckets are sorted ascending by percentile before use, so a provider that
/// changes its ordering cannot silently produce nonsense.
///
/// Returns 0-100, and always in "higher is better" terms: for a metric where
/// less is more (deaths), the raw position is inverted, so 90 always means
/// "better than 90% of players" whichever metric is being read.
pub fn percentile_of(value: f32, buckets: &[Bucket], higher_is_better: bool) -> Option<f32> {
    let sorted = sorted_buckets(buckets)?;

    let raw = raw_percentile(value, &sorted);
    Some(if higher_is_better { raw } else { 100.0 - raw })
}

/// Position within the distribution, 0-100, before direction is applied.
fn raw_percentile(value: f32, sorted: &[Bucket]) -> f32 {
    let first = sorted[0];
    let last = sorted[sorted.len() - 1];

    // Outside the reported range: clamp to the edge rather than extrapolating
    // a number the provider never measured.
    if value <= first.value {
        return first.percentile * 100.0;
    }
    if value >= last.value {
        return last.percentile * 100.0;
    }

    for pair in sorted.windows(2) {
        let (lo, hi) = (pair[0], pair[1]);
        if value >= lo.value && value <= hi.value {
            let span = hi.value - lo.value;
            // Two buckets sharing a value carry no gradient; take the lower
            // edge rather than dividing by zero.
            let t = if span.abs() < f32::EPSILON {
                0.0
            } else {
                (value - lo.value) / span
            };
            return (lo.percentile + t * (hi.percentile - lo.percentile)) * 100.0;
        }
    }

    last.percentile * 100.0
}

/// Value at a given percentile, interpolating between buckets.
pub fn value_at(percentile: f32, buckets: &[Bucket]) -> Option<f32> {
    let sorted = sorted_buckets(buckets)?;
    let target = percentile.clamp(0.0, 1.0);

    if let Some(exact) = sorted
        .iter()
        .find(|b| (b.percentile - target).abs() < f32::EPSILON)
    {
        return Some(exact.value);
    }

    let first = sorted[0];
    let last = sorted[sorted.len() - 1];
    if target <= first.percentile {
        return Some(first.value);
    }
    if target >= last.percentile {
        return Some(last.value);
    }

    for pair in sorted.windows(2) {
        let (lo, hi) = (pair[0], pair[1]);
        if target >= lo.percentile && target <= hi.percentile {
            let span = hi.percentile - lo.percentile;
            let t = if span.abs() < f32::EPSILON {
                0.0
            } else {
                (target - lo.percentile) / span
            };
            return Some(lo.value + t * (hi.value - lo.value));
        }
    }

    Some(last.value)
}

/// Distance from the player to the top-20% line, signed so a positive number
/// always means "work still to do".
pub fn gap_to_top(player: f32, top: f32, higher_is_better: bool) -> f32 {
    if higher_is_better {
        top - player
    } else {
        player - top
    }
}

fn sorted_buckets(buckets: &[Bucket]) -> Option<Vec<Bucket>> {
    if buckets.is_empty() {
        return None;
    }

    let mut sorted = buckets.to_vec();
    sorted.sort_by(|a, b| a.percentile.total_cmp(&b.percentile));
    Some(sorted)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape OpenDota actually returns for a hero.
    fn gpm() -> Vec<Bucket> {
        [
            (0.1, 361.0),
            (0.2, 424.0),
            (0.3, 472.0),
            (0.4, 516.0),
            (0.5, 556.0),
            (0.6, 596.0),
            (0.7, 638.0),
            (0.8, 684.0),
            (0.9, 740.0),
            (0.95, 790.0),
            (0.99, 886.0),
        ]
        .into_iter()
        .map(|(percentile, value)| Bucket { percentile, value })
        .collect()
    }

    #[test]
    fn a_value_on_a_bucket_returns_that_percentile() {
        let p = percentile_of(684.0, &gpm(), true).unwrap();
        assert!((p - 80.0).abs() < 0.01, "got {p}");
    }

    #[test]
    fn a_value_between_buckets_interpolates() {
        // Halfway between p0.5 (556) and p0.6 (596) is 576 -> 55th.
        let p = percentile_of(576.0, &gpm(), true).unwrap();
        assert!((p - 55.0).abs() < 0.5, "got {p}");
    }

    #[test]
    fn values_outside_the_reported_range_clamp_to_its_edges() {
        // The provider never measured below p0.1 or above p0.99, so neither
        // does this.
        assert_eq!(percentile_of(10.0, &gpm(), true).unwrap(), 10.0);
        assert_eq!(percentile_of(99_999.0, &gpm(), true).unwrap(), 99.0);
    }

    #[test]
    fn direction_is_applied_so_ninety_always_means_good() {
        // A high death rate sits high in the raw distribution, which is a bad
        // result: the reported percentile must invert.
        let deaths = vec![
            Bucket {
                percentile: 0.1,
                value: 0.1,
            },
            Bucket {
                percentile: 0.5,
                value: 0.2,
            },
            Bucket {
                percentile: 0.9,
                value: 0.4,
            },
        ];

        let many_deaths = percentile_of(0.4, &deaths, false).unwrap();
        let few_deaths = percentile_of(0.1, &deaths, false).unwrap();

        assert!((many_deaths - 10.0).abs() < 0.01, "got {many_deaths}");
        assert!((few_deaths - 90.0).abs() < 0.01, "got {few_deaths}");
        assert!(few_deaths > many_deaths, "fewer deaths must rank better");
    }

    #[test]
    fn unsorted_buckets_are_ordered_before_use() {
        let mut shuffled = gpm();
        shuffled.reverse();

        assert_eq!(
            percentile_of(684.0, &shuffled, true),
            percentile_of(684.0, &gpm(), true),
        );
    }

    #[test]
    fn an_empty_distribution_yields_nothing_rather_than_a_default() {
        assert_eq!(percentile_of(500.0, &[], true), None);
        assert_eq!(value_at(0.8, &[]), None);
    }

    #[test]
    fn duplicate_bucket_values_do_not_divide_by_zero() {
        let flat = vec![
            Bucket {
                percentile: 0.4,
                value: 100.0,
            },
            Bucket {
                percentile: 0.5,
                value: 100.0,
            },
            Bucket {
                percentile: 0.6,
                value: 200.0,
            },
        ];

        let p = percentile_of(100.0, &flat, true).unwrap();
        assert!(p.is_finite(), "got {p}");
    }

    #[test]
    fn value_at_reads_the_distribution_back() {
        assert_eq!(value_at(0.8, &gpm()), Some(684.0));
        assert_eq!(value_at(0.5, &gpm()), Some(556.0));
    }

    #[test]
    fn value_at_interpolates_between_reported_percentiles() {
        // p0.85 sits midway between p0.8 (684) and p0.9 (740).
        let v = value_at(0.85, &gpm()).unwrap();
        assert!((v - 712.0).abs() < 0.5, "got {v}");
    }

    #[test]
    fn the_gap_is_positive_whenever_there_is_work_to_do() {
        // Below the line on a more-is-better metric.
        assert_eq!(gap_to_top(600.0, 684.0, true), 84.0);
        // Already past it: negative, i.e. ahead.
        assert_eq!(gap_to_top(700.0, 684.0, true), -16.0);
        // Above the line on a less-is-better metric is also work to do.
        assert!((gap_to_top(0.4, 0.25, false) - 0.15).abs() < 1e-6);
    }

    #[test]
    fn confidence_follows_the_players_own_sample() {
        assert_eq!(confidence_for(0), Confidence::Insufficient);
        assert_eq!(confidence_for(MIN_SAMPLE - 1), Confidence::Insufficient);
        assert_eq!(confidence_for(MIN_SAMPLE), Confidence::Low);
        assert_eq!(confidence_for(LOW_SAMPLE - 1), Confidence::Low);
        assert_eq!(confidence_for(LOW_SAMPLE), Confidence::Adequate);
        assert_eq!(confidence_for(200), Confidence::Adequate);
    }
}
