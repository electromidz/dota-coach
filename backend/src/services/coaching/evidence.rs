//! Composing the measured half of a coaching analysis.
//!
//! Every sentence here is built from numbers this backend already computed —
//! `services::metrics`, the benchmark engine, the hero pool — and none of them
//! is ever recomputed from raw match rows. The model receives these sentences
//! and may cite them by id; it never receives the underlying payloads.
//!
//! Pure by design: no database, no provider, no clock beyond what the caller
//! passes in, so the exact text a model will be shown is testable.

use crate::domain::benchmark::BenchmarkResult;
use crate::domain::coaching::{Evidence, EvidenceKind};
use crate::domain::hero::{HeroFit, HeroPoolEntry};
use crate::domain::metrics::{MatchMetrics, PlayerStats};
use crate::domain::player_model::{PatternStatus, RecurringPattern};
use crate::domain::r#match::Match;
use crate::domain::training::TrainingFocus;
use crate::services::benchmarks::percentile;

/// How many heroes and recommendations reach the prompt.
///
/// The cap is about signal, not tokens: a coach who names twelve heroes has
/// not made a point. The pool is already ordered most-played first.
const MAX_POOL_EVIDENCE: usize = 3;
const MAX_FIT_EVIDENCE: usize = 2;
const MAX_PATTERN_EVIDENCE: usize = 4;
/// Matches counted as "recent form" in the player-wide evidence.
pub const FORM_WINDOW: usize = 10;

/// Everything the builder needs, already fetched and already computed.
pub struct EvidenceInputs<'a> {
    pub stats: &'a PlayerStats,
    /// Newest first.
    pub recent: &'a [Match],
    /// Results for the benchmarked hero, from the benchmark engine.
    pub benchmarks: &'a [BenchmarkResult],
    /// The hero those results describe.
    pub benchmark_hero: Option<&'a str>,
    pub pool: &'a [HeroPoolEntry],
    pub recommendations: &'a [HeroFit],
    /// Recurring patterns, as detected across the whole history.
    pub patterns: &'a [RecurringPattern],
    /// The player's current training focus, when one is set.
    pub focus: Option<&'a TrainingFocus>,
    /// Present for a match-scoped analysis.
    pub focus_match: Option<&'a Match>,
    pub focus_metrics: Option<&'a MatchMetrics>,
}

/// Build the evidence list for one analysis.
///
/// Order is deliberate and stable: career, then form, then peer comparison,
/// then repertoire, then the match under discussion. Two identical inputs
/// always produce an identical list, which is what lets a stored analysis be
/// keyed by a hash of it.
pub fn build(inputs: &EvidenceInputs<'_>) -> Vec<Evidence> {
    // A player with no stored matches has no evidence, full stop. Hero
    // recommendations still exist for them — they are built from the meta
    // alone — but "here is what the ladder is playing" is not evidence about
    // *this player*, and coaching on it would be advice with no subject.
    if inputs.stats.matches == 0 && inputs.focus_match.is_none() {
        return Vec::new();
    }

    let mut evidence = Vec::new();

    overall(inputs.stats, &mut evidence);
    form(inputs.recent, &mut evidence);
    benchmarks(inputs.benchmarks, inputs.benchmark_hero, &mut evidence);
    heroes(inputs.pool, inputs.recommendations, &mut evidence);
    recurring(inputs.patterns, &mut evidence);
    training_focus(inputs.focus, &mut evidence);

    if let Some(match_) = inputs.focus_match {
        focus_match(match_, inputs.focus_metrics, inputs.stats, &mut evidence);
    }

    evidence
}

