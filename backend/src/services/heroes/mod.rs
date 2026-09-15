//! Hero Intelligence: the player's own repertoire, and which heroes actually
//! fit them right now.
//!
//! Everything here is pure. The hero pool arrives as SQL rows, the meta as a
//! provider cohort, and the benchmark as an already-computed percentile; this
//! module only combines them. That is what makes the fit score testable, and
//! what keeps the spec's central rule enforceable:
//!
//! > a strong meta hero is still a bad recommendation if the player has no
//! > experience on it, and a slightly weaker one can be an excellent
//! > recommendation if it is already one of their best.
//!
//! The LLM never sees any of this arithmetic — it receives the finished
//! numbers and the reasons behind them.

use crate::domain::benchmark::Confidence;
use crate::domain::hero::{
    FitComponent, FitPart, FitWeights, HeroFit, HeroMeta, HeroPoolEntry, HeroTier,
    RecommendationLevel,
};
use crate::repositories::hero_pool::HeroPoolRow;
use crate::services::benchmarks::percentile;

/// How many of the player's most recent matches on a hero count as "recent
/// form". Small enough to react, large enough not to be a coin flip.
pub const RECENT_WINDOW: i64 = 10;

/// Below this many matches, a hero is being learned rather than played.
pub const STRETCH_MATCHES: i64 = 5;
/// At or above this, plus results above the player's own baseline, a hero is
/// signature.
pub const SIGNATURE_MATCHES: i64 = 10;
/// Where the experience component reaches 100.
const EXPERIENCE_SATURATION: f32 = 20.0;
/// How far a hero's win rate must sit from the player's baseline before it is
/// read as a real difference rather than noise.
const BASELINE_BAND: f32 = 0.05;

/// Fit at or above this is a recommendation; below the second, an avoid.
const RECOMMEND_AT: f32 = 70.0;
const CONSIDER_AT: f32 = 50.0;

/// Recent form this poor downgrades a hero regardless of its history.
const POOR_RECENT_WIN_RATE: f32 = 0.35;

/// The most training-focus compatibility may move a fit score, in points.
///
/// Small on purpose: the spec is explicit that focus compatibility is a
/// modifier, not a sixth weighted component. It breaks ties between heroes a
/// player could reasonably pick; it never manufactures a recommendation.
const FOCUS_MODIFIER: f32 = 5.0;

/// The player's own averages, used as the reference a hero is judged against.
///
/// Comparing a hero to the player rather than to a fixed 50% is the point: a
/// 48% hero is a strength for a 42% player and a weakness for a 55% one.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlayerBaseline {
    pub win_rate: Option<f32>,
    pub avg_kda: Option<f32>,
}

impl PlayerBaseline {
    fn win_rate_or_even(&self) -> f32 {
        self.win_rate.unwrap_or(0.5)
    }
}

/// Turn stored history into the player's hero pool.
pub fn build_pool(rows: &[HeroPoolRow], baseline: PlayerBaseline) -> Vec<HeroPoolEntry> {
    rows.iter()
        .map(|row| {
            let win_rate = row.wins as f32 / row.matches.max(1) as f32;
            let tier = classify(row.matches, win_rate, baseline);

            HeroPoolEntry {
                hero_id: row.hero_id,
                hero_name: row.hero_name.clone(),
                role: row.role.clone(),
                matches: row.matches,
                wins: row.wins,
                losses: row.matches - row.wins,
                win_rate,
                recent_matches: row.recent_matches,
                recent_win_rate: (row.recent_matches > 0)
                    .then(|| row.recent_wins as f32 / row.recent_matches as f32),
                avg_kda: row.avg_kda,
                avg_gpm: row.avg_gpm,
                last_played_at: row.last_played_at,
                tier,
                tier_label: tier.label(),
                confidence: percentile::confidence_for(row.matches),
            }
        })
        .collect()
}

