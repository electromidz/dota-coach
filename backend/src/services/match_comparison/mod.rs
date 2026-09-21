//! The arithmetic behind the match-detail comparison.
//!
//! Everything here is pure: the distribution has already been fetched and the
//! matches already read. That is what makes it testable, and what keeps a
//! provider outage a handler concern rather than a maths concern.
//!
//! Two rules this module exists to enforce:
//!
//!   - **A single match is not a small sample.** The benchmark engine's
//!     [`compare`](crate::services::benchmarks::compare) withholds a
//!     percentile below `MIN_SAMPLE`, which is correct for a career average
//!     and wrong here: "this game's gold per minute beat 78% of peer games" is
//!     a fact about a game that was played, not an estimate of the player. So
//!     this module reaches for [`percentile::percentile_of`] directly, and the
//!     sample floor still applies — unchanged — to the average beside it.
//!   - **The summary is a median, and says so.** Not a weighted composite:
//!     the weights that decide what matters about a player already live in
//!     [`FitWeights`](crate::domain::hero::FitWeights), and a second scoring
//!     system invented on this page would be a second answer to the same
//!     question.

use std::collections::HashMap;

use crate::domain::benchmark::BenchmarkMetric;
use crate::domain::match_comparison::{
    AverageReading, Highlight, MetricComparison, Reading, Suggestion, TrendPoint,
};
use crate::repositories::metrics::HeroMatchValues;
use crate::services::benchmarks::{percentile, Distribution};

/// At or above this percentile, a metric is worth calling a strength.
pub const PRO_PERCENTILE: f32 = 70.0;
/// At or below this percentile, a metric is worth calling a weakness.
pub const CON_PERCENTILE: f32 = 30.0;
/// One page cannot act on eight findings. Three of each is a read, not a dump.
const MAX_HIGHLIGHTS: usize = 3;

/// Place a set of figures in the distribution, one percentile per metric.
///
/// Direction is handled inside [`percentile::percentile_of`], so a result of
/// 90 means "better than 90% of peers" for deaths exactly as it does for gold.
/// Metrics the figures lack, or the provider does not cover, are absent rather
/// than zero.
pub fn percentiles_for(
    values: &HashMap<BenchmarkMetric, f32>,
    distribution: &Distribution,
) -> HashMap<BenchmarkMetric, f32> {
    values
        .iter()
        .filter_map(|(metric, value)| {
            let buckets = distribution.buckets.get(metric)?;
            let p = percentile::percentile_of(*value, buckets, metric.higher_is_better())?;
            Some((*metric, p))
        })
        .collect()
}

/// The median of the per-metric percentiles.
///
/// A median rather than a mean because the metrics are not independent and one
/// extreme — a 40-minute game with two tower hits — should move the summary by
/// one rank, not drag it. `None` when nothing could be placed.
pub fn standing(percentiles: &HashMap<BenchmarkMetric, f32>) -> Option<f32> {
    if percentiles.is_empty() {
        return None;
    }

    let mut values: Vec<f32> = percentiles.values().copied().collect();
    values.sort_by(|a, b| a.partial_cmp(b).expect("percentiles are never NaN"));

    let mid = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    })
}

/// Build the per-metric rows, in the dashboard order the enum already defines.
///
/// A metric neither side has a figure for is dropped entirely: a row reading
/// "— vs —" is noise, and an empty row is not the same as a bad result.
pub fn metric_comparisons(
    match_values: &HashMap<BenchmarkMetric, f32>,
    match_percentiles: &HashMap<BenchmarkMetric, f32>,
    average_values: &HashMap<BenchmarkMetric, f32>,
    average_percentiles: &HashMap<BenchmarkMetric, f32>,
    average_sample: i64,
    distribution: Option<&Distribution>,
) -> Vec<MetricComparison> {
    let confidence = percentile::confidence_for(average_sample);

    BenchmarkMetric::ALL
        .into_iter()
        .filter_map(|metric| {
            let this_match = match_values.get(&metric).map(|value| Reading {
                value: *value,
                percentile: match_percentiles.get(&metric).copied(),
            });
            let hero_average = average_values.get(&metric).map(|value| AverageReading {
                value: *value,
                percentile: average_percentiles.get(&metric).copied(),
                sample: average_sample,
                confidence,
            });

            if this_match.is_none() && hero_average.is_none() {
                return None;
            }

            let buckets = distribution.and_then(|d| d.buckets.get(&metric));

            Some(MetricComparison {
                metric,
                label: metric.label(),
                higher_is_better: metric.higher_is_better(),
                this_match,
                hero_average,
                peer_median: buckets.and_then(|b| percentile::value_at(0.5, b)),
                top_20_value: buckets.and_then(|b| percentile::value_at(0.8, b)),
            })
        })
        .collect()
}

