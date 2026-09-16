//! Building the long-term player model.
//!
//! Assembly, not measurement: the benchmark engine, the hero pool and the
//! pattern detectors have already done the arithmetic. What happens here is
//! deciding which of their outputs rise to "something the backend believes
//! about this player", and saying how much that belief is worth.
//!
//! Pure. The repository supplies history and the clock; nothing here reads a
//! database or a provider, so every rule below is testable in isolation.

pub mod patterns;

use crate::domain::benchmark::{BenchmarkResult, Confidence};
use crate::domain::hero::{HeroPoolEntry, HeroTier};
use crate::domain::metrics::RoleStats;
use crate::domain::player_model::{
    AnalyzedMatch, ModelConfidence, PlayerModel, PlayerTrait, RecentForm, RecurringPattern,
    RoleAffinity, TraitKind, TraitSource,
};
use crate::services::benchmarks::percentile;

/// Bumped when a detector or a trait rule changes, so stored rows are
/// identifiable as having come from an older definition.
pub const MODEL_VERSION: i32 = 1;

/// Percentiles at or beyond these are worth calling a strength or a weakness.
/// Anything between them is "about average", which is not a belief about the
/// player worth storing.
const STRENGTH_PERCENTILE: f32 = 70.0;
const WEAKNESS_PERCENTILE: f32 = 30.0;

/// The recent window for form. Ten is the same window the hero pool uses, so
/// "recent" means one thing across the product.
const FORM_WINDOW: usize = 10;
/// Win rates beyond these, over a full window, are a trait rather than noise.
const IN_FORM: f32 = 0.6;
const OUT_OF_FORM: f32 = 0.4;

/// Roles below this share of the history are not "preferred" by any reading.
const ROLE_SHARE_FLOOR: f32 = 0.1;

/// The result of a refresh: the history it read, and what it found in it.
pub struct Refreshed {
    pub history: Vec<AnalyzedMatch>,
    /// Patterns as just detected — rich, with examples and a recent window.
    /// Storage adds `first_detected_at` on the way back out.
    pub patterns: Vec<RecurringPattern>,
}

/// Re-detect the player's persistent patterns and record them.
///
/// Local only: it reads stored matches and metrics, so it never touches a
/// provider and can safely run on every sync as well as on every read of the
/// model. Detection over a few hundred rows is arithmetic.
///
/// Scoped to the competitive window across every role. Persistent patterns are
/// claims about how this player plays the game they are being coached for, and
/// a "high death rate" detected largely in Turbo would be a claim about a
/// different game — while narrowing it to one role belongs to
/// [`analyze_scope`], which does not persist anything.
pub async fn refresh(
    pool: &sqlx::PgPool,
    dota_player_id: uuid::Uuid,
    window: i64,
) -> Result<Refreshed, sqlx::Error> {
    let history = crate::repositories::player_model::history(
        pool,
        dota_player_id,
        &crate::domain::scope::MatchScope::competitive(window),
    )
    .await?;
    let patterns = patterns::detect(&history);

    crate::repositories::player_model::sync_patterns(pool, dota_player_id, &patterns).await?;
    crate::repositories::player_model::upsert_model(
        pool,
        dota_player_id,
        MODEL_VERSION,
        history.len() as i64,
        ModelConfidence::for_matches(history.len() as i64),
    )
    .await?;

    Ok(Refreshed { history, patterns })
}

/// Detect patterns inside one scope, without recording them.
///
/// The role-scoped counterpart to [`refresh`], and deliberately read-only.
/// `player_patterns` is keyed by player and pattern id, so writing a Carry
/// pattern and a Support pattern under the same id would have each overwrite
/// the other — and `first_detected_at`, the field that makes "you have been
/// doing this since March" possible, would end up meaning neither.
///
/// The cost is that a role-scoped pattern has no first-sighting date. That is
/// the honest answer for a figure computed fresh from a moving window, and it
/// is better than a date that silently belongs to a different role.
pub async fn analyze_scope(
    pool: &sqlx::PgPool,
    dota_player_id: uuid::Uuid,
    scope: &crate::domain::scope::MatchScope,
) -> Result<Refreshed, sqlx::Error> {
    let history = crate::repositories::player_model::history(pool, dota_player_id, scope).await?;
    let patterns = patterns::detect(&history);

    Ok(Refreshed { history, patterns })
}

