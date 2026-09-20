//! Building a coaching session snapshot.
//!
//! Pure: no database, no provider, no clock. Everything it needs has already
//! been computed by the layers below it, and the point of this module is to
//! pick out the *numbers* and write them down in a form a later session can be
//! subtracted from.
//!
//! It invents nothing. Every metric here has an existing owner —
//! [`RolePerformance`] for the role score,
//! [`PlayerStats`](crate::domain::metrics::PlayerStats) for the career
//! figures, [`BenchmarkResult`] for peer placement — and this module only
//! restates them with a stable key and a unit attached.
//!
//! The counterpart to [`crate::services::coaching::evidence`], which takes the
//! same inputs and writes *sentences* for a model to cite. Two renderings of
//! one set of facts, deliberately separate: prose cannot be compared, and
//! numbers cannot be read aloud.

use uuid::Uuid;

use crate::domain::benchmark::BenchmarkResult;
use crate::domain::coaching_session::{
    BenchmarkSnapshot, CoachingSession, HeroSnapshot, MetricSnapshot, MetricUnit, SessionDraft,
};
use crate::domain::metrics::{HeroStats, PlayerStats};
use crate::domain::player_model::PlayerModel;
use crate::domain::r#match::Match;
use crate::domain::role::{CoachableRole, RolePerformance};
use crate::domain::training::TrainingFocus;

/// How many new eligible matches in the role justify a new session.
///
/// Ten, matching [`crate::services::training::WINDOW`] rather than being
/// chosen independently: that is already the bucket size the progress series
/// uses, so a session boundary lands on a progress point instead of
/// interleaving with one.
///
/// Below this, a session would mostly re-measure the games the previous one
/// already read, and every comparison between the two would be reporting
/// noise as change.
pub const MIN_NEW_MATCHES: usize = 10;

/// How many heroes a session records.
///
/// A snapshot is a summary, not an archive. The tail of one-game heroes says
/// nothing a comparison can use.
const MAX_HEROES: usize = 5;

/// Everything a session is built from. All of it already computed.
pub struct SessionInputs<'a> {
    pub role: CoachableRole,
    /// The exact matches every figure below was read from, newest first.
    pub matches: &'a [Match],
    /// The player's role row, when the role had enough to score.
    pub role_performance: Option<&'a RolePerformance>,
    pub stats: &'a PlayerStats,
    pub model: &'a PlayerModel,
    pub benchmarks: &'a [BenchmarkResult],
    pub heroes: &'a [HeroStats],
    pub focus: Option<&'a TrainingFocus>,
}

/// Compose a session from figures the backend already produced.
pub fn build(inputs: &SessionInputs<'_>) -> SessionDraft {
    let analyzed_match_ids: Vec<Uuid> = inputs.matches.iter().map(|m| m.id).collect();
    // The matches arrive newest first, so the window's upper edge is the head.
    let newest_match_at = inputs.matches.first().map(|m| m.started_at);

    SessionDraft {
        role: inputs.role,
        analyzed_match_count: analyzed_match_ids.len() as i32,
        analyzed_match_ids,
        newest_match_at,
        performance: inputs.role_performance.map(|r| r.performance),
        metrics: metrics(inputs),
        strengths: inputs.model.strengths.clone(),
        weaknesses: inputs.model.weaknesses.clone(),
        benchmarks: benchmark_snapshots(inputs.benchmarks),
        heroes: hero_snapshots(inputs.heroes),
        training_focus_id: inputs.focus.and_then(|f| f.id),
    }
}

