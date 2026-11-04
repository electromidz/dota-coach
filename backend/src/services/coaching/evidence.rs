//! Composing the measured half of a coaching analysis.
//!
//! Every sentence here is built from numbers this backend already computed —
//! `services::metrics`, the benchmark engine, the hero pool — and none of them
//! is ever recomputed from raw match rows. The model receives these sentences
//! and may cite them by id; it never receives the underlying payloads.
//!
//! Pure by design: no database, no provider, no clock beyond what the caller
//! passes in, so the exact text a model will be shown is testable.

use crate::domain::benchmark::{BenchmarkResult, Confidence};
use crate::domain::coaching::{Evidence, EvidenceFigure, EvidenceKind};
use crate::domain::hero::{HeroFit, HeroKit, HeroPoolEntry};
use crate::domain::metrics::{MatchMetrics, PlayerStats};
use crate::domain::player_model::{PatternStatus, RecurringPattern};
use crate::domain::progress::{MetricProgress, ProgressStatus, SessionProgress};
use crate::domain::r#match::Match;
use crate::domain::role::CoachableRole;
use crate::domain::scope::SampleConfidence;
use crate::domain::training::TrainingFocus;
use crate::services::benchmarks::percentile;
use crate::services::format::ordinal;

/// How many heroes and recommendations reach the prompt.
///
/// The cap is about signal, not tokens: a coach who names twelve heroes has
/// not made a point. The pool is already ordered most-played first.
const MAX_POOL_EVIDENCE: usize = 3;
const MAX_FIT_EVIDENCE: usize = 2;
const MAX_PATTERN_EVIDENCE: usize = 4;
/// Movements, resolutions and new issues each reach the prompt at most this
/// many times. Same reasoning as the pool cap: a coach naming nine changes has
/// not said which one matters.
const MAX_PROGRESS_EVIDENCE: usize = 3;
/// Matches counted as "recent form" in the player-wide evidence.
pub const FORM_WINDOW: usize = 10;

/// What population the evidence describes.
///
/// This is not decoration. Every statement the builder composes is a claim
/// about a specific set of matches, and the model is shown the claim without
/// the set — so the set has to be *stated*, as the first piece of evidence,
/// in the same voice as the rest. Without it "you die 4.1 times per 10
/// minutes" is a sentence about nothing in particular.
///
/// The scoping itself is enforced upstream, by the queries that produced these
/// figures. This only says what happened.
#[derive(Debug, Clone, Copy)]
pub struct EvidenceScope {
    /// The role every figure below is about, when there is one.
    pub role: Option<CoachableRole>,
    /// Eligible matches the scope resolved to.
    pub matches: i64,
    pub confidence: SampleConfidence,
}

/// Everything the builder needs, already fetched and already computed.
/// One metric of the match being analysed, placed among peers on the hero.
/// A single game is exact, so it always gets a percentile; the sample floor
/// is about estimating a player's average, which this is not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchPeerReading {
    pub metric: crate::domain::benchmark::BenchmarkMetric,
    pub value: f32,
    pub percentile: f32,
    pub median: Option<f32>,
    pub top_20: Option<f32>,
}

pub struct EvidenceInputs<'a> {
    /// The population everything else describes.
    pub scope: EvidenceScope,
    pub stats: &'a PlayerStats,
    /// Newest first.
    pub recent: &'a [Match],
    /// Results for the benchmarked hero, from the benchmark engine.
    pub benchmarks: &'a [BenchmarkResult],
    /// The hero those results describe.
    pub benchmark_hero: Option<&'a str>,
    /// Its id, carried on the benchmark figures so a chart can find the hero.
    pub benchmark_hero_id: Option<i32>,
    /// Abilities and usual items for the heroes the advice is about.
    pub kits: &'a [HeroKit],
    /// The analysed match's own percentiles among peers, for match analysis.
    pub focus_peers: &'a [MatchPeerReading],
    pub pool: &'a [HeroPoolEntry],
    pub recommendations: &'a [HeroFit],
    /// Recurring patterns, as detected across the whole history.
    pub patterns: &'a [RecurringPattern],
    /// What changed since the previous coaching session.
    ///
    /// `None` has two meanings the builder tells apart: no session recorded
    /// yet, and one session with nothing before it. Both produce a statement
    /// saying so, because a model shown no progress evidence and no
    /// explanation may reach for "you have improved" anyway.
    pub progress: Option<&'a SessionProgress>,
    /// True when at least one session exists for this role. Distinguishes
    /// "nothing recorded yet" from "first session, nothing to compare".
    pub has_session: bool,
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

    scope(inputs.scope, &mut evidence);
    overall(inputs.stats, &mut evidence);
    form(inputs.recent, &mut evidence);
    benchmarks(
        inputs.benchmarks,
        inputs.benchmark_hero,
        inputs.benchmark_hero_id,
        &mut evidence,
    );
    hero_kits(inputs.kits, inputs.scope.role, &mut evidence);
    heroes(inputs.pool, inputs.recommendations, &mut evidence);
    hero_tensions(
        inputs.pool,
        inputs.recommendations,
        inputs.benchmarks,
        inputs.benchmark_hero,
        &mut evidence,
    );
    recurring(inputs.patterns, &mut evidence);
    // After the patterns it refers to, before the focus that acts on it.
    progress(inputs.progress, inputs.has_session, &mut evidence);
    training_focus(inputs.focus, &mut evidence);

    if let Some(match_) = inputs.focus_match {
        focus_match(match_, inputs.focus_metrics, inputs.stats, &mut evidence);
        match_peers(
            match_,
            inputs.focus_peers,
            inputs.benchmark_hero_id,
            &mut evidence,
        );
    }

    evidence
}