pub struct ModelInputs<'a> {
    /// Full stored history, any order.
    pub matches: &'a [AnalyzedMatch],
    /// Benchmark results for the player's most-played hero.
    pub benchmarks: &'a [BenchmarkResult],
    pub benchmark_hero: Option<&'a str>,
    pub pool: &'a [HeroPoolEntry],
    pub roles: &'a [RoleStats],
}

/// Assemble the model.
///
/// `resolved` comes from storage: patterns detected before that no longer
/// clear the threshold. They cannot be derived from the current history — the
/// whole point is that they are no longer in it — which is why the model is
/// persisted rather than recomputed and thrown away.
pub fn build(
    inputs: &ModelInputs<'_>,
    active: Vec<RecurringPattern>,
    resolved: Vec<RecurringPattern>,
    computed_at: chrono::DateTime<chrono::Utc>,
) -> PlayerModel {
    let matches_analyzed = inputs.matches.len() as i64;
    let confidence = ModelConfidence::for_matches(matches_analyzed);
    let form = recent_form(inputs.matches);

    let mut strengths = Vec::new();
    let mut weaknesses = Vec::new();

    for trait_ in benchmark_traits(inputs.benchmarks, inputs.benchmark_hero)
        .into_iter()
        .chain(hero_pool_traits(inputs.pool))
        .chain(form_trait(&form))
    {
        match trait_.kind {
            TraitKind::Strength => strengths.push(trait_),
            TraitKind::Weakness => weaknesses.push(trait_),
        }
    }

    PlayerModel {
        model_version: MODEL_VERSION,
        matches_analyzed,
        confidence,
        confidence_label: confidence.label(),
        confidence_caveat: confidence.caveat(),
        strengths,
        weaknesses,
        preferred_roles: role_affinity(inputs.roles),
        patterns: active,
        resolved_patterns: resolved,
        recent_form: form,
        computed_at,
    }
}

/// Strengths and weaknesses read from peer comparison.
///
/// A percentile the benchmark engine withheld produces nothing: the engine
/// declined to rank the player for a reason, and re-deciding that here would
/// undo it.
fn benchmark_traits(results: &[BenchmarkResult], hero: Option<&str>) -> Vec<PlayerTrait> {
    let Some(hero) = hero else {
        return Vec::new();
    };

    results
        .iter()
        .filter(|result| result.confidence != Confidence::Insufficient)
        .filter_map(|result| {
            let percentile = result.percentile?;

            let kind = if percentile >= STRENGTH_PERCENTILE {
                TraitKind::Strength
            } else if percentile <= WEAKNESS_PERCENTILE {
                TraitKind::Weakness
            } else {
                return None;
            };

            Some(PlayerTrait {
                kind,
                source: TraitSource::Benchmark,
                key: format!("benchmark.{}", result.metric.slug()),
                label: result.label.to_string(),
                statement: format!(
                    "On {hero}, your {} sits at the {percentile:.0}th percentile across {} {}.",
                    result.label.to_lowercase(),
                    result.player_sample,
                    if result.player_sample == 1 {
                        "match"
                    } else {
                        "matches"
                    },
                ),
                sample: result.player_sample,
                confidence: result.confidence,
            })
        })
        .collect()
}