/// The comparable numbers, in a stable order.
///
/// A figure the player has no data for is **absent**, never zero: a session
/// recording `kill_participation = 0` for a player whose matches never carried
/// team totals would later read as a catastrophic decline from nothing.
fn metrics(inputs: &SessionInputs<'_>) -> Vec<MetricSnapshot> {
    let mut out = Vec::new();
    let sample = inputs.stats.matches;

    if let Some(role) = inputs.role_performance {
        out.push(snapshot(
            "role.performance",
            "Role performance",
            role.performance,
            role.matches,
            MetricUnit::Score,
            true,
        ));
        out.push(snapshot(
            "role.win_rate",
            "Win rate",
            role.win_rate,
            role.matches,
            MetricUnit::Proportion,
            true,
        ));
    }

    let career = [
        (
            "overall.kda",
            "KDA",
            inputs.stats.avg_kda,
            MetricUnit::Ratio,
            true,
        ),
        (
            "overall.gold_per_min",
            "Gold per minute",
            inputs.stats.avg_gpm,
            MetricUnit::PerMinute,
            true,
        ),
        (
            "overall.xp_per_min",
            "XP per minute",
            inputs.stats.avg_xpm,
            MetricUnit::PerMinute,
            true,
        ),
        (
            "overall.last_hits",
            "Last hits",
            inputs.stats.avg_last_hits,
            MetricUnit::Count,
            true,
        ),
        (
            "overall.deaths",
            "Deaths per 10 minutes",
            inputs.stats.avg_deaths_per_10,
            MetricUnit::Per10,
            // The one metric where less is more. Recorded per snapshot so a
            // later comparison never has to know which key means what.
            false,
        ),
        (
            "overall.kills",
            "Kills per 10 minutes",
            inputs.stats.avg_kills_per_10,
            MetricUnit::Per10,
            true,
        ),
    ];

    for (key, label, value, unit, higher_is_better) in career {
        if let Some(value) = value {
            out.push(snapshot(key, label, value, sample, unit, higher_is_better));
        }
    }

    // Kill participation carries its own denominator: it is averaged only over
    // the matches that supplied team totals, which is not every match.
    if let Some(value) = inputs.stats.avg_kill_participation {
        out.push(snapshot(
            "overall.kill_participation",
            "Kill participation",
            value,
            inputs.stats.kill_participation_sample,
            MetricUnit::Proportion,
            true,
        ));
    }

    // Peer placement, which moves for reasons the raw figures do not — the
    // player can improve and still lose percentile if the bracket did too.
    for result in inputs.benchmarks {
        if let Some(percentile) = result.percentile {
            out.push(snapshot(
                &format!("benchmark.{}", result.metric.slug()),
                result.label,
                percentile,
                result.player_sample,
                MetricUnit::Percentile,
                // A percentile is direction-corrected upstream, so higher is
                // better here even for deaths.
                true,
            ));
        }
    }

    // Recurring patterns as rates, so "you did this in 60% of measurable
    // games" is comparable session to session.
    for pattern in &inputs.model.patterns {
        out.push(snapshot(
            &format!("pattern.{}", pattern.id),
            &pattern.label,
            pattern.rate,
            pattern.measured,
            MetricUnit::Proportion,
            false,
        ));
    }

    out
}

fn snapshot(
    key: &str,
    label: &str,
    value: f32,
    sample: i64,
    unit: MetricUnit,
    higher_is_better: bool,
) -> MetricSnapshot {
    MetricSnapshot {
        key: key.to_string(),
        label: label.to_string(),
        value,
        sample,
        unit,
        higher_is_better,
    }
}

fn benchmark_snapshots(results: &[BenchmarkResult]) -> Vec<BenchmarkSnapshot> {
    results
        .iter()
        .map(|r| BenchmarkSnapshot {
            metric: r.metric.slug().to_string(),
            label: r.label.to_string(),
            player_value: r.player_value,
            peer_median: r.peer_median,
            percentile: r.percentile,
            higher_is_better: r.higher_is_better,
        })
        .collect()
}

fn hero_snapshots(heroes: &[HeroStats]) -> Vec<HeroSnapshot> {
    heroes
        .iter()
        .take(MAX_HEROES)
        .map(|h| HeroSnapshot {
            hero_id: h.hero_id,
            hero_name: h.hero_name.clone(),
            matches: h.matches,
            wins: h.wins,
            win_rate: h.win_rate,
            avg_kda: Some(h.avg_kda),
        })
        .collect()
}

/// Whether enough has happened to justify a new session.
///
/// Measured in *matches this session has not already read*, not in elapsed
/// time: a player who has not played has nothing new to say, however long it
/// has been.
///
/// The first session is always justified — there is nothing to re-measure.
pub fn should_create(latest: Option<&CoachingSession>, current_match_ids: &[Uuid]) -> bool {
    let Some(latest) = latest else {
        return !current_match_ids.is_empty();
    };

    new_match_count(latest, current_match_ids) >= MIN_NEW_MATCHES
}

