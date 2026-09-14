//! Meta strength: how strong a hero currently is, on a 0-100 scale.
//!
//! Deliberately **not** the win rate. Dota win rates live in a narrow band —
//! most heroes sit between 45% and 55% — so a raw win rate is both unreadable
//! and a poor ordering: a 52% hero nobody picks and a 52% hero a quarter of the
//! ladder picks are not equally "strong".
//!
//! The score combines three signals the provider can actually supply:
//!
//!   - **Win rate**, relative to the spread of the cohort rather than to 50%.
//!   - **Pick rate**, by rank among the cohort. Rank rather than the raw share,
//!     because pick rates are heavily skewed and a handful of popular heroes
//!     would otherwise flatten everything else to zero.
//!   - **Trend**, when the provider publishes one.
//!
//! Every step is arithmetic on stored numbers: the same payload always yields
//! the same score, and each part is reported alongside the total.

use crate::domain::hero::HeroMeta;

/// Relative weights of the meta signals. Tunable for the same reason
/// [`FitWeights`](crate::domain::hero::FitWeights) is.
#[derive(Debug, Clone, Copy)]
pub struct MetaWeights {
    pub win_rate: f32,
    pub pick_rate: f32,
    pub trend: f32,
}

impl Default for MetaWeights {
    fn default() -> Self {
        Self {
            win_rate: 0.60,
            pick_rate: 0.25,
            trend: 0.15,
        }
    }
}

/// Below this many picks, a hero's figures are pulled toward neutral in
/// proportion to how short they fall. A hero with 30 games of data does not get
/// to sit at the top of the list on the strength of a 70% win rate.
pub const MIN_META_SAMPLE: i64 = 500;

/// How many points one standard deviation of win rate is worth. 20 puts a hero
/// two deviations above the cohort at 90 and keeps the middle legible.
const WIN_RATE_POINTS_PER_SD: f32 = 20.0;

/// Score every hero in the cohort, in place.
///
/// Cohort-relative by construction: `heroes` must be the full set the provider
/// returned for one context, because the win-rate spread and the pick-rate
/// ranking are both measured across it.
pub fn score_all(heroes: &mut [HeroMeta], weights: MetaWeights) {
    if heroes.is_empty() {
        return;
    }

    let total_picks: i64 = heroes.iter().map(|h| h.picks).sum();
    for hero in heroes.iter_mut() {
        hero.pick_rate = if total_picks > 0 {
            hero.picks as f32 / total_picks as f32
        } else {
            0.0
        };
    }

    let (mean_wr, sd_wr) = win_rate_spread(heroes);
    let pick_ranks = rank_percentiles(&heroes.iter().map(|h| h.pick_rate).collect::<Vec<_>>());

    for (hero, pick_rank) in heroes.iter_mut().zip(pick_ranks) {
        let win_score =
            clamp_0_100(50.0 + ((hero.win_rate - mean_wr) / sd_wr) * WIN_RATE_POINTS_PER_SD);
        let pick_score = pick_rank * 100.0;

        // A trend is a *change* in win rate: a hero up two points on its own
        // recent form has moved meaningfully, so 1pp is worth 5 points.
        let trend_score = hero
            .trend
            .map(|t| clamp_0_100(50.0 + t * 500.0))
            .unwrap_or(50.0);

        let raw = weights.win_rate * win_score
            + weights.pick_rate * pick_score
            + weights.trend * trend_score;
        let total_weight = weights.win_rate + weights.pick_rate + weights.trend;
        let weighted = if total_weight > 0.0 {
            raw / total_weight
        } else {
            50.0
        };

        hero.meta_strength = shrink_to_neutral(weighted, hero.picks);
    }
}

/// Mean and standard deviation of the cohort's win rates.
///
/// Unweighted: the question is "how much do heroes differ from each other",
/// not "what does the average game look like". A degenerate spread (every hero
/// identical, or a one-hero cohort) falls back to a floor so the division that
/// follows cannot explode.
fn win_rate_spread(heroes: &[HeroMeta]) -> (f32, f32) {
    let n = heroes.len() as f32;
    let mean = heroes.iter().map(|h| h.win_rate).sum::<f32>() / n;
    let variance = heroes
        .iter()
        .map(|h| (h.win_rate - mean).powi(2))
        .sum::<f32>()
        / n;

    (mean, variance.sqrt().max(0.001))
}

/// Position of each value within the set, 0-1.
///
/// Rank rather than value: pick rates are long-tailed, and a linear scale on
/// them would compress the whole middle of the roster into a few points.
///
/// Ties take the *midrank* — half the heroes they are level with count as
/// below them. Giving ties the lower rank instead would file three heroes tied
/// at the top of the pick list near the bottom of the scale.
fn rank_percentiles(values: &[f32]) -> Vec<f32> {
    let n = values.len();
    if n <= 1 {
        return vec![0.5; n];
    }

    values
        .iter()
        .map(|value| {
            let below = values.iter().filter(|other| *other < value).count();
            // Excludes the value itself: it is not below itself.
            let level_with = values.iter().filter(|other| *other == value).count() - 1;
            (below as f32 + level_with as f32 / 2.0) / (n - 1) as f32
        })
        .collect()
}

