//! The Overview's KPI row: each headline number against the window before it.
//!
//! Pure functions over stored matches, so the deltas are deterministic and
//! testable. The client renders them; it never subtracts.

use chrono::{DateTime, Duration, Utc};

use crate::domain::eligibility;
use crate::domain::metrics::{KpiTrend, MatchListSummary, OverviewTrends};
use crate::domain::r#match::Match;
use crate::services::metrics::kda;

/// Matches in each comparison window.
pub const WINDOW: usize = 20;
/// Games behind each point of the rolling win rate.
pub const ROLLING: usize = 10;
/// Weeks in the matches-per-week sparkline.
const WEEKS: i64 = 8;

/// `recent` is the newest eligible matches, newest first (at least
/// `2 * WINDOW` if the player has them). `since_60d` is every stored match
/// from the last 60 days, any mode; ineligible ones are skipped here.
pub fn overview(recent: &[Match], since_60d: &[Match], now: DateTime<Utc>) -> OverviewTrends {
    let latest = &recent[..recent.len().min(WINDOW)];
    let before = &recent[latest.len()..recent.len().min(2 * WINDOW)];
    // Oldest first, for the series.
    let chronological: Vec<&Match> = latest.iter().rev().collect();

    OverviewTrends {
        window: WINDOW as i64,
        win_rate: KpiTrend::compare(
            win_rate(latest),
            win_rate(before),
            rolling_win_rate(recent),
        ),
        kda: KpiTrend::compare(
            mean(latest, |m| kda(m.kills, m.deaths, m.assists)),
            mean(before, |m| kda(m.kills, m.deaths, m.assists)),
            chronological.iter().map(|m| kda(m.kills, m.deaths, m.assists)).collect(),
        ),
        gpm: KpiTrend::compare(
            mean(latest, |m| m.gpm as f32),
            mean(before, |m| m.gpm as f32),
            chronological.iter().map(|m| m.gpm as f32).collect(),
        ),
        matches_30d: matches_per_month(since_60d, now),
        rolling: ROLLING as i64,
    }
}

impl KpiTrend {
    fn compare(value: Option<f32>, previous: Option<f32>, series: Vec<f32>) -> Self {
        let delta = value.zip(previous).map(|(v, p)| v - p);
        KpiTrend {
            value,
            previous,
            delta,
            series,
        }
    }
}

fn win_rate(matches: &[Match]) -> Option<f32> {
    mean(matches, |m| if m.won { 100.0 } else { 0.0 })
}

fn mean(matches: &[Match], f: impl Fn(&Match) -> f32) -> Option<f32> {
    (!matches.is_empty()).then(|| matches.iter().map(f).sum::<f32>() / matches.len() as f32)
}

/// One point per game in the latest window: the win rate over the `ROLLING`
/// games ending there. Points without a full `ROLLING` games behind them are
/// left out rather than computed over fewer.
fn rolling_win_rate(recent: &[Match]) -> Vec<f32> {
    let oldest_first: Vec<&Match> = recent.iter().rev().collect();
    let start = oldest_first.len().saturating_sub(WINDOW);
    (start..oldest_first.len())
        .filter(|&end| end + 1 >= ROLLING)
        .map(|end| {
            let games = &oldest_first[end + 1 - ROLLING..=end];
            games.iter().filter(|m| m.won).count() as f32 / ROLLING as f32 * 100.0
        })
        .collect()
}

fn matches_per_month(since_60d: &[Match], now: DateTime<Utc>) -> KpiTrend {
    let eligible: Vec<&Match> = since_60d
        .iter()
        .filter(|m| eligibility::is_eligible(m.game_mode, m.lobby_type) && m.started_at <= now)
        .collect();
    let count_between = |from: DateTime<Utc>, to: DateTime<Utc>| {
        eligible
            .iter()
            .filter(|m| m.started_at > from && m.started_at <= to)
            .count() as f32
    };

    let month = Duration::days(30);
    let weekly = (0..WEEKS)
        .rev()
        .map(|w| {
            let to = now - Duration::weeks(w);
            count_between(to - Duration::weeks(1), to)
        })
        .collect();

    KpiTrend::compare(
        Some(count_between(now - month, now)),
        Some(count_between(now - month - month, now - month)),
        weekly,
    )
}

/// Matches the list summary reads at most.
pub const SUMMARY_LIMIT: usize = 500;