/// How many of the current window's matches the last session did not read.
///
/// Set difference rather than a count comparison: a window that has slid — new
/// games in, old games out — can hold the same *number* of matches while being
/// substantially different games.
pub fn new_match_count(latest: &CoachingSession, current_match_ids: &[Uuid]) -> usize {
    let seen: std::collections::HashSet<&Uuid> = latest.analyzed_match_ids.iter().collect();
    current_match_ids
        .iter()
        .filter(|id| !seen.contains(id))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::benchmark::{BenchmarkMetric, Confidence, Segment};
    use crate::domain::player_model::{
        ModelConfidence, PlayerTrait, RecentForm, TraitKind, TraitSource,
    };

    fn stats(matches: i64) -> PlayerStats {
        PlayerStats {
            matches,
            wins: matches / 2,
            losses: matches / 2,
            win_rate: Some(0.5),
            avg_kda: Some(3.2),
            avg_gpm: Some(512.0),
            avg_xpm: Some(600.0),
            avg_last_hits: Some(210.0),
            avg_deaths_per_10: Some(5.8),
            avg_kills_per_10: Some(2.1),
            avg_hero_damage: Some(20_000.0),
            avg_kill_participation: Some(0.62),
            kill_participation_sample: matches - 2,
            parsed_matches: 4,
        }
    }

    fn model() -> PlayerModel {
        PlayerModel {
            model_version: 1,
            matches_analyzed: 20,
            confidence: ModelConfidence::Developing,
            confidence_label: "Developing",
            confidence_caveat: "",
            strengths: vec![PlayerTrait {
                kind: TraitKind::Strength,
                source: TraitSource::Benchmark,
                key: "benchmark.gold_per_min".into(),
                label: "Gold per minute".into(),
                statement: "Strong farm.".into(),
                sample: 20,
                confidence: Confidence::Adequate,
            }],
            weaknesses: Vec::new(),
            preferred_roles: Vec::new(),
            patterns: Vec::new(),
            resolved_patterns: Vec::new(),
            recent_form: RecentForm {
                matches: 10,
                wins: 6,
                win_rate: Some(0.6),
                streak: 2,
            },
            computed_at: chrono::Utc::now(),
        }
    }

    fn benchmark(percentile: Option<f32>) -> BenchmarkResult {
        BenchmarkResult {
            metric: BenchmarkMetric::GoldPerMin,
            label: "Gold per minute",
            higher_is_better: true,
            player_value: 512.0,
            player_sample: 20,
            peer_median: Some(500.0),
            top_20_value: Some(684.0),
            percentile,
            gap_to_top_20: Some(172.0),
            peer_sample_size: None,
            confidence: Confidence::Adequate,
            segmented_by: vec![Segment::Hero],
            note: None,
        }
    }

    fn inputs<'a>(
        stats: &'a PlayerStats,
        model: &'a PlayerModel,
        benchmarks: &'a [BenchmarkResult],
    ) -> SessionInputs<'a> {
        SessionInputs {
            role: CoachableRole::Carry,
            matches: &[],
            role_performance: None,
            stats,
            model,
            benchmarks,
            heroes: &[],
            focus: None,
        }
    }

    #[test]
    fn career_figures_become_numbers_with_units() {
        let (s, m) = (stats(20), model());
        let draft = build(&inputs(&s, &m, &[]));

        let deaths = draft
            .metrics
            .iter()
            .find(|x| x.key == "overall.deaths")
            .unwrap();
        assert_eq!(deaths.value, 5.8);
        assert_eq!(deaths.unit, MetricUnit::Per10);
        // The one metric where less is more, recorded so a later comparison
        // does not have to know that by name.
        assert!(!deaths.higher_is_better);

        let gpm = draft
            .metrics
            .iter()
            .find(|x| x.key == "overall.gold_per_min")
            .unwrap();
        assert_eq!(gpm.unit, MetricUnit::PerMinute);
        assert!(gpm.higher_is_better);
    }

    #[test]
    fn a_figure_the_player_has_no_data_for_is_absent_not_zero() {
        let mut s = stats(20);
        s.avg_kill_participation = None;
        s.avg_deaths_per_10 = None;
        let m = model();

        let draft = build(&inputs(&s, &m, &[]));

        // A zero here would later read as a total collapse from nothing.
        assert!(draft
            .metrics
            .iter()
            .all(|x| x.key != "overall.kill_participation"));
        assert!(draft.metrics.iter().all(|x| x.key != "overall.deaths"));
    }

    #[test]
    fn kill_participation_carries_its_own_denominator() {
        let (s, m) = (stats(20), model());
        let draft = build(&inputs(&s, &m, &[]));

        let kp = draft
            .metrics
            .iter()
            .find(|x| x.key == "overall.kill_participation")
            .unwrap();
        // Not 20: it is averaged only over matches that carried team totals.
        assert_eq!(kp.sample, 18);
    }

    #[test]
    fn a_benchmark_percentile_is_recorded_as_higher_is_better() {
        let (s, m) = (stats(20), model());
        let benchmarks = [benchmark(Some(46.0))];
        let draft = build(&inputs(&s, &m, &benchmarks));

        let p = draft
            .metrics
            .iter()
            .find(|x| x.key == "benchmark.gold_per_min")
            .unwrap();
        assert_eq!(p.value, 46.0);
        assert_eq!(p.unit, MetricUnit::Percentile);
        // Direction was corrected upstream; a percentile is always "higher is
        // better", including for deaths.
        assert!(p.higher_is_better);
    }

    #[test]
    fn a_withheld_percentile_never_becomes_a_metric() {
        let (s, m) = (stats(2), model());
        let benchmarks = [benchmark(None)];
        let draft = build(&inputs(&s, &m, &benchmarks));

        // The benchmark engine withheld it for a thin sample. Recording it as
        // anything would reinstate the claim it declined to make.
        assert!(draft
            .metrics
            .iter()
            .all(|x| !x.key.starts_with("benchmark.")));
        // The snapshot still keeps the raw values, which were never in doubt.
        assert_eq!(draft.benchmarks.len(), 1);
        assert_eq!(draft.benchmarks[0].percentile, None);
    }

    #[test]
    fn a_session_builds_with_nothing_measured() {
        // A brand-new player: no role score, no benchmarks, no patterns.
        let mut s = stats(0);
        s.avg_kda = None;
        s.avg_gpm = None;
        s.avg_xpm = None;
        s.avg_last_hits = None;
        s.avg_deaths_per_10 = None;
        s.avg_kills_per_10 = None;
        s.avg_kill_participation = None;
        let m = model();

        let draft = build(&inputs(&s, &m, &[]));

        assert_eq!(draft.performance, None);
        assert!(draft.metrics.is_empty());
        assert_eq!(draft.analyzed_match_count, 0);
        assert_eq!(draft.newest_match_at, None);
    }

    #[test]
    fn the_first_session_is_always_justified() {
        assert!(should_create(None, &[Uuid::new_v4()]));
        // Except when there is genuinely nothing to measure.
        assert!(!should_create(None, &[]));
    }

    fn stored(ids: Vec<Uuid>) -> CoachingSession {
        CoachingSession {
            id: Uuid::new_v4(),
            role: CoachableRole::Carry,
            role_label: "Carry",
            sequence: 1,
            analyzed_match_count: ids.len() as i32,
            analyzed_match_ids: ids,
            newest_match_at: None,
            performance: Some(54.0),
            metrics: Vec::new(),
            strengths: Vec::new(),
            weaknesses: Vec::new(),
            benchmarks: Vec::new(),
            heroes: Vec::new(),
            training_focus_id: None,
            analysis_id: None,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn a_session_needs_ten_matches_the_last_one_did_not_read() {
        let seen: Vec<Uuid> = (0..20).map(|_| Uuid::new_v4()).collect();
        let latest = stored(seen.clone());

        // Nine new games is not yet a session.
        let mut window = seen.clone();
        window.extend((0..9).map(|_| Uuid::new_v4()));
        assert!(!should_create(Some(&latest), &window));

        // The tenth makes it one.
        window.push(Uuid::new_v4());
        assert!(should_create(Some(&latest), &window));
    }

    #[test]
    fn a_window_that_slid_is_measured_by_which_games_are_new() {
        // The window holds 20 matches before and after, but ten of them are
        // different games. A count comparison would see no change at all.
        let old: Vec<Uuid> = (0..20).map(|_| Uuid::new_v4()).collect();
        let latest = stored(old.clone());

        let mut window: Vec<Uuid> = old.iter().skip(10).copied().collect();
        window.extend((0..10).map(|_| Uuid::new_v4()));

        assert_eq!(window.len(), 20);
        assert_eq!(new_match_count(&latest, &window), 10);
        assert!(should_create(Some(&latest), &window));
    }

    #[test]
    fn replaying_the_same_window_is_not_a_new_session() {
        let seen: Vec<Uuid> = (0..20).map(|_| Uuid::new_v4()).collect();
        let latest = stored(seen.clone());

        assert_eq!(new_match_count(&latest, &seen), 0);
        assert!(!should_create(Some(&latest), &seen));
    }

    #[test]
    fn only_the_top_heroes_are_recorded() {
        let heroes: Vec<HeroStats> = (0..8)
            .map(|i| HeroStats {
                hero_id: i,
                hero_name: format!("Hero {i}"),
                matches: 10 - i as i64,
                wins: 5,
                win_rate: 0.5,
                avg_kda: 3.0,
                avg_gpm: 500.0,
                last_played_at: chrono::Utc::now(),
            })
            .collect();

        let (s, m) = (stats(20), model());
        let mut i = inputs(&s, &m, &[]);
        i.heroes = &heroes;

        let draft = build(&i);
        assert_eq!(draft.heroes.len(), MAX_HEROES);
        assert_eq!(draft.heroes[0].hero_id, 0);
    }
}