/// Pull a score toward 50 when the sample behind it is thin.
fn shrink_to_neutral(score: f32, picks: i64) -> f32 {
    let confidence = (picks as f32 / MIN_META_SAMPLE as f32).clamp(0.0, 1.0);
    clamp_0_100(50.0 + (score - 50.0) * confidence)
}

fn clamp_0_100(value: f32) -> f32 {
    value.clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hero(id: i32, picks: i64, win_rate: f32, trend: Option<f32>) -> HeroMeta {
        HeroMeta {
            hero_id: id,
            hero_name: format!("Hero {id}"),
            roles: Vec::new(),
            picks,
            wins: (picks as f32 * win_rate) as i64,
            win_rate,
            pick_rate: 0.0,
            trend,
            meta_strength: 0.0,
            bracket: None,
        }
    }

    /// Three heroes with an identical, healthy sample and different win rates.
    fn cohort() -> Vec<HeroMeta> {
        vec![
            hero(1, 10_000, 0.45, None),
            hero(2, 10_000, 0.50, None),
            hero(3, 10_000, 0.55, None),
        ]
    }

    #[test]
    fn a_higher_win_rate_scores_higher_within_the_same_cohort() {
        let mut heroes = cohort();
        score_all(&mut heroes, MetaWeights::default());

        assert!(heroes[2].meta_strength > heroes[1].meta_strength);
        assert!(heroes[1].meta_strength > heroes[0].meta_strength);
    }

    #[test]
    fn meta_strength_is_not_just_the_win_rate() {
        // Same win rate, wildly different popularity.
        let mut heroes = vec![
            hero(1, 100_000, 0.52, None),
            hero(2, 1_000, 0.52, None),
            hero(3, 10_000, 0.48, None),
        ];
        score_all(&mut heroes, MetaWeights::default());

        assert!(
            heroes[0].meta_strength > heroes[1].meta_strength,
            "the contested hero must outrank the ignored one at equal win rate"
        );
    }

    #[test]
    fn pick_rate_is_a_share_of_the_cohort() {
        let mut heroes = vec![hero(1, 750, 0.50, None), hero(2, 250, 0.50, None)];
        score_all(&mut heroes, MetaWeights::default());

        assert!((heroes[0].pick_rate - 0.75).abs() < 0.001);
        assert!((heroes[1].pick_rate - 0.25).abs() < 0.001);
    }

    #[test]
    fn a_thin_sample_is_pulled_toward_neutral() {
        let mut heroes = cohort();
        // A fourth hero with a spectacular win rate over 50 games.
        heroes.push(hero(4, 50, 0.70, None));
        score_all(&mut heroes, MetaWeights::default());

        let thin = heroes.iter().find(|h| h.hero_id == 4).unwrap();
        let solid = heroes.iter().find(|h| h.hero_id == 3).unwrap();

        assert!(
            thin.meta_strength < solid.meta_strength,
            "50 games at 70% must not outrank 10,000 games at 55%"
        );
        // And it stays near neutral rather than being punished into the floor.
        assert!((thin.meta_strength - 50.0).abs() < 10.0);
    }

    #[test]
    fn a_rising_hero_outscores_an_identical_flat_one() {
        let mut heroes = vec![
            hero(1, 10_000, 0.52, Some(0.02)),
            hero(2, 10_000, 0.52, Some(0.0)),
            hero(3, 10_000, 0.52, Some(-0.02)),
        ];
        score_all(&mut heroes, MetaWeights::default());

        assert!(heroes[0].meta_strength > heroes[1].meta_strength);
        assert!(heroes[1].meta_strength > heroes[2].meta_strength);
    }

    #[test]
    fn a_missing_trend_is_neutral_rather_than_a_penalty() {
        let mut with_trend = vec![
            hero(1, 10_000, 0.52, Some(0.0)),
            hero(2, 10_000, 0.48, None),
        ];
        let mut without = vec![hero(1, 10_000, 0.52, None), hero(2, 10_000, 0.48, None)];

        score_all(&mut with_trend, MetaWeights::default());
        score_all(&mut without, MetaWeights::default());

        assert!((with_trend[0].meta_strength - without[0].meta_strength).abs() < 0.001);
    }

    #[test]
    fn scores_stay_inside_the_scale_for_absurd_inputs() {
        let mut heroes = vec![
            hero(1, 1_000_000, 0.99, Some(0.9)),
            hero(2, 1_000_000, 0.01, Some(-0.9)),
        ];
        score_all(&mut heroes, MetaWeights::default());

        for h in &heroes {
            assert!((0.0..=100.0).contains(&h.meta_strength), "{h:?}");
        }
    }

    #[test]
    fn an_identical_cohort_does_not_divide_by_zero() {
        let mut heroes = vec![hero(1, 10_000, 0.5, None), hero(2, 10_000, 0.5, None)];
        score_all(&mut heroes, MetaWeights::default());

        for h in &heroes {
            assert!(h.meta_strength.is_finite());
        }
    }

    #[test]
    fn an_empty_cohort_is_a_no_op() {
        let mut heroes: Vec<HeroMeta> = Vec::new();
        score_all(&mut heroes, MetaWeights::default());
    }
}
