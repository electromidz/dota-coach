//! A single 1–10 number for how one match went.
//!
//! The match list needs one figure a player can scan down a column, and the
//! honest version of that figure is *relative to themselves*. A 420 GPM game is
//! excellent for one account and a poor one for another, so nothing here
//! compares against an absolute scale or a peer distribution: every term is the
//! match measured against the player's own typical game on that hero (see
//! [`RatingBaseline`]).
//!
//! Deterministic and pure, like the rest of `services::metrics` — the model is
//! never asked what a game was worth, it is only shown the number (CLAUDE.md
//! §8). The weights are named constants rather than literals because the
//! product spec expects them to be tuned.
//!
//! What a missing baseline does is the important rule: it contributes **zero**,
//! not a penalty. A player's first game on a hero has nothing to be measured
//! against, and a first game that reads as a 2.6 would be the fabricated
//! precision the product exists to avoid.

use std::collections::HashMap;

use crate::domain::eligibility;
use crate::domain::metrics::MatchRatingBaseline;
use crate::domain::r#match::Match;
use crate::services::metrics;

/// Where a rating starts before any evidence moves it. The midpoint of the
/// scale: a match with no baseline at all is "unremarkable", not "bad".
const BASE: f32 = 5.0;

/// How far each term may move the rating, in points.
///
/// KDA leads because it is the only term present for every match, whatever the
/// provider knew. Hero damage is last and lightest because it is `Option` on
/// most public matches and swings hardest by role — a hard support is not a bad
/// player for dealing less of it than a mid.
const KDA_WEIGHT: f32 = 1.8;
const GPM_WEIGHT: f32 = 1.2;
const XPM_WEIGHT: f32 = 1.0;
const DAMAGE_WEIGHT: f32 = 0.8;

/// Winning is evidence about the match, so it moves the number — but it is one
/// term among five, so a carried win does not read as a 10 and a hard-fought
/// loss does not read as a 1.
const WIN_BONUS: f32 = 0.7;

/// How far above or below the baseline a ratio term is allowed to count.
///
/// A 2× GPM game and a 5× GPM game are both "far above your usual"; letting the
/// second one count two and a half times as hard would hand the whole rating to
/// one stomp. Clamped rather than curved so the arithmetic stays explainable.
const MAX_RATIO_SWING: f32 = 1.0;

pub const MIN_RATING: f32 = 1.0;
pub const MAX_RATING: f32 = 10.0;

/// Fewer matches than this behind a baseline and it is not a yardstick.
///
/// Read by the caller resolving which baseline a row gets: below it, the
/// per-hero figures are dropped in favour of the player-wide ones rather than
/// rating a game against a single previous game.
pub const MIN_BASELINE_SAMPLE: i64 = 3;

/// The player's own typical game, as the yardstick for one match.
///
/// Every field is optional and every one of them is optional for the same
/// reason: the figure may not exist yet. `None` means "no basis for comparison",
/// which [`rating`] reads as "no adjustment" — never as zero.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RatingBaseline {
    /// How many matches the figures below were averaged over.
    pub sample: i64,
    pub median_kda: Option<f32>,
    pub avg_gpm: Option<f32>,
    pub avg_xpm: Option<f32>,
    pub avg_hero_damage_per_min: Option<f32>,
}

impl RatingBaseline {
    /// Nothing to compare against. Rates every match on its result alone.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Whether this baseline is built on enough matches to be worth using.
    pub fn is_usable(&self) -> bool {
        self.sample >= MIN_BASELINE_SAMPLE
    }
}

/// Every baseline a page of matches might need, with the resolution rule.
///
/// Built once per request from [`MatchRatingBaseline`] rows and asked per row,
/// so twenty matches on twelve heroes cost one query. The resolution order is
/// the whole point of the type: the hero's own figures when there are enough of
/// them, the player's overall figures otherwise, and nothing at all for an
/// account with no computed metrics yet.
#[derive(Debug, Default)]
pub struct RatingBaselines {
    /// Keyed by `(hero_id, turbo)`; the `None` hero is the player-wide row.
    by_key: HashMap<(Option<i32>, bool), RatingBaseline>,
}