/// What the shape of the repertoire says.
fn hero_pool_traits(pool: &[HeroPoolEntry]) -> Vec<PlayerTrait> {
    if pool.is_empty() {
        return Vec::new();
    }

    let count = |tier: HeroTier| pool.iter().filter(|e| e.tier == tier).count() as i64;
    let signature = count(HeroTier::Signature);
    let risk = count(HeroTier::Risk);
    let established: i64 = pool
        .iter()
        .filter(|e| e.confidence != Confidence::Insufficient)
        .count() as i64;

    let mut traits = Vec::new();

    if signature > 0 {
        traits.push(PlayerTrait {
            kind: TraitKind::Strength,
            source: TraitSource::HeroPool,
            key: "hero_pool.signature".into(),
            label: "Signature heroes".into(),
            statement: format!(
                "You have {signature} signature {} — deep history and results above your own average.",
                if signature == 1 { "hero" } else { "heroes" },
            ),
            sample: established,
            confidence: percentile::confidence_for(established),
        });
    }

    // Only a weakness when the risky heroes outnumber the reliable ones; one
    // bad hero in a wide pool is not a problem with the pool.
    let reliable = signature + count(HeroTier::Comfort);
    if risk > reliable && risk > 0 {
        traits.push(PlayerTrait {
            kind: TraitKind::Weakness,
            source: TraitSource::HeroPool,
            key: "hero_pool.risk".into(),
            label: "Unreliable hero pool".into(),
            statement: format!(
                "{risk} of your heroes have real history and results below your own average, against {reliable} you can rely on.",
            ),
            sample: established,
            confidence: percentile::confidence_for(established),
        });
    }

    traits
}

/// Form as a trait, but only over a full window.
fn form_trait(form: &RecentForm) -> Option<PlayerTrait> {
    let win_rate = form.win_rate?;
    if form.matches < FORM_WINDOW as i64 {
        return None;
    }

    let kind = if win_rate >= IN_FORM {
        TraitKind::Strength
    } else if win_rate <= OUT_OF_FORM {
        TraitKind::Weakness
    } else {
        return None;
    };

    Some(PlayerTrait {
        kind,
        source: TraitSource::Form,
        key: "form.recent".into(),
        label: "Recent form".into(),
        statement: format!(
            "You have won {} of your last {} matches ({:.0}%).",
            form.wins,
            form.matches,
            win_rate * 100.0,
        ),
        sample: form.matches,
        confidence: percentile::confidence_for(form.matches),
    })
}

/// The recent window, newest first.
fn recent_form(matches: &[AnalyzedMatch]) -> RecentForm {
    let mut ordered: Vec<&AnalyzedMatch> = matches.iter().collect();
    ordered.sort_by_key(|m| std::cmp::Reverse(m.started_at));

    let window: Vec<&AnalyzedMatch> = ordered.into_iter().take(FORM_WINDOW).collect();
    let wins = window.iter().filter(|m| m.won).count() as i64;
    let count = window.len() as i64;

    // Signed so the sign carries the direction and the magnitude carries the
    // length: +4 is four straight wins, -4 is four straight losses.
    let streak = window
        .first()
        .map(|first| {
            let run = window.iter().take_while(|m| m.won == first.won).count() as i32;
            if first.won {
                run
            } else {
                -run
            }
        })
        .unwrap_or(0);

    RecentForm {
        matches: count,
        wins,
        // No matches means no win rate, not a 0% one.
        win_rate: (count > 0).then(|| wins as f32 / count as f32),
        streak,
    }
}