fn overall(stats: &PlayerStats, out: &mut Vec<Evidence>) {
    if stats.matches == 0 {
        return;
    }

    let sample = stats.matches;
    push(
        out,
        "overall.record",
        EvidenceKind::Overall,
        "Record",
        format!(
            "Across {} stored {}, you have won {} and lost {} ({}).",
            sample,
            plural(sample, "match", "matches"),
            stats.wins,
            stats.losses,
            percent(stats.win_rate),
        ),
        sample,
    );

    if let Some(kda) = stats.avg_kda {
        push(
            out,
            "overall.kda",
            EvidenceKind::Overall,
            "Average KDA",
            format!("Your average KDA is {kda:.2}."),
            sample,
        );
    }

    if let (Some(gpm), Some(xpm)) = (stats.avg_gpm, stats.avg_xpm) {
        push(
            out,
            "overall.economy",
            EvidenceKind::Overall,
            "Average economy",
            format!("You average {gpm:.0} gold and {xpm:.0} XP per minute."),
            sample,
        );
    }

    if let Some(deaths) = stats.avg_deaths_per_10 {
        push(
            out,
            "overall.deaths",
            EvidenceKind::Overall,
            "Deaths per 10 minutes",
            format!("You die {deaths:.1} times per 10 minutes of game time."),
            sample,
        );
    }

    // Kill participation is averaged only over matches that carry team totals,
    // so it reports its own, smaller sample rather than the career one.
    if let Some(kp) = stats.avg_kill_participation {
        push(
            out,
            "overall.kill_participation",
            EvidenceKind::Overall,
            "Kill participation",
            format!(
                "You take part in {} of your team's kills, measured across {} {}.",
                percent(Some(kp)),
                stats.kill_participation_sample,
                plural(stats.kill_participation_sample, "match", "matches"),
            ),
            stats.kill_participation_sample,
        );
    }
}

fn form(recent: &[Match], out: &mut Vec<Evidence>) {
    let window: Vec<&Match> = recent.iter().take(FORM_WINDOW).collect();
    if window.is_empty() {
        return;
    }

    let wins = window.iter().filter(|m| m.won).count();
    let sample = window.len() as i64;

    push(
        out,
        "form.recent",
        EvidenceKind::Form,
        "Recent form",
        format!(
            "You have won {} of your last {} {}.",
            wins,
            sample,
            plural(sample, "match", "matches"),
        ),
        sample,
    );

    // The streak is a separate reading from the ratio: 5-5 alternating and
    // 5-5 ending in five losses are the same win rate and different situations.
    let streak = window.iter().take_while(|m| m.won == window[0].won).count() as i64;
    if streak >= 3 {
        push(
            out,
            "form.streak",
            EvidenceKind::Form,
            "Current streak",
            format!(
                "Your last {streak} matches were all {}.",
                if window[0].won { "wins" } else { "losses" }
            ),
            streak,
        );
    }
}

fn benchmarks(results: &[BenchmarkResult], hero: Option<&str>, out: &mut Vec<Evidence>) {
    let Some(hero) = hero else {
        return;
    };

    for result in results {
        let statement = match (result.percentile, result.peer_median, result.top_20_value) {
            (Some(p), Some(median), Some(top)) => format!(
                "On {hero}, your {} averages {:.1}; the peer median is {median:.1} and the top 20% start at {top:.1}. That places you at the {p:.0}th percentile.",
                result.label.to_lowercase(),
                result.player_value,
            ),
            // Withheld percentiles keep the player's own value and say why —
            // the same honesty rule the benchmark endpoint enforces.
            (None, Some(median), _) => format!(
                "On {hero}, your {} averages {:.1} against a peer median of {median:.1}. No percentile is claimed: {}",
                result.label.to_lowercase(),
                result.player_value,
                result
                    .note
                    .clone()
                    .unwrap_or_else(|| "the sample is too small.".into()),
            ),
            _ => format!(
                "On {hero}, your {} averages {:.1}. No peer distribution is available for it.",
                result.label.to_lowercase(),
                result.player_value,
            ),
        };

        out.push(Evidence {
            id: format!("benchmark.{}", result.metric.slug()),
            kind: EvidenceKind::Benchmark,
            label: format!("{} vs peers", result.label),
            statement,
            sample: result.player_sample,
            confidence: result.confidence,
        });
    }
}