impl RatingBaselines {
    pub fn from_rows(rows: &[MatchRatingBaseline]) -> Self {
        Self {
            by_key: rows
                .iter()
                .map(|row| {
                    (
                        (row.hero_id, row.turbo),
                        RatingBaseline {
                            sample: row.sample,
                            median_kda: row.median_kda,
                            avg_gpm: row.avg_gpm,
                            avg_xpm: row.avg_xpm,
                            avg_hero_damage_per_min: row.avg_hero_damage_per_min,
                        },
                    )
                })
                .collect(),
        }
    }

    /// The yardstick this match should be measured against.
    ///
    /// Turbo matches only ever resolve to Turbo baselines and standard matches
    /// only to standard ones, at both levels — that separation is why the
    /// fallback is safe to take.
    pub fn for_match(&self, m: &Match) -> RatingBaseline {
        let turbo = m.game_mode == Some(eligibility::game_mode::TURBO);

        let hero = self.by_key.get(&(Some(m.hero_id), turbo));
        if let Some(baseline) = hero.filter(|b| b.is_usable()) {
            return *baseline;
        }

        self.by_key
            .get(&(None, turbo))
            .filter(|b| b.is_usable())
            .copied()
            .unwrap_or_else(RatingBaseline::empty)
    }
}

/// How this match went for this player, from 1.0 to 10.0, to one decimal.
pub fn rating(m: &Match, baseline: &RatingBaseline) -> f32 {
    let minutes = (m.duration_seconds.max(0) as f32 / 60.0).max(1.0);

    let kda = metrics::kda(m.kills, m.deaths, m.assists);
    let damage_per_min = m.hero_damage.map(|d| d as f32 / minutes);

    let score = BASE
        + KDA_WEIGHT * ratio_term(Some(kda), baseline.median_kda)
        + GPM_WEIGHT * ratio_term(Some(m.gpm as f32), baseline.avg_gpm)
        + XPM_WEIGHT * ratio_term(Some(m.xpm as f32), baseline.avg_xpm)
        + DAMAGE_WEIGHT * ratio_term(damage_per_min, baseline.avg_hero_damage_per_min)
        + if m.won { WIN_BONUS } else { -WIN_BONUS };

    round_1dp(score.clamp(MIN_RATING, MAX_RATING))
}

/// One term, as a signed fraction in `[-MAX_RATIO_SWING, MAX_RATIO_SWING]`.
///
/// Zero whenever the comparison cannot be made: no value, no baseline, a
/// baseline of zero, or anything non-finite. That is the neutral answer, and it
/// is deliberately indistinguishable from "exactly average" — both mean this
/// term has nothing to say about the match.
fn ratio_term(value: Option<f32>, baseline: Option<f32>) -> f32 {
    let (Some(value), Some(baseline)) = (value, baseline) else {
        return 0.0;
    };
    if baseline <= 0.0 || !baseline.is_finite() || !value.is_finite() {
        return 0.0;
    }

    let deviation = value / baseline - 1.0;
    if !deviation.is_finite() {
        return 0.0;
    }

    deviation.clamp(-MAX_RATIO_SWING, MAX_RATIO_SWING)
}