/// Roles by how much the player actually plays them.
fn role_affinity(roles: &[RoleStats]) -> Vec<RoleAffinity> {
    let total: i64 = roles.iter().map(|r| r.matches).sum();
    if total == 0 {
        return Vec::new();
    }

    let mut affinity: Vec<RoleAffinity> = roles
        .iter()
        .map(|role| RoleAffinity {
            role: role.role.clone(),
            matches: role.matches,
            share: role.matches as f32 / total as f32,
            win_rate: role.win_rate,
        })
        .filter(|role| role.share >= ROLE_SHARE_FLOOR)
        .collect();

    affinity.sort_by(|a, b| b.matches.cmp(&a.matches).then_with(|| a.role.cmp(&b.role)));
    affinity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::benchmark::{BenchmarkMetric, Segment};
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn match_at(index: i64, won: bool) -> AnalyzedMatch {
        AnalyzedMatch {
            match_id: Uuid::new_v4(),
            started_at: Utc.timestamp_opt(1_700_000_000 + index * 3_600, 0).unwrap(),
            hero_id: 35,
            hero_name: "Luna".into(),
            role: "Carry".into(),
            won,
            duration_seconds: 2_400,
            gpm: 500,
            deaths_per_10: 1.0,
            kill_participation: Some(0.7),
            tower_damage: Some(3_000),
            last_hits_at_10: None,
            gold_at_10: None,
            bkb_seconds: None,
        }
    }

    fn benchmark(percentile: Option<f32>, confidence: Confidence) -> BenchmarkResult {
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
            confidence,
            segmented_by: vec![Segment::Hero],
            note: None,
        }
    }

    fn pool_entry(hero_id: i32, tier: HeroTier, confidence: Confidence) -> HeroPoolEntry {
        HeroPoolEntry {
            hero_id,
            hero_name: format!("Hero {hero_id}"),
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
            tier,
            tier_label: tier.label(),
            confidence,
        }
    }

    fn role(name: &str, matches: i64) -> RoleStats {
        RoleStats {
            role: name.into(),
            matches,
            wins: matches / 2,
            win_rate: 0.5,
            avg_kda: 3.0,
            avg_gpm: 500.0,
        }
    }

    fn inputs<'a>(
        matches: &'a [AnalyzedMatch],
        benchmarks: &'a [BenchmarkResult],
        pool: &'a [HeroPoolEntry],
        roles: &'a [RoleStats],
    ) -> ModelInputs<'a> {
        ModelInputs {
            matches,
            benchmarks,
            benchmark_hero: Some("Luna"),
            pool,
            roles,
        }
    }

    fn model(inputs: &ModelInputs<'_>) -> PlayerModel {
        build(inputs, Vec::new(), Vec::new(), Utc::now())
    }

    #[test]
    fn a_high_percentile_is_a_strength_and_a_low_one_a_weakness() {
        let matches = [match_at(0, true)];
        let strong = model(&inputs(
            &matches,
            &[benchmark(Some(85.0), Confidence::Adequate)],
            &[],
            &[],
        ));
        assert_eq!(strong.strengths.len(), 1);
        assert!(strong.strengths[0].statement.contains("85th percentile"));

        let weak = model(&inputs(
            &matches,
            &[benchmark(Some(12.0), Confidence::Adequate)],
            &[],
            &[],
        ));
        assert_eq!(weak.weaknesses.len(), 1);
    }

    #[test]
    fn an_average_percentile_is_neither() {
        let matches = [match_at(0, true)];
        let built = model(&inputs(
            &matches,
            &[benchmark(Some(50.0), Confidence::Adequate)],
            &[],
            &[],
        ));

        assert!(built.strengths.is_empty());
        assert!(built.weaknesses.is_empty());
    }

    #[test]
    fn a_withheld_percentile_produces_no_trait() {
        let matches = [match_at(0, true)];
        // The benchmark engine declined to rank this; the model must not
        // re-decide that.
        let built = model(&inputs(
            &matches,
            &[benchmark(None, Confidence::Insufficient)],
            &[],
            &[],
        ));

        assert!(built.strengths.is_empty());
        assert!(built.weaknesses.is_empty());
    }

    #[test]
    fn signature_heroes_are_a_strength() {
        let matches = [match_at(0, true)];
        let pool = [
            pool_entry(1, HeroTier::Signature, Confidence::Adequate),
            pool_entry(2, HeroTier::Comfort, Confidence::Adequate),
        ];

        let built = model(&inputs(&matches, &[], &pool, &[]));
        let signature = built
            .strengths
            .iter()
            .find(|t| t.key == "hero_pool.signature")
            .unwrap();

        assert!(signature.statement.contains("1 signature hero"));
    }

    #[test]
    fn one_bad_hero_in_a_good_pool_is_not_a_weakness() {
        let matches = [match_at(0, true)];
        let pool = [
            pool_entry(1, HeroTier::Signature, Confidence::Adequate),
            pool_entry(2, HeroTier::Comfort, Confidence::Adequate),
            pool_entry(3, HeroTier::Risk, Confidence::Adequate),
        ];

        let built = model(&inputs(&matches, &[], &pool, &[]));
        assert!(!built.weaknesses.iter().any(|t| t.key == "hero_pool.risk"));
    }

    #[test]
    fn a_pool_of_mostly_risky_heroes_is_a_weakness() {
        let matches = [match_at(0, true)];
        let pool = [
            pool_entry(1, HeroTier::Risk, Confidence::Adequate),
            pool_entry(2, HeroTier::Risk, Confidence::Adequate),
            pool_entry(3, HeroTier::Comfort, Confidence::Adequate),
        ];

        let built = model(&inputs(&matches, &[], &pool, &[]));
        assert!(built.weaknesses.iter().any(|t| t.key == "hero_pool.risk"));
    }

    #[test]
    fn form_is_only_a_trait_over_a_full_window() {
        // Five straight wins is a good week, not a trait.
        let short: Vec<AnalyzedMatch> = (0..5).map(|i| match_at(i, true)).collect();
        let built = model(&inputs(&short, &[], &[], &[]));
        assert!(!built.strengths.iter().any(|t| t.key == "form.recent"));

        let full: Vec<AnalyzedMatch> = (0..10).map(|i| match_at(i, true)).collect();
        let built = model(&inputs(&full, &[], &[], &[]));
        assert!(built.strengths.iter().any(|t| t.key == "form.recent"));
    }

    #[test]
    fn the_recent_window_reads_the_newest_matches() {
        // Twenty matches: the older ten lost, the newer ten won.
        let mut matches: Vec<AnalyzedMatch> = (0..10).map(|i| match_at(i, false)).collect();
        matches.extend((10..20).map(|i| match_at(i, true)));

        let built = model(&inputs(&matches, &[], &[], &[]));

        assert_eq!(built.recent_form.matches, 10);
        assert_eq!(built.recent_form.wins, 10);
        assert_eq!(built.recent_form.streak, 10);
    }

    #[test]
    fn a_losing_streak_is_signed_negative() {
        let mut matches: Vec<AnalyzedMatch> = (0..10).map(|i| match_at(i, true)).collect();
        matches.extend((10..14).map(|i| match_at(i, false)));

        let built = model(&inputs(&matches, &[], &[], &[]));
        assert_eq!(built.recent_form.streak, -4);
    }

    #[test]
    fn roles_are_ordered_by_use_and_rare_ones_dropped() {
        let matches = [match_at(0, true)];
        let roles = [role("Support", 10), role("Carry", 30), role("Mid", 1)];

        let built = model(&inputs(&matches, &[], &[], &roles));

        assert_eq!(built.preferred_roles.len(), 2, "Mid is 2% of the history");
        assert_eq!(built.preferred_roles[0].role, "Carry");
        assert!((built.preferred_roles[0].share - 30.0 / 41.0).abs() < 0.01);
    }

    #[test]
    fn confidence_reflects_how_much_history_there_is() {
        let sparse: Vec<AnalyzedMatch> = (0..5).map(|i| match_at(i, true)).collect();
        assert_eq!(
            model(&inputs(&sparse, &[], &[], &[])).confidence,
            ModelConfidence::Sparse
        );

        let established: Vec<AnalyzedMatch> = (0..50).map(|i| match_at(i, true)).collect();
        let built = model(&inputs(&established, &[], &[], &[]));
        assert_eq!(built.confidence, ModelConfidence::Established);
        assert_eq!(built.matches_analyzed, 50);
    }

    #[test]
    fn an_empty_history_produces_an_empty_model_rather_than_zeroes() {
        let built = model(&inputs(&[], &[], &[], &[]));

        assert_eq!(built.matches_analyzed, 0);
        assert!(built.strengths.is_empty());
        assert!(built.weaknesses.is_empty());
        assert_eq!(built.recent_form.win_rate, None);
        assert_eq!(built.recent_form.streak, 0);
    }
}
