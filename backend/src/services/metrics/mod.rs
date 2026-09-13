//! Deterministic metric calculation.
//!
//! Every value here is a pure function of stored match facts. Nothing calls a
//! provider, nothing calls an LLM, and running it twice on the same row gives
//! the same answer — that is what lets the coaching layer *interpret* numbers
//! it did not invent.
//!
//! Where an input is missing (an unparsed replay, a match detail that never
//! arrived), the output is `None`. It is never defaulted to zero: a missing
//! last-hit count and a genuine zero mean very different things to a coach.

use crate::domain::metrics::MatchMetrics;
use crate::domain::r#match::Match;

/// Bump when any formula below changes, so stored rows stay identifiable.
pub const METRICS_VERSION: i32 = 1;

/// Matches shorter than this are abandons or remakes; per-minute rates from
/// them are noise, so the duration floor keeps them from exploding.
const MIN_MINUTES: f64 = 1.0;

/// Compute every derived metric for one match.
pub fn compute(m: &Match) -> MatchMetrics {
    let minutes = duration_minutes(m.duration_seconds);
    let per_10 = |value: i32| (value as f64 / minutes) * 10.0;

    MatchMetrics {
        match_id: m.id,
        metrics_version: METRICS_VERSION,

        kda: kda(m.kills, m.deaths, m.assists),
        kills_per_10: per_10(m.kills) as f32,
        deaths_per_10: per_10(m.deaths) as f32,
        assists_per_10: per_10(m.assists) as f32,
        last_hits_per_min: (m.last_hits as f64 / minutes) as f32,

        hero_damage_per_min: m.hero_damage.map(|d| (d as f64 / minutes) as f32),
        tower_damage_per_min: m.tower_damage.map(|d| (d as f64 / minutes) as f32),

        kill_participation: kill_participation(m.kills, m.assists, m.team_kills),
        gold_advantage_at_10: None,
    }
}

/// `(kills + assists) / max(deaths, 1)`.
///
/// Dividing by 1 on a deathless game is the conventional Dota definition; it
/// keeps the value finite without pretending the player died.
pub fn kda(kills: i32, deaths: i32, assists: i32) -> f32 {
    (kills + assists) as f32 / deaths.max(1) as f32
}

/// Share of the team's kills the player took part in, 0-1.
///
/// `None` when the team total is unknown, and when it is zero — a team that
/// killed nobody gives no information about participation, and 0/0 is not 0%.
pub fn kill_participation(kills: i32, assists: i32, team_kills: Option<i32>) -> Option<f32> {
    let total = team_kills?;
    if total <= 0 {
        return None;
    }

    // Cap at 1.0: the provider's team total can lag a player's own counters.
    Some((((kills + assists) as f32) / total as f32).min(1.0))
}

/// Deaths per 10 minutes of game time.
pub fn deaths_per_10(deaths: i32, duration_seconds: i32) -> f32 {
    ((deaths as f64 / duration_minutes(duration_seconds)) * 10.0) as f32
}

fn duration_minutes(duration_seconds: i32) -> f64 {
    (duration_seconds.max(0) as f64 / 60.0).max(MIN_MINUTES)
}

/// Read a per-minute series at a given minute.
///
/// OpenDota indexes these arrays by minute, so `at(lh_t, 10)` is "last hits at
/// the ten minute mark". A game that ended before that minute simply has no
/// value there.
pub fn series_at(series: &[i32], minute: usize) -> Option<i32> {
    series.get(minute).copied()
}

/// First purchase time for an item, in seconds from the horn.
///
/// Pre-horn purchases carry a negative time in the provider's log; those are
/// starting items and are never the timing anyone means.
pub fn first_purchase_seconds(log: &[(String, i32)], item_key: &str) -> Option<i32> {
    log.iter()
        .filter(|(key, time)| key == item_key && *time >= 0)
        .map(|(_, time)| *time)
        .min()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kda_counts_kills_and_assists_against_deaths() {
        assert_eq!(kda(8, 4, 12), 5.0);
        assert_eq!(kda(0, 5, 0), 0.0);
    }

    #[test]
    fn a_deathless_game_divides_by_one_rather_than_zero() {
        assert_eq!(kda(5, 0, 5), 10.0);
        assert!(kda(5, 0, 5).is_finite());
    }

    #[test]
    fn deaths_per_10_scales_to_game_length() {
        // 6 deaths in 30 minutes = 2 per 10.
        assert_eq!(deaths_per_10(6, 1_800), 2.0);
        // The same 6 deaths in 60 minutes is half the rate.
        assert_eq!(deaths_per_10(6, 3_600), 1.0);
    }

    #[test]
    fn a_zero_length_match_cannot_divide_by_zero() {
        let rate = deaths_per_10(3, 0);
        assert!(rate.is_finite(), "got {rate}");
        // Floored at one minute, so 3 deaths reads as 30 per 10 minutes.
        assert_eq!(rate, 30.0);
    }

    #[test]
    fn kill_participation_is_a_share_of_team_kills() {
        assert_eq!(kill_participation(5, 10, Some(30)), Some(0.5));
    }

    #[test]
    fn kill_participation_is_unknown_rather_than_zero_without_team_totals() {
        assert_eq!(kill_participation(5, 10, None), None);
        // A team with no kills says nothing about participation.
        assert_eq!(kill_participation(0, 0, Some(0)), None);
    }

    #[test]
    fn kill_participation_never_exceeds_one() {
        // Provider counters can disagree slightly; the share stays a share.
        assert_eq!(kill_participation(10, 10, Some(15)), Some(1.0));
    }

    #[test]
    fn a_series_reads_by_minute_and_stops_at_the_end_of_the_game() {
        let lh: Vec<i32> = (0..=20).collect();
        assert_eq!(series_at(&lh, 10), Some(10));
        assert_eq!(series_at(&lh, 15), Some(15));
        // A 12-minute game has no 15-minute mark.
        assert_eq!(series_at(&lh[..13], 15), None);
    }

    #[test]
    fn item_timing_ignores_pre_horn_shopping() {
        let log = vec![
            ("tango".to_string(), -89),
            ("black_king_bar".to_string(), 1_420),
            ("black_king_bar".to_string(), 2_600),
        ];

        assert_eq!(first_purchase_seconds(&log, "black_king_bar"), Some(1_420));
    }

    #[test]
    fn an_item_never_bought_has_no_timing() {
        let log = vec![("tango".to_string(), -89)];
        assert_eq!(first_purchase_seconds(&log, "blink"), None);
    }

    #[test]
    fn a_starting_item_bought_only_pre_horn_has_no_timing() {
        // Negative-only entries must not become a "0 second" timing.
        let log = vec![("branches".to_string(), -89)];
        assert_eq!(first_purchase_seconds(&log, "branches"), None);
    }
}