/// The strengths and weaknesses of one set of percentiles.
///
/// Returned as `(pros, cons)`, each sorted by how far it sits from the middle,
/// so the most emphatic finding leads. Because percentiles arrive already
/// direction-corrected, a high figure is good for every metric and deaths need
/// no special case here — the correction happened once, upstream.
pub fn pros_and_cons(
    percentiles: &HashMap<BenchmarkMetric, f32>,
    values: &HashMap<BenchmarkMetric, f32>,
    distribution: &Distribution,
    bracket_label: &str,
) -> (Vec<Highlight>, Vec<Highlight>) {
    let mut pros: Vec<Highlight> = Vec::new();
    let mut cons: Vec<Highlight> = Vec::new();

    for metric in BenchmarkMetric::ALL {
        let (Some(&p), Some(&value)) = (percentiles.get(&metric), values.get(&metric)) else {
            continue;
        };

        let median = distribution
            .buckets
            .get(&metric)
            .and_then(|b| percentile::value_at(0.5, b));

        if p >= PRO_PERCENTILE {
            pros.push(highlight(metric, value, p, median, bracket_label));
        } else if p <= CON_PERCENTILE {
            cons.push(highlight(metric, value, p, median, bracket_label));
        }
    }

    // Furthest from the middle first, in both directions.
    pros.sort_by(|a, b| b.percentile.total_cmp(&a.percentile));
    cons.sort_by(|a, b| a.percentile.total_cmp(&b.percentile));
    pros.truncate(MAX_HIGHLIGHTS);
    cons.truncate(MAX_HIGHLIGHTS);

    (pros, cons)
}

fn highlight(
    metric: BenchmarkMetric,
    value: f32,
    percentile: f32,
    median: Option<f32>,
    bracket_label: &str,
) -> Highlight {
    let detail = match median {
        Some(median) => format!(
            "{} against a {bracket_label} median of {}.",
            format_value(value),
            format_value(median),
        ),
        None => format!("{} for this game.", format_value(value)),
    };

    Highlight {
        metric,
        label: metric.label(),
        value,
        percentile,
        detail,
    }
}

/// What to work on, from the weakest metric that has a peer median to aim at.
///
/// One suggestion, not a list: the product selects a single training focus
/// elsewhere for the same reason, and a page that names five things to fix
/// names none.
pub fn suggestion(
    percentiles: &HashMap<BenchmarkMetric, f32>,
    values: &HashMap<BenchmarkMetric, f32>,
    distribution: &Distribution,
    duration_seconds: i32,
    hero_name: &str,
    bracket_label: &str,
) -> Option<Suggestion> {
    let (metric, percentile_value) = percentiles
        .iter()
        .filter(|(_, p)| **p <= CON_PERCENTILE)
        .min_by(|a, b| a.1.total_cmp(b.1))
        .map(|(m, p)| (*m, *p))?;

    let player_value = *values.get(&metric)?;
    let peer_median = distribution
        .buckets
        .get(&metric)
        .and_then(|b| percentile::value_at(0.5, b))?;

    // Always "how far to move", never a signed direction the reader has to
    // interpret: `gap_to_top` is the same convention, applied to the median.
    let gap = percentile::gap_to_top(player_value, peer_median, metric.higher_is_better());

    let minutes = (duration_seconds as f32 / 60.0).max(1.0);
    let whole_game_unit = metric.countable_unit();
    let whole_game_delta = whole_game_unit.map(|_| gap * minutes);

    let text = match (whole_game_unit, whole_game_delta) {
        (Some(unit), Some(delta)) => format!(
            "{} is where this game sat lowest against {bracket_label} players on {hero_name} \
             (p{percentile}). Matching their median over {duration} minutes is about {delta} \
             {direction} {unit}.",
            metric.label(),
            percentile = percentile_value.round() as i32,
            duration = minutes.round() as i32,
            delta = delta.abs().round().max(1.0) as i32,
            direction = if metric.higher_is_better() {
                "more"
            } else {
                "fewer"
            },
        ),
        _ => format!(
            "{} is where this game sat lowest against {bracket_label} players on {hero_name} \
             (p{percentile}): {player} against their median of {median}.",
            metric.label(),
            percentile = percentile_value.round() as i32,
            player = format_value(player_value),
            median = format_value(peer_median),
        ),
    };

    Some(Suggestion {
        metric,
        label: metric.label(),
        percentile: percentile_value,
        player_value,
        peer_median,
        whole_game_delta,
        whole_game_unit,
        text,
    })
}