fn heroes(pool: &[HeroPoolEntry], recommendations: &[HeroFit], out: &mut Vec<Evidence>) {
    for entry in pool.iter().take(MAX_POOL_EVIDENCE) {
        let recent = match entry.recent_win_rate {
            Some(rate) => format!(
                " Over the last {} on it you have won {}.",
                entry.recent_matches,
                percent(Some(rate))
            ),
            None => String::new(),
        };

        push(
            out,
            &format!("hero.{}", entry.hero_id),
            EvidenceKind::Hero,
            &format!("{} record", entry.hero_name),
            format!(
                "{}: {} {}, {}-{} ({}), classified {} in your pool.{recent}",
                entry.hero_name,
                entry.matches,
                plural(entry.matches, "match", "matches"),
                entry.wins,
                entry.losses,
                percent(Some(entry.win_rate)),
                entry.tier_label.to_lowercase(),
            ),
            entry.matches,
        );
    }

    for fit in recommendations.iter().take(MAX_FIT_EVIDENCE) {
        let meta = match fit.meta_strength {
            Some(strength) => format!(" Its current meta strength is {strength:.0}/100."),
            None => String::new(),
        };

        push(
            out,
            &format!("fit.{}", fit.hero_id),
            EvidenceKind::Hero,
            &format!("{} fit score", fit.hero_name),
            format!(
                "{} scores {:.0}/100 for fit and is rated \"{}\", on {} {} of your history.{meta}",
                fit.hero_name,
                fit.fit_score,
                fit.level_label.to_lowercase(),
                fit.matches,
                plural(fit.matches, "match", "matches"),
            ),
            fit.matches,
        );
    }
}

/// Patterns as evidence.
///
/// This is what lets an insight be a *recurring pattern* rather than a reading
/// of an average: the statement already carries both denominators, so the
/// model can point at "12 of the 20 matches this could be measured in" instead
/// of extrapolating a habit from a mean.
///
/// Resolved patterns are excluded. They are worth showing a player — you fixed
/// this — but as evidence for current advice they would be actively wrong.
fn recurring(patterns: &[RecurringPattern], out: &mut Vec<Evidence>) {
    for pattern in patterns
        .iter()
        .filter(|p| p.status != PatternStatus::Resolved)
        .take(MAX_PATTERN_EVIDENCE)
    {
        push(
            out,
            &format!("pattern.{}", pattern.id),
            EvidenceKind::Pattern,
            &pattern.label,
            pattern.statement.clone(),
            // The sample is what the pattern could be checked in, not the
            // career total: that is the number the claim actually rests on.
            pattern.measured,
        );
    }
}

/// The current training focus as evidence.
///
/// Without this the coach would cheerfully write advice pointing somewhere
/// else — which is precisely the "overwhelm the user with ten weaknesses"
/// failure the focus exists to prevent.
fn training_focus(focus: Option<&TrainingFocus>, out: &mut Vec<Evidence>) {
    let Some(focus) = focus else {
        return;
    };

    let measure = focus.measure;
    let current = match focus.current_value {
        Some(value) => format!(
            " Over your last matches it stands at {}.",
            measure.format(value)
        ),
        None => String::new(),
    };

    push(
        out,
        "focus.current",
        EvidenceKind::Focus,
        "Current training focus",
        format!(
            "{}: {} was {} when this focus was set, and the target is {}.{current}",
            focus.title,
            measure.label().to_lowercase(),
            measure.format(focus.baseline_value),
            measure.format(focus.target_value),
        ),
        focus.sample,
    );
}