fn round_1dp(value: f32) -> f32 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    /// A 40-minute ranked win with unremarkable figures.
    fn sample() -> Match {
        Match {
            id: Uuid::nil(),
            dota_player_id: Uuid::nil(),
            match_id: 1,
            hero_id: 26,
            hero_name: "Lion".into(),
            role: "Hard Support".into(),
            lane_role: Some(1),
            won: true,
            duration_seconds: 2_400,
            kills: 5,
            deaths: 5,
            assists: 15,
            gpm: 400,
            xpm: 500,
            last_hits: 80,
            denies: None,
            net_worth: None,
            hero_damage: Some(24_000),
            tower_damage: None,
            hero_healing: None,
            game_mode: Some(22),
            lobby_type: Some(7),
            party_size: None,
            started_at: Utc::now(),
            detail_synced: true,
            team_kills: None,
            team_deaths: None,
            replay_parsed: false,
            last_hits_at_10: None,
            last_hits_at_15: None,
            gold_at_10: None,
            gold_at_15: None,
            xp_at_10: None,
            xp_at_15: None,
            bkb_seconds: None,
            blink_seconds: None,
            midas_seconds: None,
            teamfight_participation: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metrics_kda: None,
        }
    }

    /// The baseline that says "this match was exactly your usual game".
    fn matching_baseline(m: &Match) -> RatingBaseline {
        RatingBaseline {
            sample: 20,
            median_kda: Some(metrics::kda(m.kills, m.deaths, m.assists)),
            avg_gpm: Some(m.gpm as f32),
            avg_xpm: Some(m.xpm as f32),
            avg_hero_damage_per_min: Some(
                m.hero_damage.unwrap() as f32 / (m.duration_seconds as f32 / 60.0),
            ),
        }
    }

    #[test]
    fn a_typical_win_sits_just_above_the_midpoint() {
        let m = sample();
        assert_eq!(rating(&m, &matching_baseline(&m)), BASE + WIN_BONUS);
    }

    #[test]
    fn a_typical_loss_sits_just_below_the_midpoint() {
        let mut m = sample();
        let baseline = matching_baseline(&m);
        m.won = false;
        assert_eq!(rating(&m, &baseline), BASE - WIN_BONUS);
    }

    #[test]
    fn a_first_game_on_a_hero_is_not_penalised() {
        let m = sample();
        // No baseline at all: only the result may move the number.
        assert_eq!(rating(&m, &RatingBaseline::empty()), BASE + WIN_BONUS);

        let mut loss = sample();
        loss.won = false;
        assert_eq!(rating(&loss, &RatingBaseline::empty()), BASE - WIN_BONUS);
    }

    #[test]
    fn a_zero_baseline_is_treated_as_no_baseline() {
        let m = sample();
        let zeroed = RatingBaseline {
            sample: 20,
            median_kda: Some(0.0),
            avg_gpm: Some(0.0),
            avg_xpm: Some(0.0),
            avg_hero_damage_per_min: Some(0.0),
        };
        assert_eq!(rating(&m, &zeroed), BASE + WIN_BONUS);
    }

    #[test]
    fn beating_your_own_average_raises_the_rating() {
        let m = sample();
        let baseline = matching_baseline(&m);

        let mut better = sample();
        better.kills = 15;
        better.deaths = 2;
        better.gpm = 700;
        better.xpm = 800;
        better.hero_damage = Some(50_000);

        assert!(rating(&better, &baseline) > rating(&m, &baseline));
    }

    #[test]
    fn falling_short_of_your_own_average_lowers_the_rating() {
        let m = sample();
        let baseline = matching_baseline(&m);

        let mut worse = sample();
        worse.kills = 0;
        worse.deaths = 14;
        worse.assists = 1;
        worse.gpm = 180;
        worse.xpm = 220;
        worse.hero_damage = Some(4_000);

        assert!(rating(&worse, &baseline) < rating(&m, &baseline));
    }

    #[test]
    fn a_missing_hero_damage_is_neutral_rather_than_zero() {
        let m = sample();
        let baseline = matching_baseline(&m);

        let mut unparsed = sample();
        unparsed.hero_damage = None;

        // Every other term matches the baseline, so both land on the same
        // number: the absent term dropped out instead of scoring zero damage.
        assert_eq!(rating(&unparsed, &baseline), rating(&m, &baseline));
    }

    #[test]
    fn the_scale_is_clamped_at_both_ends() {
        let baseline = RatingBaseline {
            sample: 20,
            median_kda: Some(4.0),
            avg_gpm: Some(400.0),
            avg_xpm: Some(500.0),
            avg_hero_damage_per_min: Some(600.0),
        };

        let mut best = sample();
        best.kills = 40;
        best.deaths = 0;
        best.assists = 40;
        best.gpm = 5_000;
        best.xpm = 6_000;
        best.hero_damage = Some(400_000);
        let top = rating(&best, &baseline);
        assert!((MIN_RATING..=MAX_RATING).contains(&top), "got {top}");

        let mut worst = sample();
        worst.won = false;
        worst.kills = 0;
        worst.deaths = 20;
        worst.assists = 0;
        worst.gpm = 0;
        worst.xpm = 0;
        worst.hero_damage = Some(0);
        let bottom = rating(&worst, &baseline);
        assert!((MIN_RATING..=MAX_RATING).contains(&bottom), "got {bottom}");
        assert!(bottom < top);
    }

    #[test]
    fn a_huge_outlier_cannot_swing_further_than_the_cap() {
        let baseline = RatingBaseline {
            sample: 20,
            median_kda: Some(3.0),
            avg_gpm: Some(400.0),
            avg_xpm: Some(500.0),
            avg_hero_damage_per_min: Some(600.0),
        };

        let mut good = sample();
        good.gpm = 800; // exactly 2x the baseline: the cap
        let mut absurd = sample();
        absurd.gpm = 8_000; // ten times it

        assert_eq!(rating(&good, &baseline), rating(&absurd, &baseline));
    }

    #[test]
    fn the_result_is_always_one_decimal() {
        let m = sample();
        let baseline = RatingBaseline {
            sample: 20,
            median_kda: Some(2.7),
            avg_gpm: Some(437.0),
            avg_xpm: Some(561.0),
            avg_hero_damage_per_min: Some(613.0),
        };

        let value = rating(&m, &baseline);
        assert_eq!(value, round_1dp(value));
    }

    #[test]
    fn a_zero_length_match_cannot_divide_by_zero() {
        let mut m = sample();
        m.duration_seconds = 0;
        let value = rating(&m, &matching_baseline(&sample()));
        assert!(value.is_finite() && (MIN_RATING..=MAX_RATING).contains(&value));
    }

    fn row(
        hero_id: Option<i32>,
        turbo: bool,
        sample: i64,
        avg_gpm: f32,
    ) -> MatchRatingBaseline {
        MatchRatingBaseline {
            hero_id,
            turbo,
            sample,
            median_kda: Some(3.0),
            avg_gpm: Some(avg_gpm),
            avg_xpm: Some(500.0),
            avg_hero_damage_per_min: Some(600.0),
        }
    }

    #[test]
    fn a_hero_with_enough_games_is_rated_against_itself() {
        let baselines = RatingBaselines::from_rows(&[
            row(Some(26), false, 12, 380.0),
            row(None, false, 200, 520.0),
        ]);

        assert_eq!(baselines.for_match(&sample()).avg_gpm, Some(380.0));
    }

    #[test]
    fn a_hero_with_too_few_games_falls_back_to_the_player() {
        let baselines = RatingBaselines::from_rows(&[
            row(Some(26), false, MIN_BASELINE_SAMPLE - 1, 380.0),
            row(None, false, 200, 520.0),
        ]);

        assert_eq!(baselines.for_match(&sample()).avg_gpm, Some(520.0));
    }

    #[test]
    fn turbo_is_never_rated_against_standard_games() {
        let baselines = RatingBaselines::from_rows(&[
            row(Some(26), false, 12, 380.0),
            row(None, false, 200, 520.0),
            row(Some(26), true, 8, 900.0),
            row(None, true, 30, 880.0),
        ]);

        let mut turbo = sample();
        turbo.game_mode = Some(eligibility::game_mode::TURBO);
        assert_eq!(baselines.for_match(&turbo).avg_gpm, Some(900.0));

        // And a standard game never borrows the Turbo figures.
        assert_eq!(baselines.for_match(&sample()).avg_gpm, Some(380.0));
    }

    #[test]
    fn a_turbo_game_with_no_turbo_history_gets_no_baseline_rather_than_the_standard_one() {
        let baselines = RatingBaselines::from_rows(&[
            row(Some(26), false, 12, 380.0),
            row(None, false, 200, 520.0),
        ]);

        let mut turbo = sample();
        turbo.game_mode = Some(eligibility::game_mode::TURBO);
        assert_eq!(baselines.for_match(&turbo), RatingBaseline::empty());
    }

    #[test]
    fn an_account_with_no_computed_metrics_resolves_to_nothing() {
        let baselines = RatingBaselines::default();
        assert_eq!(baselines.for_match(&sample()), RatingBaseline::empty());
    }

    #[test]
    fn a_match_with_no_reported_mode_is_rated_as_a_standard_game() {
        let baselines = RatingBaselines::from_rows(&[row(None, false, 200, 520.0)]);

        let mut unknown = sample();
        unknown.game_mode = None;
        assert_eq!(baselines.for_match(&unknown).avg_gpm, Some(520.0));
    }

    #[test]
    fn a_thin_baseline_is_not_usable() {
        assert!(!RatingBaseline::empty().is_usable());
        assert!(!RatingBaseline {
            sample: MIN_BASELINE_SAMPLE - 1,
            ..RatingBaseline::empty()
        }
        .is_usable());
        assert!(RatingBaseline {
            sample: MIN_BASELINE_SAMPLE,
            ..RatingBaseline::empty()
        }
        .is_usable());
    }
}