/// The population, stated first and in the same form as everything else.
///
/// It leads deliberately: the model reads the list in order, and every figure
/// after this one is qualified by it.
fn scope(scope: EvidenceScope, out: &mut Vec<Evidence>) {
    let statement = match scope.role {
        Some(role) => format!(
            "Everything below is measured over your last {} eligible {} as {}. \
             Only Ranked and public All Pick games count; Turbo, Ability Draft, \
             custom and event games are excluded, and so is every match you \
             played in another role.",
            scope.matches,
            plural(scope.matches, "match", "matches"),
            role.label(),
        ),
        None => format!(
            "Everything below is measured over your last {} eligible {}. Only \
             Ranked and public All Pick games count; Turbo, Ability Draft, \
             custom and event games are excluded.",
            scope.matches,
            plural(scope.matches, "match", "matches"),
        ),
    };

    out.push(Evidence {
        id: "scope.population".to_string(),
        kind: EvidenceKind::Overall,
        label: "What was analysed".to_string(),
        statement,
        sample: scope.matches,
        // The sample-size reading for the window as a whole, which is a
        // different question from whether a percentile may be claimed.
        confidence: match scope.confidence {
            SampleConfidence::Limited => Confidence::Insufficient,
            SampleConfidence::Moderate => Confidence::Low,
            SampleConfidence::Strong => Confidence::Adequate,
        },
        figure: None,
    });
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

fn benchmarks(
    results: &[BenchmarkResult],
    hero: Option<&str>,
    hero_id: Option<i32>,
    out: &mut Vec<Evidence>,
) {
    let Some(hero) = hero else {
        return;
    };

    for result in results {
        let statement = match (result.percentile, result.peer_median, result.top_20_value) {
            (Some(p), Some(median), Some(top)) => format!(
                "On {hero}, your {} averages {:.1}; the peer median is {median:.1} and the top 20% start at {top:.1}. That places you at the {} percentile.",
                result.label.to_lowercase(),
                result.player_value,
                ordinal(p),
            ),
            // Withheld percentiles keep the player's own value and say why —
            // the same honesty rule the benchmark endpoint enforces.
            (None, Some(median), _) => format!(
                "On {hero}, your {} averages {:.1} against a peer median of {median:.1}. {}",
                result.label.to_lowercase(),
                result.player_value,
                result
                    .note
                    .clone()
                    .unwrap_or_else(|| "Your average: needs more games for a percentile.".into()),
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
            figure: Some(EvidenceFigure {
                metric: Some(result.metric.slug().to_string()),
                hero_id,
                value: result.player_value,
                higher_is_better: result.higher_is_better,
                unit: None,
                median: result.peer_median,
                top_20: result.top_20_value,
                percentile: result.percentile,
                previous: None,
                percent_change: None,
                status: None,
                baseline: None,
            }),
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

/// A hero's kit as evidence: what it casts and what is usually built on it.
///
/// No numbers, so it can never trip the figure check; it exists so advice can
/// name a real ability or item instead of a generic tip.
fn hero_kits(kits: &[HeroKit], role: Option<CoachableRole>, out: &mut Vec<Evidence>) {
    for kit in kits {
        if kit.abilities.is_empty() && kit.core_items.is_empty() {
            continue;
        }
        let mut parts = Vec::new();
        if let Some(role) = role {
            parts.push(format!("Played here as {}.", role.label()));
        }
        if !kit.roles.is_empty() {
            parts.push(format!("Hero tags: {}.", kit.roles.join(", ")));
        }
        if !kit.abilities.is_empty() {
            parts.push(format!("Abilities: {}.", kit.abilities.join(", ")));
        }
        if !kit.core_items.is_empty() {
            parts.push(format!(
                "Most-bought core items on this hero: {}.",
                kit.core_items.join(", ")
            ));
        }
        push(
            out,
            &format!("kit.{}", kit.hero_id),
            EvidenceKind::Hero,
            &format!("{} kit", kit.hero_name),
            format!("{}: {}", kit.hero_name, parts.join(" ")),
            0,
        );
    }
}

/// A percentile at or above this on a hero counts as doing well on it.
const TENSION_STRONG_PERCENTILE: f32 = 60.0;

/// Where the hero pool and the measured figures disagree about a hero.
///
/// The pool can mark a hero "risk" (results below the player's own baseline)
/// or the fit score can say "avoid for now", while the same hero carries a
/// winning record or strong peer percentiles. Both are measured and both are
/// true; dropping either would let the coach praise a hero the rest of the
/// page warns about, or the reverse. This states the disagreement so the model
/// has to address it rather than pick a side silently.
fn hero_tensions(
    pool: &[HeroPoolEntry],
    recommendations: &[HeroFit],
    benchmarks: &[BenchmarkResult],
    benchmark_hero: Option<&str>,
    out: &mut Vec<Evidence>,
) {
    use crate::domain::hero::{HeroTier, RecommendationLevel};

    let mut seen: Vec<i32> = Vec::new();
    for entry in pool.iter().take(MAX_POOL_EVIDENCE) {
        let fit = recommendations.iter().find(|f| f.hero_id == entry.hero_id);
        let warning = if entry.tier == HeroTier::Risk {
            Some(format!("classified {} in your pool", entry.tier_label.to_lowercase()))
        } else if fit.is_some_and(|f| f.level == RecommendationLevel::AvoidForNow) {
            fit.map(|f| format!("rated \"{}\" for fit", f.level_label.to_lowercase()))
        } else {
            None
        };
        let Some(warning) = warning else { continue };

        // The strongest peer reading on this hero, when it is the benchmarked one.
        let best = (benchmark_hero == Some(entry.hero_name.as_str()))
            .then(|| {
                benchmarks
                    .iter()
                    .filter_map(|r| r.percentile.map(|p| (r, p)))
                    .max_by(|a, b| a.1.total_cmp(&b.1))
            })
            .flatten()
            .filter(|(_, p)| *p >= TENSION_STRONG_PERCENTILE);
        let winning = entry.win_rate > 0.5;

        let praise = match best {
            Some((result, p)) => format!(
                "your {} on {} sits at the {} percentile",
                result.label.to_lowercase(),
                entry.hero_name,
                ordinal(p),
            ),
            None if winning => format!(
                "you are {}-{} ({}) on it",
                entry.wins,
                entry.losses,
                percent(Some(entry.win_rate)),
            ),
            None => continue,
        };

        if seen.contains(&entry.hero_id) {
            continue;
        }
        seen.push(entry.hero_id);

        push(
            out,
            &format!("tension.{}", entry.hero_id),
            EvidenceKind::Hero,
            &format!("{} mixed signals", entry.hero_name),
            format!(
                "{} is {warning}, yet {praise}. Both readings are measured and they \
                 disagree; advice about {} has to address both.",
                entry.hero_name, entry.hero_name,
            ),
            entry.matches,
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
    let position = CoachableRole::from_stored(&match_.role)
        .map_or("an unknown position", CoachableRole::label);

    push(
        out,
        "match.result",
        EvidenceKind::Match,
        "Result",
        format!(
            "This match was a {} on {} ({}, estimated), lasting {:.0} minutes.",
            if match_.won { "win" } else { "loss" },
            match_.hero_name,
            position,
            minutes,
        ),
        1,
    );

    // The baseline is the player's other eligible games on this hero in this
    // position; this match is never part of it.
    let others = stats.matches;
    let baseline = format!(
        "your {} average over {} other {}",
        match_.hero_name,
        others,
        plural(others, "game", "games"),
    );
    if others == 0 {
        push(
            out,
            "match.baseline",
            EvidenceKind::Match,
            "Personal baseline",
            format!(
                "There are no other eligible games on {} in this position to compare this one with.",
                match_.hero_name
            ),
            0,
        );
    }

    push_figure(
        out,
        "match.kda",
        EvidenceKind::Match,
        "Kills, deaths, assists",
        match (metrics.map(|m| m.kda), stats.avg_kda) {
            (Some(kda), Some(average)) => format!(
                "You finished {}/{}/{} for a KDA of {kda:.2}, against {baseline} of {average:.2}.",
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
        metrics.map(|m| EvidenceFigure {
            metric: Some("kda".into()),
            hero_id: Some(match_.hero_id),
            value: m.kda,
            higher_is_better: true,
            unit: None,
            median: None,
            top_20: None,
            percentile: None,
            previous: None,
            percent_change: None,
            status: None,
            baseline: stats.avg_kda,
        }),
    );

    if let Some(deaths_per_10) = metrics.map(|m| m.deaths_per_10) {
        push_figure(
            out,
            "match.deaths",
            EvidenceKind::Match,
            "Death rate",
            match stats.avg_deaths_per_10 {
                Some(average) => format!(
                    "That is {deaths_per_10:.1} deaths per 10 minutes, against {baseline} of {average:.1}.",
                ),
                None => format!("That is {deaths_per_10:.1} deaths per 10 minutes."),
            },
            1,
            Some(EvidenceFigure {
            metric: Some("deaths_per_10".into()),
            hero_id: Some(match_.hero_id),
            value: deaths_per_10,
            higher_is_better: false,
            unit: None,
            median: None,
            top_20: None,
            percentile: None,
            previous: None,
            percent_change: None,
            status: None,
            baseline: stats.avg_deaths_per_10,
        }),
        );
    }

    push_figure(
        out,
        "match.economy",
        EvidenceKind::Match,
        "Economy",
        match (stats.avg_gpm, stats.avg_xpm) {
            (Some(gpm), Some(xpm)) => format!(
                "You earned {} gold and {} XP per minute, against {baseline} of {gpm:.0} and {xpm:.0}.",
                match_.gpm, match_.xpm,
            ),
            _ => format!(
                "You earned {} gold and {} XP per minute.",
                match_.gpm, match_.xpm
            ),
        },
        1,
        Some(EvidenceFigure {
            metric: Some("gold_per_min".into()),
            hero_id: Some(match_.hero_id),
            value: match_.gpm as f32,
            higher_is_better: true,
            unit: None,
            median: None,
            top_20: None,
            percentile: None,
            previous: None,
            percent_change: None,
            status: None,
            baseline: stats.avg_gpm,
        }),
    );

    if let Some(kp) = metrics.and_then(|m| m.kill_participation) {
        push_figure(
            out,
            "match.kill_participation",
            EvidenceKind::Match,
            "Kill participation",
            match stats.avg_kill_participation {
                Some(average) => format!(
                    "You were involved in {} of your team's kills, against {baseline} of {}.",
                    percent(Some(kp)),
                    percent(Some(average)),
                ),
                None => format!(
                    "You were involved in {} of your team's kills.",
                    percent(Some(kp))
                ),
            },
            1,
            Some(EvidenceFigure {
            metric: Some("kill_participation".into()),
            hero_id: Some(match_.hero_id),
            value: kp,
            higher_is_better: true,
            unit: None,
            median: None,
            top_20: None,
            percentile: None,
            previous: None,
            percent_change: None,
            status: None,
            baseline: stats.avg_kill_participation,
        }),
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

/// "This game: 97th percentile" for each metric of the analysed match.
///
/// Worded apart from the average's percentile on purpose: the average can be
/// withheld for a thin sample ("Your average: needs 5 games") while the game
/// itself is always placed. One phrasing each, everywhere.
fn match_peers(
    match_: &Match,
    readings: &[MatchPeerReading],
    hero_id: Option<i32>,
    out: &mut Vec<Evidence>,
) {
    for r in readings {
        let label = r.metric.label();
        push_figure(
            out,
            &format!("match.peer.{}", r.metric.slug()),
            EvidenceKind::Match,
            &format!("{label}, this game"),
            format!(
                "This game: {} {} on {}, the {} percentile of peers.",
                round(r.value),
                label.to_lowercase(),
                match_.hero_name,
                ordinal(r.percentile),
            ),
            1,
            Some(EvidenceFigure {
                metric: Some(r.metric.slug().to_string()),
                hero_id: hero_id.or(Some(match_.hero_id)),
                value: r.value,
                higher_is_better: r.metric.higher_is_better(),
                unit: None,
                median: r.median,
                top_20: r.top_20,
                percentile: Some(r.percentile),
                previous: None,
                percent_change: None,
                status: None,
                baseline: None,
            }),
        );
    }
}

/// What changed since the previous coaching session.
///
/// The absence of this evidence is itself worth stating. A model given current
/// figures and no comparison has, in practice, a standing temptation to say
/// "this has improved" — so when there is nothing to compare, that fact is
/// written down in the same voice as everything else rather than left as a
/// gap the model can fill.
///
/// Only genuine movements are listed. A metric the engine could not compare —
/// thin sample, redefined, measured on one side only — contributes nothing
/// here, because "we do not know" is not a finding a coach should narrate.
fn progress(progress: Option<&SessionProgress>, has_session: bool, out: &mut Vec<Evidence>) {
    let Some(progress) = progress else {
        let statement = if has_session {
            "This is the first coaching session recorded for this role, so there are no \
             earlier measurements to compare against."
        } else {
            "No previous coaching session has been recorded for this role, so there are no \
             earlier measurements to compare against."
        };

        push(
            out,
            "progress.none",
            EvidenceKind::Progress,
            "No previous session",
            statement.to_string(),
            0,
        );
        return;
    };

    push(
        out,
        "progress.session",
        EvidenceKind::Progress,
        "Since your last session",
        format!(
            "These figures are coaching session {} for {}. The previous session was {}, and \
             everything below compares the two.",
            progress.current_sequence, progress.role_label, progress.previous_sequence,
        ),
        progress.matches,
    );

    if let Some(performance) = &progress.performance {
        if let (Some(previous), Some(current)) = (performance.previous, performance.current) {
            push_figure(
                out,
                "progress.performance",
                EvidenceKind::Progress,
                "Performance since your last session",
                format!(
                    "Your {} performance score is {}, {} {} at your previous coaching session.",
                    progress.role_label,
                    round(current),
                    movement_word(performance.status),
                    round(previous),
                ),
                progress.matches,
                Some(progress_figure(performance, current, previous)),
            );
        }
    }

    // The movements, biggest first, capped. A coach who lists nine changes has
    // not said which one matters.
    let mut moved: Vec<&MetricProgress> = progress
        .metrics
        .iter()
        .filter(|m| {
            m.key != "role.performance"
                && matches!(
                    m.status,
                    ProgressStatus::Improved | ProgressStatus::Declined
                )
        })
        .collect();
    moved.sort_by(|a, b| {
        b.direction_delta
            .unwrap_or(0.0)
            .abs()
            .total_cmp(&a.direction_delta.unwrap_or(0.0).abs())
    });

    for metric in moved.into_iter().take(MAX_PROGRESS_EVIDENCE) {
        let (Some(previous), Some(current)) = (metric.previous, metric.current) else {
            continue;
        };

        push_figure(
            out,
            &format!("progress.{}", metric.key),
            EvidenceKind::Progress,
            &format!("{} since your last session", metric.label),
            format!(
                "{} {} to {}, from {} at your previous coaching session.",
                metric.label,
                movement_verb(metric.status),
                round(current),
                round(previous),
            ),
            progress.matches,
            Some(progress_figure(metric, current, previous)),
        );
    }

    for metric in progress
        .metrics
        .iter()
        .filter(|m| m.status == ProgressStatus::ResolvedIssue)
        .take(MAX_PROGRESS_EVIDENCE)
    {
        push(
            out,
            &format!("progress.resolved.{}", metric.key),
            EvidenceKind::Progress,
            "Resolved since your last session",
            format!(
                "{} was flagged at your previous coaching session and is no longer detected.",
                metric.label,
            ),
            progress.matches,
        );
    }

    for metric in progress
        .metrics
        .iter()
        .filter(|m| m.status == ProgressStatus::NewIssue)
        .take(MAX_PROGRESS_EVIDENCE)
    {
        push(
            out,
            &format!("progress.new.{}", metric.key),
            EvidenceKind::Progress,
            "New since your last session",
            format!(
                "{} was not detected at your previous coaching session and is now.",
                metric.label,
            ),
            progress.matches,
        );
    }
}

/// The two readings behind a progress statement, for a before -> after row.
fn progress_figure(metric: &MetricProgress, current: f32, previous: f32) -> EvidenceFigure {
    EvidenceFigure {
        metric: None,
        hero_id: None,
        value: current,
        higher_is_better: metric.higher_is_better,
        unit: Some(metric.unit),
        median: None,
        top_20: None,
        percentile: None,
        previous: Some(previous),
        percent_change: metric.percent_change,
        status: Some(metric.status),
        baseline: None,
    }
}

/// "up from" / "down from" / "unchanged from".
fn movement_word(status: ProgressStatus) -> &'static str {
    match status {
        ProgressStatus::Improved => "up from",
        ProgressStatus::Declined => "down from",
        _ => "unchanged from",
    }
}

fn movement_verb(status: ProgressStatus) -> &'static str {
    match status {
        ProgressStatus::Improved => "improved",
        ProgressStatus::Declined => "worsened",
        _ => "moved",
    }
}

/// Figures in a progress sentence read at the precision a player thinks in.
///
/// The same rule the rest of this module uses: large numbers whole, small
/// rates with their decimals. A death rate printed as "6" when it is 5.8 would
/// be a figure the grounding check then refuses to find in the evidence.
fn round(value: f32) -> String {
    if value.abs() >= 100.0 {
        format!("{}", value.round() as i64)
    } else if value.abs() >= 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
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
    push_figure(out, id, kind, label, statement, sample, None);
}

/// One match's core item completions against peers on the hero.
///
/// `match.item.{key}`, a figure with the completion time as `value` and the
/// peer median as `median` (seconds), so the findings guard can tell a late
/// item from an on-time one without parsing the sentence.
pub fn item_timings(
    timings: &[crate::domain::item_timing::ItemTiming],
    hero: &str,
) -> Vec<Evidence> {
    use crate::domain::item_timing::TimingVerdict;
    use crate::domain::match_facts::clock;

    let mut out = Vec::new();
    for t in timings {
        let statement = match (t.peer_median_seconds, t.verdict) {
            (Some(median), Some(verdict)) => {
                let gap = (t.seconds - median).abs();
                let reading = match verdict {
                    TimingVerdict::Late => format!("{} later than peers", clock(gap)),
                    TimingVerdict::Early => format!("{} earlier than peers", clock(gap)),
                    TimingVerdict::OnTime => "on time".to_string(),
                };
                format!(
                    "{} at {}: {reading}. Peers on {hero} complete it around {} (median of {} completions, all ranks).",
                    t.label,
                    clock(t.seconds),
                    clock(median),
                    t.peer_games,
                )
            }
            _ => format!(
                "{} at {}. Not compared: too few peer completions on {hero} recorded.",
                t.label,
                clock(t.seconds),
            ),
        };
        push_figure(
            &mut out,
            &format!("match.item.{}", t.item.key()),
            EvidenceKind::Match,
            &format!("{} timing", t.label),
            statement,
            1,
            Some(EvidenceFigure {
                metric: Some("item_seconds".into()),
                hero_id: None,
                value: t.seconds as f32,
                higher_is_better: false,
                unit: None,
                median: t.peer_median_seconds.map(|m| m as f32),
                top_20: None,
                percentile: None,
                previous: None,
                percent_change: None,
                status: None,
                baseline: None,
            }),
        );
    }
    out
}

/// The same, carrying the numbers the statement was written from.
fn push_figure(
    out: &mut Vec<Evidence>,
    id: &str,
    kind: EvidenceKind,
    label: &str,
    statement: String,
    sample: i64,
    figure: Option<EvidenceFigure>,
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
        figure,
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
            role_source: "estimated".into(),
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
                .then(|| "Your average: needs 5 games for a percentile (you have 2).".to_string()),
        }
    }

    fn pool_entry() -> HeroPoolEntry {
        HeroPoolEntry {
            hero_id: 35,
            hero_name: "Luna".into(),
            role: "Carry".into(),
            position_label: "Carry",
            matches: 20,
            wins: 11,
            losses: 9,
            win_rate: 0.55,
            shrunk_win_rate: 0.55,
            win_rate_delta: None,
            recent_matches: 10,
            recent_win_rate: Some(0.6),
            avg_kda: 3.4,
            avg_gpm: 540.0,
            last_played_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
            recent_results: Vec::new(),
            meta_strength: None,
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
            scope: EvidenceScope {
                role: Some(CoachableRole::Carry),
                matches: stats.matches,
                confidence: SampleConfidence::for_matches(stats.matches),
            },
            stats,
            recent,
            benchmarks,
            benchmark_hero: Some("Luna"),
            benchmark_hero_id: Some(35),
            kits: &[],
            focus_peers: &[],
            pool,
            recommendations,
            patterns: &[],
            progress: None,
            has_session: false,
            focus: None,
            focus_match: None,
            focus_metrics: None,
        }
    }

    fn find<'a>(items: &'a [Evidence], id: &str) -> &'a str {
        items
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("{id} missing"))
            .statement
            .as_str()
    }

    #[test]
    fn a_kit_names_abilities_items_and_role_without_numbers() {
        let stats = stats();
        let kit = HeroKit {
            hero_id: 87,
            hero_name: "Disruptor".into(),
            roles: vec!["Support".into(), "Disabler".into()],
            abilities: vec!["Thunder Strike".into(), "Static Storm".into()],
            core_items: vec!["Force Staff".into(), "Glimmer Cape".into()],
        };
        let mut i = inputs(&stats, &[], &[], &[], &[]);
        let kits = [kit];
        i.kits = &kits;
        let evidence = build(&i);

        let s = find(&evidence, "kit.87");
        assert!(s.contains("Static Storm"), "{s}");
        assert!(s.contains("Force Staff"), "{s}");
        assert!(s.contains("Carry"), "{s}");
        assert!(!s.chars().any(|c| c.is_ascii_digit()), "no figures in a kit: {s}");
    }

    #[test]
    fn benchmark_evidence_carries_the_figures_its_sentence_states() {
        let stats = stats();
        let benchmarks = [benchmark(Some(46.0))];
        let evidence = build(&inputs(&stats, &[], &benchmarks, &[], &[]));
        let item = evidence.iter().find(|e| e.id == "benchmark.gold_per_min").unwrap();
        let f = item.figure.as_ref().expect("figure");
        assert_eq!(f.value, 512.0);
        assert_eq!(f.median, Some(500.0));
        assert_eq!(f.top_20, Some(684.0));
        assert_eq!(f.percentile, Some(46.0));
        assert_eq!(f.hero_id, Some(35));
        assert!(item.statement.contains("46th"));
    }

    #[test]
    fn a_risk_hero_with_strong_figures_is_flagged_as_a_tension() {
        let stats = stats();
        let benchmarks = [benchmark(Some(66.0))];
        let pool = [HeroPoolEntry {
            tier: HeroTier::Risk,
            tier_label: "Risk",
            ..pool_entry()
        }];
        let evidence = build(&inputs(&stats, &[], &benchmarks, &pool, &[]));

        let s = find(&evidence, "tension.35");
        assert!(s.contains("classified risk"), "{s}");
        assert!(s.contains("66th percentile"), "{s}");
        // The plain pool record is still there: neither reading is hidden.
        assert!(find(&evidence, "hero.35").contains("classified risk"));
    }

    #[test]
    fn an_avoid_rating_against_a_winning_record_is_a_tension_too() {
        let stats = stats();
        let pool = [pool_entry()];
        let fits = [HeroFit {
            level: RecommendationLevel::AvoidForNow,
            level_label: "Avoid for now",
            ..fit()
        }];
        let evidence = build(&inputs(&stats, &[], &[], &pool, &fits));

        let s = find(&evidence, "tension.35");
        assert!(s.contains("avoid for now"), "{s}");
        assert!(s.contains("11-9"), "{s}");
    }

    #[test]
    fn no_tension_when_the_warning_and_the_figures_agree() {
        let stats = stats();
        let benchmarks = [benchmark(Some(30.0))];
        let pool = [HeroPoolEntry {
            tier: HeroTier::Risk,
            tier_label: "Risk",
            wins: 4,
            losses: 9,
            win_rate: 0.31,
            shrunk_win_rate: 0.31,
            win_rate_delta: None,
            ..pool_entry()
        }];
        let evidence = build(&inputs(&stats, &[], &benchmarks, &pool, &[]));
        assert!(evidence.iter().all(|e| !e.id.starts_with("tension.")));

        // And none for a signature hero, however good.
        let pool = [pool_entry()];
        let evidence = build(&inputs(&stats, &[], &[benchmark(Some(90.0))], &pool, &[]));
        assert!(evidence.iter().all(|e| !e.id.starts_with("tension.")));
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

        assert!(gpm.statement.contains("Your average: needs"), "{}", gpm.statement);
        assert!(gpm.statement.contains("you have 2"));
        assert_eq!(gpm.confidence, Confidence::Insufficient);
    }

    #[test]
    fn a_single_game_is_always_placed_and_worded_as_this_game() {
        use crate::domain::benchmark::BenchmarkMetric;
        let stats = stats();
        let focus = match_(true, 1);
        let peers = [MatchPeerReading {
            metric: BenchmarkMetric::GoldPerMin,
            value: 812.0,
            percentile: 97.0,
            median: Some(540.0),
            top_20: Some(690.0),
        }];
        let benchmarks = [benchmark(None)];
        let mut input = inputs(&stats, &[], &benchmarks, &[], &[]);
        input.focus_match = Some(&focus);
        input.focus_peers = &peers;
        let evidence = build(&input);

        // The game: placed, whatever the sample behind the average.
        let game = evidence.iter().find(|e| e.id == "match.peer.gold_per_min").unwrap();
        assert!(game.statement.starts_with("This game: 812"), "{}", game.statement);
        assert!(game.statement.contains("97th percentile"));
        let f = game.figure.as_ref().unwrap();
        assert_eq!((f.value, f.percentile, f.median), (812.0, Some(97.0), Some(540.0)));
        // The average: withheld, and says so in the other phrasing.
        assert!(find(&evidence, "benchmark.gold_per_min").contains("Your average: needs"));
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
        // Says what the average is over: this hero, other games only.
        assert!(kda.statement.contains("your Luna average over 20 other games of 3.20"), "{}", kda.statement);
        assert_eq!(kda.sample, 1);
        assert_eq!(
            kda.confidence,
            Confidence::Insufficient,
            "one match generalizes to nothing, however exact it is"
        );
    }

    #[test]
    fn a_match_with_no_other_games_says_so_instead_of_comparing_with_itself() {
        let stats = PlayerStats {
            matches: 0,
            avg_kda: None,
            avg_deaths_per_10: None,
            avg_gpm: None,
            avg_xpm: None,
            avg_kill_participation: None,
            ..stats()
        };
        let focus = match_(false, 1);
        let metrics = metrics();
        let mut input = inputs(&stats, &[], &[], &[], &[]);
        input.focus_match = Some(&focus);
        input.focus_metrics = Some(&metrics);

        let evidence = build(&input);
        assert!(find(&evidence, "match.baseline").contains("no other eligible games"));
        let kda = find(&evidence, "match.kda");
        assert!(!kda.contains("average"), "no average to compare with: {kda}");
        // And no career evidence pretends there is one.
        assert!(evidence.iter().all(|e| !e.id.starts_with("overall.")));
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

#[cfg(test)]
mod progress_tests {
    use super::*;
    use crate::domain::progress::{MetricProgress, ProgressStatus, SessionProgress};
    use uuid::Uuid;

    fn metric(
        key: &str,
        label: &str,
        previous: Option<f32>,
        current: Option<f32>,
        status: ProgressStatus,
    ) -> MetricProgress {
        let direction_delta = match (previous, current) {
            (Some(p), Some(c)) => Some(c - p),
            _ => None,
        };

        MetricProgress {
            key: key.to_string(),
            label: label.to_string(),
            unit: crate::domain::coaching_session::MetricUnit::Per10,
            higher_is_better: true,
            previous,
            current,
            delta: direction_delta,
            direction_delta,
            percent_change: None,
            previous_sample: Some(20),
            current_sample: Some(20),
            status,
            status_label: status.label(),
            note: None,
        }
    }

    fn session_progress(metrics: Vec<MetricProgress>) -> SessionProgress {
        SessionProgress {
            role: CoachableRole::Carry,
            role_label: "Carry",
            previous_session_id: Uuid::new_v4(),
            previous_sequence: 1,
            previous_at: chrono::Utc::now(),
            current_session_id: Uuid::new_v4(),
            current_sequence: 2,
            current_at: chrono::Utc::now(),
            matches: 8,
            performance: Some(metric(
                "role.performance",
                "Role performance",
                Some(54.0),
                Some(61.0),
                ProgressStatus::Improved,
            )),
            metrics,
            headline: None,
        }
    }

    fn build_progress(p: Option<&SessionProgress>, has_session: bool) -> Vec<Evidence> {
        let mut out = Vec::new();
        progress(p, has_session, &mut out);
        out
    }

    fn statement<'a>(items: &'a [Evidence], id: &str) -> &'a str {
        items
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| {
                panic!(
                    "{id} present in {:?}",
                    items.iter().map(|e| &e.id).collect::<Vec<_>>()
                )
            })
            .statement
            .as_str()
    }

    #[test]
    fn every_progress_row_reports_the_same_match_count() {
        let items = build_progress(
            Some(&session_progress(vec![metric(
                "overall.deaths",
                "Deaths per 10 minutes",
                Some(7.2),
                Some(5.8),
                ProgressStatus::Improved,
            )])),
            true,
        );

        let counts: Vec<i64> = items.iter().map(|e| e.sample).collect();
        assert!(counts.len() >= 3, "header, performance and a movement");
        // The header used to say 0 while the rows said 8.
        assert!(counts.iter().all(|&n| n == 8), "{counts:?}");
    }

    #[test]
    fn the_absence_of_a_comparison_is_stated_rather_than_left_blank() {
        // A model given current figures and no comparison will reach for
        // "this has improved" unless told otherwise, so the absence is
        // written down in the same voice as everything else.
        let first = build_progress(None, true);
        assert!(statement(&first, "progress.none").contains("first coaching session"));

        let none = build_progress(None, false);
        assert!(statement(&none, "progress.none").contains("No previous coaching session"));
    }

    #[test]
    fn the_performance_change_names_both_readings() {
        let items = build_progress(Some(&session_progress(Vec::new())), true);

        let s = statement(&items, "progress.performance");
        // Both numbers appear, so the figure-grounding check can verify a
        // model that quotes either of them.
        assert!(s.contains("61"), "{s}");
        assert!(s.contains("54"), "{s}");
        assert!(s.contains("up from"), "{s}");
    }

    #[test]
    fn only_real_movements_become_evidence() {
        let items = build_progress(
            Some(&session_progress(vec![
                metric(
                    "overall.deaths",
                    "Deaths per 10 minutes",
                    Some(7.2),
                    Some(5.8),
                    ProgressStatus::Improved,
                ),
                // Neither of these is a finding a coach should narrate.
                metric(
                    "overall.kda",
                    "KDA",
                    Some(3.0),
                    Some(3.02),
                    ProgressStatus::Stable,
                ),
                metric(
                    "overall.gold_per_min",
                    "Gold per minute",
                    Some(500.0),
                    Some(505.0),
                    ProgressStatus::InsufficientData,
                ),
            ])),
            true,
        );

        assert!(items.iter().any(|e| e.id == "progress.overall.deaths"));
        assert!(!items.iter().any(|e| e.id == "progress.overall.kda"));
        assert!(!items
            .iter()
            .any(|e| e.id == "progress.overall.gold_per_min"));
    }

    #[test]
    fn resolved_and_new_issues_are_reported_separately() {
        let items = build_progress(
            Some(&session_progress(vec![
                metric(
                    "pattern.high_death_rate",
                    "Dies too often",
                    Some(0.6),
                    None,
                    ProgressStatus::ResolvedIssue,
                ),
                metric(
                    "pattern.low_cs_at_10",
                    "Low CS at 10",
                    None,
                    Some(0.5),
                    ProgressStatus::NewIssue,
                ),
            ])),
            true,
        );

        assert!(
            statement(&items, "progress.resolved.pattern.high_death_rate")
                .contains("no longer detected")
        );
        assert!(statement(&items, "progress.new.pattern.low_cs_at_10").contains("was not detected"));
    }

    #[test]
    fn every_progress_item_is_kinded_as_progress() {
        // The kind is what lets a reader — and the improvement rule in the
        // validator — tell a change from a current reading.
        let items = build_progress(
            Some(&session_progress(vec![metric(
                "overall.deaths",
                "Deaths per 10 minutes",
                Some(7.2),
                Some(5.8),
                ProgressStatus::Improved,
            )])),
            true,
        );

        assert!(items.iter().all(|e| e.kind == EvidenceKind::Progress));
        assert!(items.iter().all(|e| e.id.starts_with("progress.")));
    }

    #[test]
    fn the_movements_are_capped_so_one_change_can_still_matter() {
        let metrics: Vec<MetricProgress> = (0..8)
            .map(|i| {
                metric(
                    &format!("overall.metric_{i}"),
                    &format!("Metric {i}"),
                    Some(10.0),
                    Some(10.0 + i as f32),
                    ProgressStatus::Improved,
                )
            })
            .collect();

        let items = build_progress(Some(&session_progress(metrics)), true);
        let movements = items
            .iter()
            .filter(|e| e.id.starts_with("progress.overall."))
            .count();
        assert_eq!(movements, MAX_PROGRESS_EVIDENCE);

        // Biggest first, so the cap keeps the changes that matter most.
        assert!(items.iter().any(|e| e.id == "progress.overall.metric_7"));
        assert!(!items.iter().any(|e| e.id == "progress.overall.metric_1"));
    }

    #[test]
    fn a_late_bkb_is_stated_against_the_peer_median() {
        use crate::domain::item_timing::{compare, CoreItem, TimingBucket};
        let buckets = [
            TimingBucket { seconds: 1200, games: 8 },
            TimingBucket { seconds: 1500, games: 10 },
            TimingBucket { seconds: 1800, games: 10 },
        ];
        let late = compare(CoreItem::BlackKingBar, 52 * 60 + 2, &buckets);
        let thin = compare(CoreItem::BlinkDagger, 900, &[]);
        let ev = item_timings(&[late, thin], "Mirana");

        assert_eq!(ev[0].id, "match.item.black_king_bar");
        assert!(
            ev[0].statement.starts_with("Black King Bar at 52:02: 29:32 later than peers."),
            "{}",
            ev[0].statement
        );
        assert_eq!(ev[0].figure.as_ref().unwrap().median, Some(1350.0));
        assert!(ev[1].statement.contains("Not compared"), "{}", ev[1].statement);
        assert_eq!(ev[1].figure.as_ref().unwrap().median, None);
    }

}