/// Evidence about one match, compared against the player's own averages.
///
/// The comparison is what makes a single match coachable: 6 deaths is not
/// information, "6 deaths against your average of 3.2 per 10 minutes" is.
fn focus_match(
    match_: &Match,
    metrics: Option<&MatchMetrics>,
    stats: &PlayerStats,
    out: &mut Vec<Evidence>,
) {
    let minutes = (match_.duration_seconds as f32 / 60.0).max(1.0);

    push(
        out,
        "match.result",
        EvidenceKind::Match,
        "Result",
        format!(
            "This match was a {} on {} ({}), lasting {:.0} minutes.",
            if match_.won { "win" } else { "loss" },
            match_.hero_name,
            match_.role,
            minutes,
        ),
        1,
    );

    push(
        out,
        "match.kda",
        EvidenceKind::Match,
        "Kills, deaths, assists",
        match (metrics.map(|m| m.kda), stats.avg_kda) {
            (Some(kda), Some(average)) => format!(
                "You finished {}/{}/{} for a KDA of {kda:.2}, against your average of {average:.2}.",
                match_.kills, match_.deaths, match_.assists,
            ),
            (Some(kda), None) => format!(
                "You finished {}/{}/{} for a KDA of {kda:.2}.",
                match_.kills, match_.deaths, match_.assists,
            ),
            _ => format!(
                "You finished {}/{}/{}.",
                match_.kills, match_.deaths, match_.assists
            ),
        },
        1,
    );

    if let Some(deaths_per_10) = metrics.map(|m| m.deaths_per_10) {
        push(
            out,
            "match.deaths",
            EvidenceKind::Match,
            "Death rate",
            match stats.avg_deaths_per_10 {
                Some(average) => format!(
                    "That is {deaths_per_10:.1} deaths per 10 minutes, against your average of {average:.1}.",
                ),
                None => format!("That is {deaths_per_10:.1} deaths per 10 minutes."),
            },
            1,
        );
    }

    push(
        out,
        "match.economy",
        EvidenceKind::Match,
        "Economy",
        match (stats.avg_gpm, stats.avg_xpm) {
            (Some(gpm), Some(xpm)) => format!(
                "You earned {} gold and {} XP per minute, against your averages of {gpm:.0} and {xpm:.0}.",
                match_.gpm, match_.xpm,
            ),
            _ => format!(
                "You earned {} gold and {} XP per minute.",
                match_.gpm, match_.xpm
            ),
        },
        1,
    );

    if let Some(kp) = metrics.and_then(|m| m.kill_participation) {
        push(
            out,
            "match.kill_participation",
            EvidenceKind::Match,
            "Kill participation",
            match stats.avg_kill_participation {
                Some(average) => format!(
                    "You were involved in {} of your team's kills, against your average of {}.",
                    percent(Some(kp)),
                    percent(Some(average)),
                ),
                None => format!(
                    "You were involved in {} of your team's kills.",
                    percent(Some(kp))
                ),
            },
            1,
        );
    }

    // Everything below needs a parsed replay. Absent is absent: no default,
    // and no sentence claiming a laning phase nobody measured.
    if let Some(last_hits) = match_.last_hits_at_10 {
        push(
            out,
            "match.last_hits_at_10",
            EvidenceKind::Match,
            "Last hits at 10 minutes",
            format!(
                "At 10 minutes you had {last_hits} last hits, finishing the game with {}.",
                match_.last_hits
            ),
            1,
        );
    }

    if let Some(gold) = match_.gold_at_10 {
        push(
            out,
            "match.gold_at_10",
            EvidenceKind::Match,
            "Gold at 10 minutes",
            format!("At 10 minutes you had {gold} net worth."),
            1,
        );
    }

    if let Some(participation) = match_.teamfight_participation {
        push(
            out,
            "match.teamfight_participation",
            EvidenceKind::Match,
            "Teamfight participation",
            format!(
                "The provider measured your teamfight participation at {}.",
                percent(Some(participation))
            ),
            1,
        );
    }

    for (id, label, seconds) in [
        (
            "match.timing_bkb",
            "Black King Bar timing",
            match_.bkb_seconds,
        ),
        (
            "match.timing_blink",
            "Blink Dagger timing",
            match_.blink_seconds,
        ),
        (
            "match.timing_midas",
            "Hand of Midas timing",
            match_.midas_seconds,
        ),
    ] {
        if let Some(seconds) = seconds {
            push(
                out,
                id,
                EvidenceKind::Match,
                label,
                format!(
                    "You completed your {} at {}:{:02}.",
                    label.trim_end_matches(" timing"),
                    seconds / 60,
                    seconds % 60,
                ),
                1,
            );
        }
    }
}

fn push(
    out: &mut Vec<Evidence>,
    id: &str,
    kind: EvidenceKind,
    label: &str,
    statement: String,
    sample: i64,
) {
    out.push(Evidence {
        id: id.to_string(),
        kind,
        label: label.to_string(),
        statement,
        sample,
        // Uniform rule: confidence describes how far a figure generalizes, not
        // whether it is accurate. One match is exact and generalizes to
        // nothing, which is exactly what `Insufficient` should mean here.
        confidence: percentile::confidence_for(sample),
    });
}

fn percent(value: Option<f32>) -> String {
    match value {
        Some(v) => format!("{:.0}%", v * 100.0),
        None => "an unknown share".to_string(),
    }
}