/// How many games the trend plots.
pub const TREND_WINDOW: usize = 10;

/// The slice of history the trend should cover: the match being viewed and the
/// games before it.
///
/// Not simply "the newest ten". Opening a game from last month should show how
/// it compared with the ones leading up to *it* — a chart that instead plots
/// this week's form beside a month-old match invites the reader to compare two
/// things that were never adjacent.
///
/// `history` is newest first, so the games before the current one are the ones
/// after it in the slice. A match that is not in the history at all — a Turbo
/// game, or one older than the analysis window — falls back to the most recent
/// window, which the caller marks as containing no current match.
pub fn trend_window(
    history: &[HeroMatchValues],
    current_match_id: uuid::Uuid,
) -> &[HeroMatchValues] {
    let start = history
        .iter()
        .position(|m| m.match_id == current_match_id)
        .unwrap_or(0);
    let end = (start + TREND_WINDOW).min(history.len());
    &history[start..end]
}

/// Reduce each past game on this hero to a standing.
///
/// `history` arrives newest first and stays that way. A game whose figures
/// place nowhere in the distribution is dropped rather than plotted at zero —
/// a missing point is honest, a zero is a lie shaped like data.
pub fn trend(
    history: &[HeroMatchValues],
    distribution: &Distribution,
    current_match_id: uuid::Uuid,
) -> Vec<TrendPoint> {
    history
        .iter()
        .filter_map(|m| {
            let standing = standing(&percentiles_for(&m.figures.values(), distribution))?;
            Some(TrendPoint {
                match_id: m.match_id,
                dota_match_id: m.dota_match_id,
                started_at: m.started_at,
                won: m.won,
                standing,
                is_current: m.match_id == current_match_id,
            })
        })
        .collect()
}

/// This match's standing minus the previous game's on the same hero.
///
/// "Previous" means the next game backwards in time, not the newest in the
/// list: a player opening a match from last week should see how it compared
/// with the one before *it*, not with today's.
pub fn delta_vs_previous(trend: &[TrendPoint], current_match_id: uuid::Uuid) -> Option<f32> {
    let index = trend.iter().position(|p| p.match_id == current_match_id)?;
    let current = trend.get(index)?;
    // Newest first, so the older game is the next index along.
    let previous = trend.get(index + 1)?;
    Some(current.standing - previous.standing)
}