/// Where a hero sits in the repertoire.
///
/// Evidence first: a hero with three games is `Stretch` however those three
/// went, because three games cannot distinguish a signature hero from a lucky
/// streak. Only once there is history does the result matter.
fn classify(matches: i64, win_rate: f32, baseline: PlayerBaseline) -> HeroTier {
    let reference = baseline.win_rate_or_even();

    if matches < STRETCH_MATCHES {
        HeroTier::Stretch
    } else if matches >= SIGNATURE_MATCHES && win_rate >= reference + BASELINE_BAND {
        HeroTier::Signature
    } else if win_rate >= reference - BASELINE_BAND {
        HeroTier::Comfort
    } else {
        HeroTier::Risk
    }
}

/// Everything known about one candidate hero at scoring time.
///
/// Each optional field means "not knowable", never "zero": a hero with no pool
/// entry has never been played, and a missing benchmark means the provider was
/// unavailable. The two are treated very differently below.
pub struct FitInput<'a> {
    pub hero_id: i32,
    pub hero_name: &'a str,
    pub pool: Option<&'a HeroPoolEntry>,
    pub meta: Option<&'a HeroMeta>,
    /// 0-100, averaged across the metrics the benchmark engine could place.
    pub benchmark_percentile: Option<f32>,
    /// -1 to 1: how this hero's figures compare to the player's own for
    /// whatever they are currently working on. `None` when there is no focus,
    /// or too little history on the hero to say.
    pub focus_alignment: Option<f32>,
    /// What the focus is called, for the reason line.
    pub focus_title: Option<&'a str>,
    pub baseline: PlayerBaseline,
}