fn plural(count: i64, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 {
        one
    } else {
        many
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::benchmark::{BenchmarkMetric, Confidence, Segment};
    use crate::domain::hero::{HeroTier, RecommendationLevel};
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn stats() -> PlayerStats {
        PlayerStats {
            matches: 20,
            wins: 11,
            losses: 9,
            win_rate: Some(0.55),
            avg_kda: Some(3.2),
            avg_gpm: Some(520.0),
            avg_xpm: Some(600.0),
            avg_last_hits: Some(240.0),
            avg_deaths_per_10: Some(1.4),
            avg_kills_per_10: Some(2.0),
            avg_hero_damage: Some(24_000.0),
            avg_kill_participation: Some(0.62),
            kill_participation_sample: 18,
            parsed_matches: 4,
        }
    }

    fn match_(won: bool, id: i64) -> Match {
        Match {
            id: Uuid::nil(),
            dota_player_id: Uuid::nil(),
            match_id: id,
            hero_id: 35,
            hero_name: "Luna".into(),
            role: "Carry".into(),
            lane_role: Some(1),
            won,
            duration_seconds: 2_400,
            kills: 8,
            deaths: 4,
            assists: 12,
            gpm: 550,
            xpm: 620,
            last_hits: 300,
            denies: None,
            net_worth: None,
            hero_damage: Some(25_000),
            tower_damage: Some(3_000),
            hero_healing: Some(0),
            game_mode: Some(22),
            lobby_type: Some(7),
            party_size: Some(1),
            started_at: Utc.timestamp_opt(1_700_000_000 + id, 0).unwrap(),
            detail_synced: true,
            team_kills: Some(30),
            team_deaths: Some(20),
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
            metrics_kda: Some(5.0),
        }
    }

    fn metrics() -> MatchMetrics {
        MatchMetrics {
            match_id: Uuid::nil(),
            metrics_version: 1,
            kda: 5.0,
            kills_per_10: 2.0,
            deaths_per_10: 1.0,
            assists_per_10: 3.0,
            last_hits_per_min: 7.5,
            hero_damage_per_min: Some(625.0),
            tower_damage_per_min: Some(75.0),
            kill_participation: Some(0.67),
            gold_advantage_at_10: None,
        }
    }

    fn benchmark(percentile: Option<f32>) -> BenchmarkResult {
        BenchmarkResult {
            metric: BenchmarkMetric::GoldPerMin,
            label: "Gold per minute",
            higher_is_better: true,
            player_value: 512.0,
            player_sample: if percentile.is_some() { 20 } else { 2 },
            peer_median: Some(500.0),
            top_20_value: Some(684.0),
            percentile,
            gap_to_top_20: Some(172.0),
            peer_sample_size: None,
            confidence: if percentile.is_some() {
                Confidence::Adequate
            } else {
                Confidence::Insufficient
            },
            segmented_by: vec![Segment::Hero],
            note: percentile
                .is_none()
                .then(|| "you need 5 matches, you have 2.".to_string()),
        }
    }

    fn pool_entry() -> HeroPoolEntry {
        HeroPoolEntry {
            hero_id: 35,
            hero_name: "Luna".into(),
            role: "Carry".into(),
            matches: 20,
            wins: 11,
            losses: 9,
            win_rate: 0.55,
            recent_matches: 10,
            recent_win_rate: Some(0.6),
            avg_kda: 3.4,
            avg_gpm: 540.0,
            last_played_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
            tier: HeroTier::Signature,
            tier_label: "Signature",
            confidence: Confidence::Adequate,
        }
    }

    fn fit() -> HeroFit {
        HeroFit {
            hero_id: 35,
            hero_name: "Luna".into(),
            fit_score: 78.0,
            level: RecommendationLevel::Recommended,
            level_label: "Recommended",
            parts: Vec::new(),
            reasons: Vec::new(),
            caveats: Vec::new(),
            matches: 20,
            tier: Some(HeroTier::Signature),
            meta_strength: Some(72.0),
            focus_adjustment: 0.0,
        }
    }

    fn inputs<'a>(
        stats: &'a PlayerStats,
        recent: &'a [Match],
        benchmarks: &'a [BenchmarkResult],
        pool: &'a [HeroPoolEntry],
        recommendations: &'a [HeroFit],
    ) -> EvidenceInputs<'a> {
        EvidenceInputs {
            stats,
            recent,
            benchmarks,
            benchmark_hero: Some("Luna"),
            pool,
            recommendations,
            patterns: &[],
            focus: None,
            focus_match: None,
            focus_metrics: None,
        }
    }

    #[test]
    fn a_full_history_produces_career_form_benchmark_and_hero_evidence() {
        let stats = stats();
        let recent: Vec<Match> = (0..10).map(|i| match_(i % 2 == 0, i)).collect();
        let benchmarks = [benchmark(Some(46.0))];
        let pool = [pool_entry()];
        let fits = [fit()];

        let evidence = build(&inputs(&stats, &recent, &benchmarks, &pool, &fits));
        let ids: Vec<&str> = evidence.iter().map(|e| e.id.as_str()).collect();

        assert!(ids.contains(&"overall.record"));
        assert!(ids.contains(&"form.recent"));
        assert!(ids.contains(&"benchmark.gold_per_min"));
        assert!(ids.contains(&"hero.35"));
        assert!(ids.contains(&"fit.35"));
    }

    #[test]
    fn every_statement_carries_its_own_numbers() {
        let stats = stats();
        let recent: Vec<Match> = (0..10).map(|i| match_(i % 2 == 0, i)).collect();
        let evidence = build(&inputs(&stats, &recent, &[benchmark(Some(46.0))], &[], &[]));

        let record = evidence.iter().find(|e| e.id == "overall.record").unwrap();
        assert!(record.statement.contains("20 stored matches"));
        assert!(record.statement.contains("55%"));

        let gpm = evidence
            .iter()
            .find(|e| e.id == "benchmark.gold_per_min")
            .unwrap();
        assert!(gpm.statement.contains("46th percentile"));
        assert!(gpm.statement.contains("684"), "the top-20% line is stated");
    }

    #[test]
    fn a_withheld_percentile_is_explained_rather_than_omitted() {
        let stats = stats();
        let evidence = build(&inputs(&stats, &[], &[benchmark(None)], &[], &[]));
        let gpm = evidence
            .iter()
            .find(|e| e.id == "benchmark.gold_per_min")
            .unwrap();

        assert!(gpm.statement.contains("No percentile is claimed"));
        assert!(gpm.statement.contains("you have 2"));
        assert_eq!(gpm.confidence, Confidence::Insufficient);
    }

    #[test]
    fn ids_are_unique_so_a_citation_is_unambiguous() {
        let stats = stats();
        let recent: Vec<Match> = (0..10).map(|i| match_(i % 2 == 0, i)).collect();
        let pool = [pool_entry()];
        let fits = [fit()];
        let mut evidence = build(&inputs(
            &stats,
            &recent,
            &[benchmark(Some(46.0))],
            &pool,
            &fits,
        ));

        let before = evidence.len();
        evidence.sort_by(|a, b| a.id.cmp(&b.id));
        evidence.dedup_by(|a, b| a.id == b.id);
        assert_eq!(evidence.len(), before);
    }

    #[test]
    fn an_empty_history_produces_no_evidence_rather_than_zeroes() {
        let empty = PlayerStats {
            matches: 0,
            wins: 0,
            losses: 0,
            win_rate: None,
            avg_kda: None,
            avg_gpm: None,
            avg_xpm: None,
            avg_last_hits: None,
            avg_deaths_per_10: None,
            avg_kills_per_10: None,
            avg_hero_damage: None,
            avg_kill_participation: None,
            kill_participation_sample: 0,
            parsed_matches: 0,
        };

        assert!(build(&inputs(&empty, &[], &[], &[], &[])).is_empty());

        // Not even a hero recommendation counts: it describes the meta, not
        // the player, and there is no player here to coach.
        let fits = [fit()];
        let pool = [pool_entry()];
        assert!(build(&inputs(&empty, &[], &[], &pool, &fits)).is_empty());
    }

    #[test]
    fn a_losing_streak_is_its_own_evidence() {
        let stats = stats();
        let recent: Vec<Match> = (0..10).map(|i| match_(i >= 4, i)).collect();
        let evidence = build(&inputs(&stats, &recent, &[], &[], &[]));

        let streak = evidence.iter().find(|e| e.id == "form.streak").unwrap();
        assert!(streak.statement.contains("4 matches were all losses"));
    }

    #[test]
    fn an_alternating_record_has_no_streak_evidence() {
        let stats = stats();
        let recent: Vec<Match> = (0..10).map(|i| match_(i % 2 == 0, i)).collect();
        let evidence = build(&inputs(&stats, &recent, &[], &[], &[]));

        assert!(!evidence.iter().any(|e| e.id == "form.streak"));
    }

    #[test]
    fn match_evidence_compares_the_game_against_the_players_own_averages() {
        let stats = stats();
        let focus = match_(false, 1);
        let metrics = metrics();

        let mut input = inputs(&stats, &[], &[], &[], &[]);
        input.focus_match = Some(&focus);
        input.focus_metrics = Some(&metrics);

        let evidence = build(&input);
        let kda = evidence.iter().find(|e| e.id == "match.kda").unwrap();

        assert!(kda.statement.contains("8/4/12"));
        assert!(kda.statement.contains("your average of 3.20"));
        assert_eq!(kda.sample, 1);
        assert_eq!(
            kda.confidence,
            Confidence::Insufficient,
            "one match generalizes to nothing, however exact it is"
        );
    }

    #[test]
    fn an_unparsed_match_claims_no_laning_phase() {
        let stats = stats();
        let focus = match_(false, 1);
        let metrics = metrics();

        let mut input = inputs(&stats, &[], &[], &[], &[]);
        input.focus_match = Some(&focus);
        input.focus_metrics = Some(&metrics);

        let evidence = build(&input);
        assert!(!evidence.iter().any(|e| e.id == "match.last_hits_at_10"));
        assert!(!evidence.iter().any(|e| e.id == "match.gold_at_10"));
    }

    #[test]
    fn a_parsed_match_adds_the_laning_phase_and_item_timings() {
        let stats = stats();
        let mut focus = match_(true, 1);
        focus.replay_parsed = true;
        focus.last_hits_at_10 = Some(43);
        focus.gold_at_10 = Some(3_100);
        focus.blink_seconds = Some(1_085);
        let metrics = metrics();

        let mut input = inputs(&stats, &[], &[], &[], &[]);
        input.focus_match = Some(&focus);
        input.focus_metrics = Some(&metrics);

        let evidence = build(&input);
        let lh = evidence
            .iter()
            .find(|e| e.id == "match.last_hits_at_10")
            .unwrap();
        assert!(lh.statement.contains("43 last hits"));

        let blink = evidence
            .iter()
            .find(|e| e.id == "match.timing_blink")
            .unwrap();
        assert!(blink.statement.contains("18:05"), "{}", blink.statement);
    }

    #[test]
    fn the_hero_pool_is_capped_so_the_prompt_stays_pointed() {
        let stats = stats();
        let pool: Vec<HeroPoolEntry> = (0..8)
            .map(|i| HeroPoolEntry {
                hero_id: i,
                ..pool_entry()
            })
            .collect();

        let evidence = build(&inputs(&stats, &[], &[], &pool, &[]));
        let heroes = evidence
            .iter()
            .filter(|e| e.kind == EvidenceKind::Hero)
            .count();

        assert_eq!(heroes, MAX_POOL_EVIDENCE);
    }

    #[test]
    fn the_same_inputs_always_produce_the_same_evidence() {
        let stats = stats();
        let recent: Vec<Match> = (0..10).map(|i| match_(i % 2 == 0, i)).collect();
        let pool = [pool_entry()];

        let first = build(&inputs(
            &stats,
            &recent,
            &[benchmark(Some(46.0))],
            &pool,
            &[],
        ));
        let second = build(&inputs(
            &stats,
            &recent,
            &[benchmark(Some(46.0))],
            &pool,
            &[],
        ));

        let ids = |e: &[Evidence]| e.iter().map(|x| x.id.clone()).collect::<Vec<_>>();
        let text = |e: &[Evidence]| e.iter().map(|x| x.statement.clone()).collect::<Vec<_>>();

        assert_eq!(ids(&first), ids(&second));
        assert_eq!(text(&first), text(&second));
    }
}