/// Large figures read better whole; small rates need their decimals. The same
/// rule the client's chart labels use, applied server-side so prose and chart
/// never disagree about what a number is.
fn format_value(value: f32) -> String {
    if value.abs() >= 100.0 {
        format!("{}", value.round() as i64)
    } else if value.abs() >= 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::benchmark::{Bucket, ResolvedBracket};
    use crate::domain::hero::RankBracket;
    use crate::repositories::metrics::BenchmarkFigures;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn test_buckets(pairs: &[(f32, f32)]) -> Vec<Bucket> {
        pairs
            .iter()
            .map(|(percentile, value)| Bucket {
                percentile: *percentile,
                value: *value,
            })
            .collect()
    }

    fn distribution() -> Distribution {
        Distribution {
            buckets: HashMap::from([
                (
                    BenchmarkMetric::GoldPerMin,
                    test_buckets(&[(0.1, 300.0), (0.5, 500.0), (0.8, 700.0), (0.9, 800.0)]),
                ),
                (
                    BenchmarkMetric::DeathsPerMin,
                    test_buckets(&[(0.1, 0.05), (0.5, 0.20), (0.8, 0.35), (0.9, 0.45)]),
                ),
            ]),
            segmented_by: Vec::new(),
            sample_size: None,
            bracket: ResolvedBracket::exact(RankBracket::Legend),
        }
    }

    fn values(pairs: &[(BenchmarkMetric, f32)]) -> HashMap<BenchmarkMetric, f32> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn a_single_match_gets_a_percentile_with_no_sample_floor() {
        // The whole point of this module: one game, one percentile, no
        // "not enough matches" note. The benchmark engine would withhold this.
        let p = percentiles_for(
            &values(&[(BenchmarkMetric::GoldPerMin, 700.0)]),
            &distribution(),
        );
        assert!((p[&BenchmarkMetric::GoldPerMin] - 80.0).abs() < 0.5);
    }

    #[test]
    fn deaths_are_direction_corrected_so_high_always_means_good() {
        let p = percentiles_for(
            &values(&[(BenchmarkMetric::DeathsPerMin, 0.05)]),
            &distribution(),
        );
        // Dying least of anyone is a strength, so it must not read as p10.
        assert!(
            p[&BenchmarkMetric::DeathsPerMin] > 85.0,
            "{:?}",
            p[&BenchmarkMetric::DeathsPerMin]
        );

        let p = percentiles_for(
            &values(&[(BenchmarkMetric::DeathsPerMin, 0.45)]),
            &distribution(),
        );
        assert!(p[&BenchmarkMetric::DeathsPerMin] < 15.0);
    }

    #[test]
    fn a_metric_the_provider_does_not_cover_is_absent_not_zero() {
        let p = percentiles_for(
            &values(&[(BenchmarkMetric::XpPerMin, 600.0)]),
            &distribution(),
        );
        assert!(p.is_empty());
    }

    #[test]
    fn the_standing_is_the_median_of_the_metric_percentiles() {
        let p = HashMap::from([
            (BenchmarkMetric::GoldPerMin, 10.0),
            (BenchmarkMetric::XpPerMin, 50.0),
            (BenchmarkMetric::DeathsPerMin, 90.0),
        ]);
        assert_eq!(standing(&p), Some(50.0));

        // Even count averages the middle pair rather than picking a side.
        let p = HashMap::from([
            (BenchmarkMetric::GoldPerMin, 40.0),
            (BenchmarkMetric::XpPerMin, 60.0),
        ]);
        assert_eq!(standing(&p), Some(50.0));

        assert_eq!(standing(&HashMap::new()), None);
    }

    #[test]
    fn one_extreme_metric_does_not_drag_the_standing() {
        // The reason it is a median. Four solid metrics and one disaster
        // should read as "a good game with a problem", not a bad game.
        let p = HashMap::from([
            (BenchmarkMetric::GoldPerMin, 70.0),
            (BenchmarkMetric::XpPerMin, 72.0),
            (BenchmarkMetric::LastHitsPerMin, 68.0),
            (BenchmarkMetric::KillsPerMin, 75.0),
            (BenchmarkMetric::TowerDamage, 1.0),
        ]);
        assert_eq!(standing(&p), Some(70.0));
    }

    #[test]
    fn pros_and_cons_split_on_the_percentile_thresholds() {
        let percentiles = HashMap::from([
            (BenchmarkMetric::GoldPerMin, 82.0),
            (BenchmarkMetric::DeathsPerMin, 18.0),
        ]);
        let vals = values(&[
            (BenchmarkMetric::GoldPerMin, 720.0),
            (BenchmarkMetric::DeathsPerMin, 0.40),
        ]);

        let (pros, cons) = pros_and_cons(&percentiles, &vals, &distribution(), "Legend");

        assert_eq!(pros.len(), 1);
        assert_eq!(pros[0].metric, BenchmarkMetric::GoldPerMin);
        // The median it was measured against travels with it.
        assert!(pros[0].detail.contains("Legend"), "{}", pros[0].detail);

        assert_eq!(cons.len(), 1);
        assert_eq!(cons[0].metric, BenchmarkMetric::DeathsPerMin);
    }

    #[test]
    fn a_middling_game_produces_no_highlights_rather_than_manufactured_ones() {
        let percentiles = HashMap::from([
            (BenchmarkMetric::GoldPerMin, 55.0),
            (BenchmarkMetric::DeathsPerMin, 48.0),
        ]);
        let vals = values(&[
            (BenchmarkMetric::GoldPerMin, 550.0),
            (BenchmarkMetric::DeathsPerMin, 0.21),
        ]);

        let (pros, cons) = pros_and_cons(&percentiles, &vals, &distribution(), "Legend");
        assert!(pros.is_empty());
        assert!(cons.is_empty());
    }

    #[test]
    fn highlights_are_capped_and_lead_with_the_most_emphatic() {
        let percentiles = HashMap::from([
            (BenchmarkMetric::GoldPerMin, 71.0),
            (BenchmarkMetric::XpPerMin, 95.0),
            (BenchmarkMetric::LastHitsPerMin, 88.0),
            (BenchmarkMetric::KillsPerMin, 74.0),
        ]);
        let vals = values(&[
            (BenchmarkMetric::GoldPerMin, 700.0),
            (BenchmarkMetric::XpPerMin, 900.0),
            (BenchmarkMetric::LastHitsPerMin, 11.0),
            (BenchmarkMetric::KillsPerMin, 0.5),
        ]);

        let (pros, _) = pros_and_cons(&percentiles, &vals, &distribution(), "Legend");
        assert_eq!(pros.len(), MAX_HIGHLIGHTS);
        assert_eq!(pros[0].metric, BenchmarkMetric::XpPerMin);
    }

    #[test]
    fn the_suggestion_converts_a_rate_gap_into_a_whole_game_count() {
        let percentiles = HashMap::from([(BenchmarkMetric::DeathsPerMin, 12.0)]);
        // 0.40 deaths/min against a median of 0.20, over a 40-minute game:
        // 0.20 * 40 = 8 fewer deaths.
        let vals = values(&[(BenchmarkMetric::DeathsPerMin, 0.40)]);

        let s = suggestion(
            &percentiles,
            &vals,
            &distribution(),
            40 * 60,
            "Anti-Mage",
            "Legend",
        )
        .expect("a weak metric with a median produces a suggestion");

        assert_eq!(s.metric, BenchmarkMetric::DeathsPerMin);
        assert_eq!(s.whole_game_unit, Some("deaths"));
        assert!((s.whole_game_delta.unwrap() - 8.0).abs() < 0.1);
        assert!(s.text.contains("fewer"), "{}", s.text);
        assert!(s.text.contains("Anti-Mage"), "{}", s.text);
    }

    #[test]
    fn a_rate_metric_keeps_its_rate_rather_than_a_fabricated_total() {
        let percentiles = HashMap::from([(BenchmarkMetric::GoldPerMin, 15.0)]);
        let vals = values(&[(BenchmarkMetric::GoldPerMin, 320.0)]);

        let s = suggestion(
            &percentiles,
            &vals,
            &distribution(),
            30 * 60,
            "Anti-Mage",
            "Legend",
        )
        .unwrap();

        assert_eq!(s.whole_game_unit, None);
        assert_eq!(s.whole_game_delta, None);
        assert_eq!(s.peer_median, 500.0);
    }

    #[test]
    fn a_good_game_gets_no_suggestion() {
        let percentiles = HashMap::from([(BenchmarkMetric::GoldPerMin, 88.0)]);
        let vals = values(&[(BenchmarkMetric::GoldPerMin, 780.0)]);

        assert!(suggestion(
            &percentiles,
            &vals,
            &distribution(),
            30 * 60,
            "Anti-Mage",
            "Legend",
        )
        .is_none());
    }

    fn match_row(id: Uuid, gpm: f64, day: u32) -> HeroMatchValues {
        HeroMatchValues {
            match_id: id,
            dota_match_id: 1_000 + day as i64,
            started_at: Utc.with_ymd_and_hms(2026, 1, day, 12, 0, 0).unwrap(),
            won: true,
            duration_seconds: 2_100,
            figures: BenchmarkFigures {
                gold_per_min: Some(gpm),
                ..Default::default()
            },
        }
    }

    #[test]
    fn the_trend_marks_the_match_being_viewed() {
        let current = Uuid::new_v4();
        let history = vec![
            match_row(current, 700.0, 3),
            match_row(Uuid::new_v4(), 500.0, 2),
        ];

        let points = trend(&history, &distribution(), current);
        assert_eq!(points.len(), 2);
        assert!(points[0].is_current);
        assert!(!points[1].is_current);
    }

    #[test]
    fn the_delta_compares_against_the_game_before_this_one() {
        let current = Uuid::new_v4();
        let history = vec![
            // Newest is *not* the match being viewed, which is the case that
            // catches a delta wired to the head of the list.
            match_row(Uuid::new_v4(), 800.0, 4),
            match_row(current, 700.0, 3),
            match_row(Uuid::new_v4(), 500.0, 2),
        ];

        let points = trend(&history, &distribution(), current);
        let delta = delta_vs_previous(&points, current).unwrap();

        // p80 for 700 against p50 for 500.
        assert!((delta - 30.0).abs() < 1.0, "{delta}");
    }

    #[test]
    fn a_first_game_on_a_hero_has_no_delta() {
        let current = Uuid::new_v4();
        let points = trend(&[match_row(current, 700.0, 3)], &distribution(), current);
        assert_eq!(delta_vs_previous(&points, current), None);
    }

    #[test]
    fn a_match_outside_the_trend_has_no_delta() {
        // A Turbo game is not in the competitive history it is plotted beside,
        // so there is nothing to subtract from.
        let points = trend(
            &[match_row(Uuid::new_v4(), 700.0, 3)],
            &distribution(),
            Uuid::new_v4(),
        );
        assert_eq!(delta_vs_previous(&points, Uuid::new_v4()), None);
    }

    #[test]
    fn a_game_that_places_nowhere_is_dropped_from_the_trend_rather_than_plotted_at_zero() {
        let mut row = match_row(Uuid::new_v4(), 700.0, 3);
        row.figures.gold_per_min = None;

        assert!(trend(&[row], &distribution(), Uuid::new_v4()).is_empty());
    }

    #[test]
    fn the_trend_window_ends_at_the_match_being_viewed() {
        let current = Uuid::new_v4();
        // Newest first: two games after the one being viewed, one before it.
        let history = vec![
            match_row(Uuid::new_v4(), 800.0, 5),
            match_row(Uuid::new_v4(), 790.0, 4),
            match_row(current, 700.0, 3),
            match_row(Uuid::new_v4(), 500.0, 2),
        ];

        let window = trend_window(&history, current);
        assert_eq!(window.len(), 2, "the current game and the one before it");
        assert_eq!(window[0].match_id, current);
        assert_eq!(window[1].dota_match_id, 1_002);
    }

    #[test]
    fn the_trend_window_is_capped() {
        let current = Uuid::new_v4();
        let mut history = vec![match_row(current, 700.0, 28)];
        for day in 1..=20 {
            history.push(match_row(Uuid::new_v4(), 600.0, day));
        }

        assert_eq!(trend_window(&history, current).len(), TREND_WINDOW);
    }

    #[test]
    fn a_match_outside_the_history_still_gets_a_window_to_plot() {
        // A Turbo game is not in the competitive history, but the player's
        // recent form on the hero is still worth showing beside it.
        let history = vec![
            match_row(Uuid::new_v4(), 800.0, 5),
            match_row(Uuid::new_v4(), 700.0, 4),
        ];

        let window = trend_window(&history, Uuid::new_v4());
        assert_eq!(window.len(), 2);
        // And nothing in it claims to be the match being viewed.
        let points = trend(window, &distribution(), Uuid::new_v4());
        assert!(points.iter().all(|p| !p.is_current));
    }

    #[test]
    fn a_metric_neither_side_has_is_left_out_of_the_rows() {
        let rows = metric_comparisons(
            &values(&[(BenchmarkMetric::GoldPerMin, 700.0)]),
            &HashMap::new(),
            &values(&[(BenchmarkMetric::GoldPerMin, 550.0)]),
            &HashMap::new(),
            12,
            Some(&distribution()),
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].metric, BenchmarkMetric::GoldPerMin);
        assert_eq!(rows[0].peer_median, Some(500.0));
        assert_eq!(rows[0].top_20_value, Some(700.0));
    }

    #[test]
    fn the_average_carries_its_confidence_and_the_single_match_does_not_need_one() {
        let rows = metric_comparisons(
            &values(&[(BenchmarkMetric::GoldPerMin, 700.0)]),
            &values(&[(BenchmarkMetric::GoldPerMin, 80.0)]),
            &values(&[(BenchmarkMetric::GoldPerMin, 550.0)]),
            &values(&[(BenchmarkMetric::GoldPerMin, 60.0)]),
            2,
            Some(&distribution()),
        );

        let row = &rows[0];
        // Two matches is below the floor, and the average says so.
        assert_eq!(
            row.hero_average.unwrap().confidence,
            crate::domain::benchmark::Confidence::Insufficient
        );
        // The single match still has its percentile: it is a fact, not an
        // estimate, and no floor applies to it.
        assert_eq!(row.this_match.unwrap().percentile, Some(80.0));
    }
}