/// Score one hero for one player.
///
/// Components whose input is unknowable are dropped and the remaining weights
/// renormalized, so a missing benchmark lowers *confidence* rather than the
/// score. Zero experience, by contrast, is knowledge: it scores 0 and is
/// weighted like any other component.
pub fn score(input: &FitInput<'_>, weights: FitWeights) -> HeroFit {
    let mut parts: Vec<FitPart> = Vec::new();
    let mut reasons: Vec<String> = Vec::new();
    let mut caveats: Vec<String> = Vec::new();

    let matches = input.pool.map(|p| p.matches).unwrap_or(0);

    // --- Experience ------------------------------------------------------
    // Always knowable: never having played a hero is a fact about the player.
    // Square-rooted so the first few games count for more than the twentieth.
    let experience = 100.0
        * (matches as f32 / EXPERIENCE_SATURATION)
            .clamp(0.0, 1.0)
            .sqrt();
    parts.push(part(
        FitComponent::Experience,
        experience,
        weights,
        match matches {
            0 => "You have never played this hero.".to_string(),
            1 => "1 stored match on this hero.".to_string(),
            n => format!("{n} stored matches on this hero."),
        },
    ));

    // --- Player performance ----------------------------------------------
    if let Some(pool) = input.pool.filter(|p| p.matches > 0) {
        let reference_wr = input.baseline.win_rate_or_even();
        let win_delta = pool.win_rate - reference_wr;
        // 10 points of win rate is worth 20 points of score.
        let win_score = (50.0 + win_delta * 200.0).clamp(0.0, 100.0);

        // KDA as a ratio against the player's own average, so an even
        // performance lands on 50 whatever their absolute KDA looks like.
        let kda_score = match input.baseline.avg_kda.filter(|k| *k > 0.0) {
            Some(reference) => (50.0 * (pool.avg_kda / reference)).clamp(0.0, 100.0),
            None => 50.0,
        };

        // Thin history is pulled toward neutral rather than trusted.
        let evidence = (pool.matches as f32 / SIGNATURE_MATCHES as f32).clamp(0.0, 1.0);
        let raw = (win_score + kda_score) / 2.0;
        let performance = 50.0 + (raw - 50.0) * evidence;

        parts.push(part(
            FitComponent::PlayerPerformance,
            performance,
            weights,
            format!(
                "{:.0}% win rate against your {:.0}% overall, KDA {:.1}.",
                pool.win_rate * 100.0,
                reference_wr * 100.0,
                pool.avg_kda,
            ),
        ));

        if win_delta >= BASELINE_BAND {
            reasons.push(format!(
                "You win {:.0} points more often on this hero than you do overall.",
                win_delta * 100.0
            ));
        } else if win_delta <= -BASELINE_BAND {
            reasons.push(format!(
                "You win {:.0} points less often on this hero than you do overall.",
                -win_delta * 100.0
            ));
        }
    }

    // --- Meta strength ----------------------------------------------------
    match input.meta {
        Some(meta) => {
            parts.push(part(
                FitComponent::MetaStrength,
                meta.meta_strength,
                weights,
                format!(
                    "{:.0}% win rate across {} recorded picks{}.",
                    meta.win_rate * 100.0,
                    meta.picks,
                    meta.bracket
                        .map(|b| format!(" in {}", b.label()))
                        .unwrap_or_default(),
                ),
            ));

            if meta.meta_strength >= 65.0 {
                reasons.push("The hero is strong in the current meta.".to_string());
            } else if meta.meta_strength <= 35.0 {
                reasons.push("The hero is out of favour in the current meta.".to_string());
            }
        }
        None => caveats.push(
            "Meta data was unavailable, so this score is based on your own history only."
                .to_string(),
        ),
    }

    // --- Benchmark --------------------------------------------------------
    match input.benchmark_percentile {
        Some(value) => {
            parts.push(part(
                FitComponent::Benchmark,
                value,
                weights,
                format!("Around the {value:.0}th percentile of players on this hero."),
            ));
        }
        None if matches > 0 => caveats.push(
            "No peer comparison for this hero yet, so the benchmark is not in this score."
                .to_string(),
        ),
        None => {}
    }

    // --- Recent form ------------------------------------------------------
    if let Some((pool, recent_wr)) = input.pool.and_then(|p| p.recent_win_rate.map(|wr| (p, wr))) {
        // Five recent games is where this is taken at face value; below that
        // it is pulled toward neutral in proportion.
        let evidence = (pool.recent_matches as f32 / 5.0).clamp(0.0, 1.0);
        let raw = recent_wr * 100.0;
        let form = 50.0 + (raw - 50.0) * evidence;

        parts.push(part(
            FitComponent::RecentForm,
            form,
            weights,
            format!(
                "{} of your last {} on this hero.",
                (recent_wr * pool.recent_matches as f32).round() as i64,
                pool.recent_matches,
            ),
        ));

        if pool.recent_matches < 5 {
            caveats.push(format!(
                "Recent form rests on {} match{}.",
                pool.recent_matches,
                if pool.recent_matches == 1 { "" } else { "es" }
            ));
        }
    }

    let mut fit_score = weighted_total(&mut parts);

    // Training-focus compatibility, applied last and bounded: a modifier on a
    // finished score rather than an input to it.
    let focus_adjustment = input.focus_alignment.unwrap_or(0.0) * FOCUS_MODIFIER;
    if focus_adjustment.abs() >= 0.5 {
        let title = input.focus_title.unwrap_or("your current focus");
        reasons.push(if focus_adjustment > 0.0 {
            format!("Your figures on this hero are better than your own average for {title}.")
        } else {
            format!("Your figures on this hero are worse than your own average for {title}.")
        });
    }
    fit_score = (fit_score + focus_adjustment).clamp(0.0, 100.0);

    let level = level_for(fit_score, matches, input.pool, &mut caveats);

    if matches == 0 {
        caveats
            .push("No games on this hero yet, so your performance on it is unknown.".to_string());
    } else if matches < STRETCH_MATCHES {
        caveats.push(format!(
            "Only {matches} match{} of history — treat this as indicative.",
            if matches == 1 { "" } else { "es" }
        ));
    }

    HeroFit {
        hero_id: input.hero_id,
        hero_name: input.hero_name.to_string(),
        fit_score,
        level,
        level_label: level.label(),
        parts,
        reasons,
        caveats,
        matches,
        tier: input.pool.map(|p| p.tier),
        meta_strength: input.meta.map(|m| m.meta_strength),
        focus_adjustment,
    }
}