/// The summary strip over a filtered list, newest first. Reads at most
/// `SUMMARY_LIMIT`; `total` is how many the filter matched.
pub fn list_summary(matches: &[Match], total: i64) -> MatchListSummary {
    let read = &matches[..matches.len().min(SUMMARY_LIMIT)];
    let wins = read.iter().filter(|m| m.won).count() as i64;
    let recent: Vec<bool> = read.iter().take(WINDOW).rev().map(|m| m.won).collect();

    MatchListSummary {
        matches: read.len() as i64,
        capped: total > read.len() as i64,
        wins,
        win_rate: win_rate(read),
        avg_kda: mean(read, |m| kda(m.kills, m.deaths, m.assists)),
        avg_gpm: mean(read, |m| m.gpm as f32),
        recent_results: recent,
        rolling_win_rate: rolling_win_rate(read),
        rolling: ROLLING as i64,
        best_gpm_match_id: read.iter().max_by_key(|m| m.gpm).map(|m| m.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::eligibility::{game_mode, lobby_type};
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap()
    }

    fn game(i: i64, won: bool, gpm: i32, days_ago: i64) -> Match {
        let at = now() - Duration::days(days_ago);
        Match {
            id: uuid::Uuid::new_v4(),
            dota_player_id: uuid::Uuid::nil(),
            match_id: i,
            hero_id: 1,
            hero_name: "Anti-Mage".into(),
            role: "Carry".into(),
            role_source: "estimated".into(),
            lane_role: Some(1),
            won,
            duration_seconds: 2_400,
            kills: 5,
            deaths: 5,
            assists: 5,
            gpm,
            xpm: 600,
            last_hits: 240,
            denies: None,
            net_worth: None,
            hero_damage: None,
            tower_damage: None,
            hero_healing: None,
            game_mode: Some(game_mode::ALL_DRAFT),
            lobby_type: Some(lobby_type::RANKED),
            party_size: Some(1),
            started_at: at,
            detail_synced: true,
            team_kills: Some(30),
            team_deaths: Some(25),
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
            created_at: at,
            updated_at: at,
            metrics_kda: None,
        }
    }

    /// Newest first: `n` games, one a day.
    fn history(n: i64, won: impl Fn(i64) -> bool, gpm: impl Fn(i64) -> i32) -> Vec<Match> {
        (0..n).map(|i| game(i, won(i), gpm(i), i)).collect()
    }

    #[test]
    fn each_kpi_is_the_latest_window_against_the_one_before() {
        // Newest 20 win and farm 500; the 20 before lose and farm 400.
        let recent = history(40, |i| i < 20, |i| if i < 20 { 500 } else { 400 });

        let t = overview(&recent, &[], now());

        assert_eq!(t.win_rate.value, Some(100.0));
        assert_eq!(t.win_rate.previous, Some(0.0));
        assert_eq!(t.win_rate.delta, Some(100.0));
        assert_eq!(t.gpm.value, Some(500.0));
        assert_eq!(t.gpm.delta, Some(100.0));
        assert_eq!(t.kda.value, Some(2.0));
        assert_eq!(t.kda.delta, Some(0.0));
        assert_eq!(t.gpm.series.len(), 20);
    }

    #[test]
    fn no_previous_window_means_no_delta() {
        let recent = history(12, |_| true, |_| 450);

        let t = overview(&recent, &[], now());

        assert_eq!(t.win_rate.value, Some(100.0));
        assert_eq!(t.win_rate.previous, None);
        assert_eq!(t.win_rate.delta, None, "a delta is never made from one side");
    }

    #[test]
    fn the_rolling_win_rate_needs_a_full_window_behind_each_point() {
        // 12 games: points exist only from the 10th game on.
        let recent = history(12, |i| i % 2 == 0, |_| 450);

        let t = overview(&recent, &[], now());

        assert_eq!(t.win_rate.series.len(), 3);
        assert!(t.win_rate.series.iter().all(|v| *v == 50.0));
    }

    #[test]
    fn matches_this_month_skip_turbo_and_compare_to_last_month() {
        let mut since = history(10, |_| true, |_| 450); // days 0-9
        since.extend((0..4).map(|i| game(100 + i, true, 450, 40 + i))); // days 40-43
        let mut turbo = game(200, true, 450, 1);
        turbo.game_mode = Some(game_mode::TURBO);
        since.push(turbo);

        let t = overview(&[], &since, now());

        assert_eq!(t.matches_30d.value, Some(10.0));
        assert_eq!(t.matches_30d.previous, Some(4.0));
        assert_eq!(t.matches_30d.delta, Some(6.0));
        assert_eq!(t.matches_30d.series.len(), 8);
        assert_eq!(t.matches_30d.series.iter().sum::<f32>(), 14.0, "8 weeks reach day 43; Turbo excluded");
    }

    #[test]
    fn an_empty_history_has_no_values() {
        let t = overview(&[], &[], now());
        assert_eq!(t.gpm.value, None);
        assert!(t.gpm.series.is_empty());
        assert_eq!(t.matches_30d.value, Some(0.0));
    }

    #[test]
    fn the_list_summary_reads_the_filtered_games() {
        let mut games = history(25, |i| i % 5 != 0, |i| 400 + i as i32);
        games[3].gpm = 900;

        let s = list_summary(&games, 25);

        assert_eq!(s.matches, 25);
        assert!(!s.capped);
        assert_eq!(s.wins, 20);
        assert_eq!(s.win_rate, Some(80.0));
        assert_eq!(s.avg_kda, Some(2.0));
        assert_eq!(s.recent_results.len(), 20);
        assert_eq!(s.best_gpm_match_id, Some(games[3].id));
        // Oldest first: the newest game (index 0, a loss) is last.
        assert_eq!(s.recent_results.last(), Some(&false));
    }

    #[test]
    fn a_summary_over_more_than_it_read_says_so() {
        let games = history(5, |_| true, |_| 450);
        assert!(list_summary(&games, 900).capped);
        assert_eq!(list_summary(&[], 0).win_rate, None);
    }
}
