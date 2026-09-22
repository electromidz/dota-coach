use chrono::{Duration, TimeZone, Utc};
use uuid::Uuid;

use super::*;
use crate::domain::eligibility::{game_mode, lobby_type};

fn config() -> CalibrationConfig {
    CalibrationConfig {
        confidence_per_match_pct: 1.5,
        confidence_threshold_pct: 30.0,
        decay_days: 180,
        win_base_mmr: 30.0,
        loss_base_mmr: 25.0,
    }
}

/// Fixed so nothing here depends on the wall clock.
fn day(n: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap() + Duration::days(n)
}

/// A stored ranked All Pick match. Every test starts from this and changes
/// only the field it is about.
fn ranked(match_id: i64, won: bool, at: DateTime<Utc>) -> Match {
    Match {
        id: Uuid::new_v4(),
        dota_player_id: Uuid::nil(),
        match_id,
        hero_id: 1,
        hero_name: "Anti-Mage".into(),
        role: "Carry".into(),
        lane_role: Some(1),
        won,
        duration_seconds: 2_400,
        kills: 6,
        deaths: 3,
        assists: 9,
        gpm: 500,
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

fn snapshot(tier: Option<i16>, at: DateTime<Utc>) -> RankSnapshot {
    RankSnapshot {
        rank_tier: tier,
        leaderboard_rank: None,
        captured_at: at,
    }
}

// ---------------------------------------------------------------------------
// rank_confidence
// ---------------------------------------------------------------------------

#[test]
fn no_matches_is_no_confidence_rather_than_a_default() {
    let confidence = rank_confidence(&[], &config(), day(0));

    assert_eq!(confidence.confidence_pct, 0.0);
    assert_eq!(confidence.matches_counted, 0);
    assert!(!confidence.is_calibrated);
}

#[test]
fn exactly_at_the_threshold_counts_as_calibrated() {
    // 20 matches * 1.5% = 30.0%, the configured threshold exactly.
    let matches: Vec<Match> = (0..20).map(|i| ranked(i, true, day(i))).collect();

    let confidence = rank_confidence(&matches, &config(), day(20));

    assert_eq!(confidence.matches_counted, 20);
    assert_eq!(confidence.confidence_pct, 30.0);
    assert!(
        confidence.is_calibrated,
        "the boundary is inclusive: 30% is calibrated, not one match short"
    );
}

#[test]
fn confidence_is_capped_at_one_hundred_percent() {
    let matches: Vec<Match> = (0..200).map(|i| ranked(i, true, day(i))).collect();

    let confidence = rank_confidence(&matches, &config(), day(200));

    assert_eq!(confidence.confidence_pct, 100.0);
    assert!(confidence.is_calibrated);
}

#[test]
fn a_gap_longer_than_the_decay_window_resets_the_count() {
    // Ten games, a seven-month break, then three more.
    let mut matches: Vec<Match> = (0..10).map(|i| ranked(i, true, day(i))).collect();
    matches.extend((0..3).map(|i| ranked(100 + i, true, day(220 + i))));

    let confidence = rank_confidence(&matches, &config(), day(223));

    assert_eq!(
        confidence.matches_counted, 3,
        "games from before the break are not evidence about where the player sits now"
    );
}

#[test]
fn an_account_idle_past_the_decay_window_reads_as_zero() {
    let matches: Vec<Match> = (0..40).map(|i| ranked(i, true, day(i))).collect();

    // Every match is real, but the newest is a year old.
    let confidence = rank_confidence(&matches, &config(), day(400));

    assert_eq!(confidence.matches_counted, 0);
    assert!(!confidence.is_calibrated);
}

#[test]
fn turbo_and_unranked_games_do_not_calibrate_a_rank() {
    let mut matches = vec![ranked(1, true, day(1))];

    let mut turbo = ranked(2, true, day(2));
    turbo.game_mode = Some(game_mode::TURBO);
    matches.push(turbo);

    // Unranked public matchmaking: part of the coaching population, but it
    // does not move a medal.
    let mut unranked = ranked(3, true, day(3));
    unranked.lobby_type = Some(lobby_type::NORMAL);
    matches.push(unranked);

    let mut tournament = ranked(4, true, day(4));
    tournament.lobby_type = Some(2);
    matches.push(tournament);

    let mut unknown = ranked(5, true, day(5));
    unknown.game_mode = None;
    matches.push(unknown);

    let confidence = rank_confidence(&matches, &config(), day(6));

    assert_eq!(
        confidence.matches_counted, 1,
        "only the ranked All Pick game counts"
    );
}

// ---------------------------------------------------------------------------
// streak
// ---------------------------------------------------------------------------

#[test]
fn an_empty_history_has_a_streak_with_no_direction() {
    let s = streak(&[]);

    assert_eq!(s.count, 0);
    assert_eq!(
        s.kind, None,
        "a zero streak has no direction; defaulting it would render 'Win 0'"
    );
}

#[test]
fn a_single_match_is_a_streak_of_one() {
    let s = streak(&[ranked(1, false, day(1))]);

    assert_eq!(s.count, 1);
    assert_eq!(s.kind, Some(StreakKind::Loss));
}

#[test]
fn alternating_results_never_streak_past_one() {
    let matches: Vec<Match> = (0..8).map(|i| ranked(i, i % 2 == 0, day(i))).collect();

    let s = streak(&matches);

    assert_eq!(s.count, 1);
}

#[test]
fn the_streak_is_read_from_the_newest_match_backwards() {
    // Three losses, then four wins. Input order is deliberately shuffled to
    // prove the function sorts rather than trusting the caller.
    let mut matches: Vec<Match> = (0..4).map(|i| ranked(i, true, day(10 + i))).collect();
    matches.extend((0..3).map(|i| ranked(100 + i, false, day(i))));
    matches.reverse();

    let s = streak(&matches);

    assert_eq!(s.count, 4);
    assert_eq!(s.kind, Some(StreakKind::Win));
}

#[test]
fn an_unranked_win_does_not_extend_a_ranked_losing_streak() {
    let mut matches = vec![ranked(1, false, day(1)), ranked(2, false, day(2))];

    let mut turbo_win = ranked(3, true, day(3));
    turbo_win.game_mode = Some(game_mode::TURBO);
    matches.push(turbo_win);

    let s = streak(&matches);

    assert_eq!(s.count, 2);
    assert_eq!(s.kind, Some(StreakKind::Loss));
}

// ---------------------------------------------------------------------------
// role_preference
// ---------------------------------------------------------------------------

#[test]
fn role_preference_reads_the_stored_label_and_sums_to_a_hundred() {
    let mut matches: Vec<Match> = Vec::new();
    for i in 0..5 {
        matches.push(ranked(i, true, day(i)));
    }
    for i in 0..3 {
        let mut m = ranked(100 + i, true, day(10 + i));
        m.role = "Offlane".into();
        matches.push(m);
    }
    for i in 0..2 {
        let mut m = ranked(200 + i, true, day(20 + i));
        m.role = "Mid".into();
        matches.push(m);
    }

    let roles = role_preference(&matches);

    assert_eq!(roles.len(), 3);
    assert_eq!(roles[0].role, "Carry", "most played first");
    assert_eq!(roles[0].matches, 5);
    assert_eq!(roles[0].pct, 50.0);
    assert_eq!(roles[1].role, "Offlane");
    assert_eq!(roles[2].role, "Mid");

    let total: f32 = roles.iter().map(|r| r.pct).sum();
    assert!((total - 100.0).abs() < 0.01, "shares add up: {total}");
}

#[test]
fn role_preference_is_empty_rather_than_a_zero_row_without_matches() {
    assert!(role_preference(&[]).is_empty());
}

// ---------------------------------------------------------------------------
// trajectory
// ---------------------------------------------------------------------------

#[test]
fn two_readings_with_no_matches_between_them_produce_no_estimates() {
    let snapshots = vec![snapshot(Some(43), day(0)), snapshot(Some(45), day(7))];

    let points = trajectory(&snapshots, &[], &config());

    assert_eq!(points.len(), 2);
    assert!(
        points.iter().all(|p| !p.estimated),
        "nothing was played, so there is nothing to model"
    );
    assert_eq!(points[0].rank_tier, 43);
    assert_eq!(points[1].rank_tier, 45);
}

#[test]
fn every_point_between_two_readings_is_flagged_estimated() {
    let snapshots = vec![snapshot(Some(43), day(0)), snapshot(Some(45), day(7))];
    let matches: Vec<Match> = (1..=5).map(|i| ranked(i, i % 2 == 0, day(i))).collect();

    let points = trajectory(&snapshots, &matches, &config());

    assert_eq!(points.len(), 7, "two real readings plus five played games");

    let real: Vec<&TrajectoryPoint> = points.iter().filter(|p| !p.estimated).collect();
    assert_eq!(real.len(), 2, "only the snapshots are real");
    assert_eq!(real[0].at, day(0));
    assert_eq!(real[1].at, day(7));

    for point in points.iter().filter(|p| p.at != day(0) && p.at != day(7)) {
        assert!(
            point.estimated,
            "a modeled point must never claim to be a measurement: {point:?}"
        );
    }
}

#[test]
fn a_modeled_segment_lands_on_the_next_real_reading() {
    let snapshots = vec![snapshot(Some(40), day(0)), snapshot(Some(44), day(10))];
    let matches: Vec<Match> = (1..=9).map(|i| ranked(i, true, day(i))).collect();

    let points = trajectory(&snapshots, &matches, &config());

    assert_eq!(points.first().unwrap().rank_tier, 40);
    assert_eq!(
        points.last().unwrap().rank_tier,
        44,
        "the model shapes the path, the snapshots fix both of its ends"
    );
    assert!(!points.last().unwrap().estimated);
}

#[test]
fn a_reading_with_no_rank_breaks_the_line_instead_of_drawing_across_it() {
    // The profile went private in the middle of the window.
    let snapshots = vec![
        snapshot(Some(40), day(0)),
        snapshot(None, day(5)),
        snapshot(Some(44), day(10)),
    ];
    let matches: Vec<Match> = (1..=9).map(|i| ranked(i, true, day(i))).collect();

    let points = trajectory(&snapshots, &matches, &config());

    assert_eq!(
        points.len(),
        2,
        "two isolated readings and nothing modeled across the gap: {points:?}"
    );
    assert!(points.iter().all(|p| !p.estimated));
}

#[test]
fn nothing_is_modeled_past_the_newest_reading() {
    let snapshots = vec![snapshot(Some(43), day(0)), snapshot(Some(45), day(5))];
    // Three games played after the last snapshot was taken.
    let matches: Vec<Match> = (6..=8).map(|i| ranked(i, true, day(i))).collect();

    let points = trajectory(&snapshots, &matches, &config());

    assert_eq!(points.len(), 2);
    assert!(
        points.iter().all(|p| p.at <= day(5)),
        "a free-running tail has no second anchor, so it is not drawn"
    );
}

#[test]
fn a_single_reading_is_a_point_and_never_a_line() {
    let snapshots = vec![snapshot(Some(43), day(0))];
    let matches: Vec<Match> = (1..=5).map(|i| ranked(i, true, day(i))).collect();

    let points = trajectory(&snapshots, &matches, &config());

    assert_eq!(points.len(), 1);
    assert!(!points[0].estimated);
}

#[test]
fn an_empty_history_charts_nothing() {
    assert!(trajectory(&[], &[], &config()).is_empty());
}

#[test]
fn a_shared_reading_between_segments_is_emitted_once() {
    let snapshots = vec![
        snapshot(Some(40), day(0)),
        snapshot(Some(42), day(5)),
        snapshot(Some(44), day(10)),
    ];
    let matches: Vec<Match> = (1..=9)
        .filter(|i| *i != 5)
        .map(|i| ranked(i, true, day(i)))
        .collect();

    let points = trajectory(&snapshots, &matches, &config());

    let at_day_five: Vec<&TrajectoryPoint> = points.iter().filter(|p| p.at == day(5)).collect();
    assert_eq!(
        at_day_five.len(),
        1,
        "the middle reading ends one segment and starts the next"
    );
    assert!(!at_day_five[0].estimated);
}

#[test]
fn modeled_points_stay_inside_the_band_their_readings_define() {
    // A brutal losing run that nonetheless ended one tier higher: the dip is
    // allowed to show, but not to run away.
    let snapshots = vec![snapshot(Some(50), day(0)), snapshot(Some(51), day(20))];
    let matches: Vec<Match> = (1..=19).map(|i| ranked(i, i > 15, day(i))).collect();

    let points = trajectory(&snapshots, &matches, &config());

    for point in &points {
        assert!(
            point.rank_tier >= 49 && point.rank_tier <= 52,
            "a modeled tier outside the readings' band plus one star is a fabrication: {point:?}"
        );
    }
}

#[test]
fn unranked_games_do_not_appear_on_the_ranked_trajectory() {
    let snapshots = vec![snapshot(Some(43), day(0)), snapshot(Some(45), day(7))];
    let mut matches = vec![ranked(1, true, day(1))];

    let mut turbo = ranked(2, true, day(2));
    turbo.game_mode = Some(game_mode::TURBO);
    matches.push(turbo);

    let points = trajectory(&snapshots, &matches, &config());

    assert_eq!(
        points.len(),
        3,
        "two readings and the one ranked game between them: {points:?}"
    );
}

// ---------------------------------------------------------------------------
// methodology
// ---------------------------------------------------------------------------

#[test]
fn the_methodology_block_reports_the_configuration_actually_in_force() {
    let mut config = config();
    config.win_base_mmr = 27.0;
    config.confidence_threshold_pct = 42.0;

    let m = methodology(&config);

    assert_eq!(m.win_base_mmr, 27.0);
    assert_eq!(m.loss_base_mmr, 25.0);
    assert_eq!(m.confidence_threshold_pct, 42.0);
    assert_eq!(m.confidence_per_match_pct, 1.5);
}

#[test]
fn the_modeled_path_follows_results_rather_than_the_calendar() {
    // A realistic window: one star gained over eleven days, ten ranked games,
    // the first five lost and the last five won. Time-linear interpolation
    // would climb steadily throughout; the model should sag while the player
    // was losing and only recover at the end.
    let snapshots = vec![snapshot(Some(40), day(0)), snapshot(Some(41), day(11))];
    let matches: Vec<Match> = (1..=10).map(|i| ranked(i, i > 5, day(i))).collect();

    let points = trajectory(&snapshots, &matches, &config());
    let modeled: Vec<i16> = points
        .iter()
        .filter(|p| p.estimated)
        .map(|p| p.rank_tier)
        .collect();

    assert_eq!(modeled.len(), 10);
    assert_eq!(
        modeled.first(),
        Some(&40),
        "the path starts where the first real reading did"
    );
    assert_eq!(
        modeled.last(),
        Some(&41),
        "and arrives at the second one: {modeled:?}"
    );

    // The trough of the losing run must sit at or below where the window
    // started — a straight ramp would already be climbing here.
    let trough = modeled[..5].iter().min().copied().unwrap();
    assert!(
        trough <= 40,
        "five straight losses should not read as progress: {modeled:?}"
    );
}

/// The failure this replaced: when a window's wins and losses nearly cancel,
/// a per-segment scale derived from the net movement explodes, and the whole
/// path collapses onto the clamp floor with a cliff at the last point.
#[test]
fn a_window_that_nets_out_does_not_collapse_into_a_cliff() {
    let snapshots = vec![snapshot(Some(40), day(0)), snapshot(Some(41), day(11))];
    let matches: Vec<Match> = (1..=10).map(|i| ranked(i, i > 5, day(i))).collect();

    let points = trajectory(&snapshots, &matches, &config());
    let modeled: Vec<i16> = points
        .iter()
        .filter(|p| p.estimated)
        .map(|p| p.rank_tier)
        .collect();

    let jumps: Vec<i16> = modeled.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    assert!(
        jumps.iter().all(|j| *j <= 1),
        "a modeled path moves a star at a time, never in one leap: {modeled:?}"
    );
}

#[test]
fn every_trajectory_point_names_its_medal() {
    let snapshots = vec![snapshot(Some(43), day(0)), snapshot(Some(45), day(7))];
    let matches: Vec<Match> = (1..=3).map(|i| ranked(i, true, day(i))).collect();

    let points = trajectory(&snapshots, &matches, &config());

    // Named server-side so no client ships a second copy of the medal table.
    assert_eq!(points.first().unwrap().label.as_deref(), Some("Archon 3"));
    assert_eq!(points.last().unwrap().label.as_deref(), Some("Archon 5"));
    assert!(
        points.iter().all(|p| p.label.is_some()),
        "a modeled point still sits at a real tier, so it still has a name"
    );
}

// ---------------------------------------------------------------------------
// momentum
// ---------------------------------------------------------------------------

#[test]
fn momentum_starts_at_zero_and_reports_relative_movement() {
    let matches: Vec<Match> = (1..=5).map(|i| ranked(i, true, day(i))).collect();

    let m = momentum(&matches, &config());

    assert_eq!(m.points.len(), 5);
    assert_eq!(m.points[0].index, 1, "1-based, oldest first");
    // Five wins at the 30 base: the curve is a climb from zero, and the first
    // point is one match's movement — never a starting rating.
    assert!(m.points[0].cumulative > 0.0 && m.points[0].cumulative < 40.0);
    assert!(m.net > m.points[0].cumulative);
    assert_eq!(m.wins, 5);
    assert_eq!(m.losses, 0);
}

#[test]
fn momentum_falls_across_a_losing_run() {
    let matches: Vec<Match> = (1..=4).map(|i| ranked(i, false, day(i))).collect();

    let m = momentum(&matches, &config());

    assert!(m.net < 0.0, "four losses is a fall, not a flat line");
    assert_eq!(m.losses, 4);
    // Monotonically down: each loss moves the running total further negative.
    for pair in m.points.windows(2) {
        assert!(pair[1].cumulative < pair[0].cumulative);
    }
}

#[test]
fn momentum_reads_the_most_recent_window_oldest_first() {
    // Thirty matches, so the window has to choose.
    let matches: Vec<Match> = (1..=30).map(|i| ranked(i, true, day(i))).collect();

    let m = momentum(&matches, &config());

    assert_eq!(m.points.len(), MOMENTUM_WINDOW as usize);
    assert_eq!(m.window, MOMENTUM_WINDOW);
    assert_eq!(
        m.points.first().unwrap().match_id,
        11,
        "the newest twenty, presented oldest first"
    );
    assert_eq!(m.points.last().unwrap().match_id, 30);
}

#[test]
fn momentum_ignores_games_that_cannot_move_a_medal() {
    let mut matches = vec![ranked(1, true, day(1))];

    let mut turbo = ranked(2, true, day(2));
    turbo.game_mode = Some(game_mode::TURBO);
    matches.push(turbo);

    let mut unranked = ranked(3, true, day(3));
    unranked.lobby_type = Some(lobby_type::NORMAL);
    matches.push(unranked);

    let m = momentum(&matches, &config());

    assert_eq!(m.points.len(), 1, "only the ranked game moves the curve");
}

#[test]
fn momentum_is_empty_rather_than_a_flat_line_without_matches() {
    let m = momentum(&[], &config());

    assert!(m.points.is_empty());
    assert_eq!(m.net, 0.0);
    assert_eq!(m.wins, 0);
    assert_eq!(m.losses, 0);
}

/// The honesty property, as a test rather than a comment: the curve reports
/// movement from zero. If someone ever seeds it with an absolute rating, the
/// first point stops being one match's worth of movement.
#[test]
fn momentum_never_reports_an_absolute_rating() {
    let matches: Vec<Match> = (1..=20).map(|i| ranked(i, i % 2 == 0, day(i))).collect();

    let m = momentum(&matches, &config());

    let first = &m.points[0];
    assert_eq!(
        first.cumulative, first.delta,
        "the curve starts at zero, so the first point is exactly its own delta"
    );
    assert!(
        m.points.iter().all(|p| p.cumulative.abs() < 2_000.0),
        "these are movements in the hundreds, not a rating in the thousands"
    );
}