/// Normalize the present components' weights to sum to 1 and combine them.
///
/// The weights are rewritten in place so the reported share is the one that
/// actually produced the total, not the configured share of a component that
/// was dropped.
fn weighted_total(parts: &mut [FitPart]) -> f32 {
    let total_weight: f32 = parts.iter().map(|p| p.weight).sum();
    if total_weight <= 0.0 {
        return 0.0;
    }

    for part in parts.iter_mut() {
        part.weight /= total_weight;
    }

    parts
        .iter()
        .map(|p| p.score * p.weight)
        .sum::<f32>()
        .clamp(0.0, 100.0)
}

/// Turn a score into a recommendation, then apply the rules a score alone
/// cannot express.
///
/// Both guards exist because the spec forbids meta from overriding coaching
/// context: a hero the player has barely touched is never "Recommended"
/// however strong it currently is, and a hero they are actively losing on is
/// demoted whatever its history says.
fn level_for(
    fit_score: f32,
    matches: i64,
    pool: Option<&HeroPoolEntry>,
    caveats: &mut Vec<String>,
) -> RecommendationLevel {
    let mut level = if fit_score >= RECOMMEND_AT {
        RecommendationLevel::Recommended
    } else if fit_score >= CONSIDER_AT {
        RecommendationLevel::Consider
    } else {
        RecommendationLevel::AvoidForNow
    };

    if matches < percentile::MIN_SAMPLE && level == RecommendationLevel::Recommended {
        level = RecommendationLevel::Consider;
        caveats.push(
            "Held back from a full recommendation until you have more games on it.".to_string(),
        );
    }

    let poor_recent_form = pool.is_some_and(|p| {
        p.recent_matches >= 5
            && p.recent_win_rate
                .is_some_and(|wr| wr < POOR_RECENT_WIN_RATE)
    });
    if poor_recent_form {
        level = match level {
            RecommendationLevel::Recommended => RecommendationLevel::Consider,
            _ => RecommendationLevel::AvoidForNow,
        };
        caveats.push("Recent results on this hero have been poor.".to_string());
    }

    level
}

fn part(component: FitComponent, score: f32, weights: FitWeights, detail: String) -> FitPart {
    FitPart {
        component,
        label: component.label(),
        score: score.clamp(0.0, 100.0),
        weight: weights.for_component(component),
        detail,
    }
}

/// Order candidates for display: best fit first, then the hero the player
/// knows better, so two equal scores do not shuffle between requests.
pub fn rank(mut fits: Vec<HeroFit>) -> Vec<HeroFit> {
    fits.sort_by(|a, b| {
        b.fit_score
            .partial_cmp(&a.fit_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.matches.cmp(&a.matches))
            .then(a.hero_id.cmp(&b.hero_id))
    });
    fits
}

/// A one-line summary of the pool, for the dashboard.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct PoolSummary {
    pub heroes: i64,
    pub signature: i64,
    pub comfort: i64,
    pub stretch: i64,
    pub risk: i64,
    /// Heroes with enough matches for their figures to bear weight.
    pub established: i64,
}

pub fn summarize(pool: &[HeroPoolEntry]) -> PoolSummary {
    let count = |tier: HeroTier| pool.iter().filter(|e| e.tier == tier).count() as i64;

    PoolSummary {
        heroes: pool.len() as i64,
        signature: count(HeroTier::Signature),
        comfort: count(HeroTier::Comfort),
        stretch: count(HeroTier::Stretch),
        risk: count(HeroTier::Risk),
        established: pool
            .iter()
            .filter(|e| e.confidence != Confidence::Insufficient)
            .count() as i64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn row(hero_id: i32, matches: i64, wins: i64, recent: (i64, i64), kda: f32) -> HeroPoolRow {
        HeroPoolRow {
            hero_id,
            hero_name: format!("Hero {hero_id}"),
            role: "Carry".into(),
            matches,
            wins,
            recent_matches: recent.0,
            recent_wins: recent.1,
            avg_kda: kda,
            avg_gpm: 500.0,
            last_played_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        }
    }

    fn baseline() -> PlayerBaseline {
        PlayerBaseline {
            win_rate: Some(0.50),
            avg_kda: Some(3.0),
        }
    }

    fn meta(hero_id: i32, strength: f32) -> HeroMeta {
        HeroMeta {
            hero_id,
            hero_name: format!("Hero {hero_id}"),
            roles: vec!["Carry".into()],
            picks: 50_000,
            wins: 26_000,
            win_rate: 0.52,
            pick_rate: 0.03,
            trend: None,
            meta_strength: strength,
            bracket: None,
        }
    }

    fn pool_of(rows: &[HeroPoolRow]) -> Vec<HeroPoolEntry> {
        build_pool(rows, baseline())
    }

    fn fit(pool: Option<&HeroPoolEntry>, meta: Option<&HeroMeta>, bench: Option<f32>) -> HeroFit {
        let input = FitInput {
            hero_id: meta.map(|m| m.hero_id).or(pool.map(|p| p.hero_id)).unwrap(),
            hero_name: "Hero",
            pool,
            meta,
            benchmark_percentile: bench,
            focus_alignment: None,
            focus_title: None,
            baseline: baseline(),
        };
        score(&input, FitWeights::default())
    }

    // --- Classification ---------------------------------------------------

    #[test]
    fn a_thin_history_is_a_stretch_hero_however_it_went() {
        // 3-0 is not a signature hero, it is three games.
        let pool = pool_of(&[row(1, 3, 3, (3, 3), 6.0)]);
        assert_eq!(pool[0].tier, HeroTier::Stretch);
        assert_eq!(pool[0].confidence, Confidence::Insufficient);
    }

    #[test]
    fn deep_history_above_the_players_own_baseline_is_signature() {
        let pool = pool_of(&[row(1, 30, 20, (10, 7), 4.0)]);
        assert_eq!(pool[0].tier, HeroTier::Signature);
    }

    #[test]
    fn a_hero_around_the_baseline_is_comfort_not_signature() {
        let pool = pool_of(&[row(1, 30, 15, (10, 5), 3.0)]);
        assert_eq!(pool[0].tier, HeroTier::Comfort);
    }

    #[test]
    fn real_history_below_the_baseline_is_a_risk() {
        let pool = pool_of(&[row(1, 20, 6, (10, 3), 2.0)]);
        assert_eq!(pool[0].tier, HeroTier::Risk);
    }

    #[test]
    fn classification_is_relative_to_the_player_not_to_fifty_percent() {
        let struggling = PlayerBaseline {
            win_rate: Some(0.40),
            avg_kda: Some(2.0),
        };
        // 48% is below even, but well above this player's own baseline.
        let rows = [row(1, 20, 10, (10, 5), 3.0)];
        assert_eq!(build_pool(&rows, struggling)[0].tier, HeroTier::Signature);
        assert_eq!(build_pool(&rows, baseline())[0].tier, HeroTier::Comfort);
    }

    #[test]
    fn a_pool_summary_counts_every_tier() {
        let pool = pool_of(&[
            row(1, 30, 20, (10, 7), 4.0), // signature
            row(2, 20, 10, (10, 5), 3.0), // comfort
            row(3, 20, 6, (10, 3), 2.0),  // risk
            row(4, 2, 1, (2, 1), 3.0),    // stretch
        ]);
        let summary = summarize(&pool);

        assert_eq!(summary.heroes, 4);
        assert_eq!(summary.signature, 1);
        assert_eq!(summary.comfort, 1);
        assert_eq!(summary.risk, 1);
        assert_eq!(summary.stretch, 1);
        assert_eq!(summary.established, 3);
    }

    // --- Fit score --------------------------------------------------------

    #[test]
    fn a_strong_meta_hero_the_player_has_never_touched_is_not_recommended() {
        let strong = meta(13, 95.0);
        let result = fit(None, Some(&strong), None);

        assert_ne!(result.level, RecommendationLevel::Recommended);
        assert!(result
            .caveats
            .iter()
            .any(|c| c.contains("never played") || c.contains("No games on this hero")));
    }

    #[test]
    fn an_experienced_hero_can_outrank_a_stronger_meta_hero() {
        let pool = pool_of(&[row(1, 40, 26, (10, 7), 4.5)]);
        let weaker_meta = meta(1, 60.0);
        let stronger_meta = meta(13, 95.0);

        let known = fit(Some(&pool[0]), Some(&weaker_meta), Some(75.0));
        let unknown = fit(None, Some(&stronger_meta), None);

        assert!(
            known.fit_score > unknown.fit_score,
            "known {known:?} vs unknown {unknown:?}"
        );
        assert_eq!(known.level, RecommendationLevel::Recommended);
    }

    #[test]
    fn poor_recent_form_demotes_a_hero_with_good_history() {
        let good = pool_of(&[row(1, 40, 26, (10, 7), 4.5)]);
        let slumping = pool_of(&[row(1, 40, 26, (10, 2), 4.5)]);

        let steady = fit(Some(&good[0]), Some(&meta(1, 70.0)), Some(75.0));
        let slump = fit(Some(&slumping[0]), Some(&meta(1, 70.0)), Some(75.0));

        assert_eq!(steady.level, RecommendationLevel::Recommended);
        assert_ne!(slump.level, RecommendationLevel::Recommended);
        assert!(slump.caveats.iter().any(|c| c.contains("Recent results")));
    }

    #[test]
    fn a_missing_component_renormalizes_rather_than_scoring_zero() {
        let pool = pool_of(&[row(1, 40, 26, (10, 7), 4.5)]);

        let with_bench = fit(Some(&pool[0]), Some(&meta(1, 70.0)), Some(70.0));
        let without_bench = fit(Some(&pool[0]), Some(&meta(1, 70.0)), None);

        // Dropping a component the hero scored *at* its own average must not
        // move the total much, and must never crater it.
        assert!((with_bench.fit_score - without_bench.fit_score).abs() < 6.0);
        assert!(without_bench
            .caveats
            .iter()
            .any(|c| c.contains("No peer comparison")));
    }

    #[test]
    fn reported_weights_are_the_ones_that_produced_the_score() {
        let pool = pool_of(&[row(1, 40, 26, (10, 7), 4.5)]);
        let result = fit(Some(&pool[0]), None, None);

        let total: f32 = result.parts.iter().map(|p| p.weight).sum();
        assert!((total - 1.0).abs() < 0.001, "weights must sum to 1");
        assert!(!result
            .parts
            .iter()
            .any(|p| p.component == FitComponent::MetaStrength));

        let recomputed: f32 = result.parts.iter().map(|p| p.score * p.weight).sum();
        assert!((recomputed - result.fit_score).abs() < 0.01);
    }

    #[test]
    fn an_unavailable_meta_is_a_caveat_not_a_penalty() {
        let pool = pool_of(&[row(1, 40, 26, (10, 7), 4.5)]);
        let result = fit(Some(&pool[0]), None, Some(75.0));

        assert!(result.caveats.iter().any(|c| c.contains("Meta data")));
        assert!(result.fit_score > CONSIDER_AT);
    }

    #[test]
    fn every_score_stays_inside_the_scale() {
        let extremes = pool_of(&[
            row(1, 200, 200, (10, 10), 50.0),
            row(2, 200, 0, (10, 0), 0.0),
        ]);

        for entry in &extremes {
            for strength in [0.0, 50.0, 100.0] {
                let result = fit(
                    Some(entry),
                    Some(&meta(entry.hero_id, strength)),
                    Some(100.0),
                );
                assert!((0.0..=100.0).contains(&result.fit_score), "{result:?}");
            }
        }
    }

    #[test]
    fn ranking_is_stable_for_equal_scores() {
        let a = fit(None, Some(&meta(2, 50.0)), None);
        let b = fit(None, Some(&meta(1, 50.0)), None);

        let ranked = rank(vec![a, b]);
        // Equal fit, equal experience: the lower hero id wins the tie, every
        // time, rather than depending on input order.
        assert_eq!(ranked[0].hero_id, 1);
    }

    #[test]
    fn the_score_explains_itself() {
        let pool = pool_of(&[row(1, 40, 26, (10, 7), 4.5)]);
        let result = fit(Some(&pool[0]), Some(&meta(1, 80.0)), Some(75.0));

        assert_eq!(result.parts.len(), 5, "every component was knowable");
        assert!(result.parts.iter().all(|p| !p.detail.is_empty()));
        assert!(!result.reasons.is_empty());
    }
}
