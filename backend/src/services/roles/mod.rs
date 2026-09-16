//! Role performance, and the advisory pick that follows from it.
//!
//! Pure: the repository has already restricted the window to eligible matches
//! and summed them per stored role. What happens here is folding those sums
//! onto the five coachable roles, scoring each one, and saying which is
//! strongest — all of it arithmetic over measured figures, none of it a model
//! call.
//!
//! # The performance score
//!
//! No such score existed before this module, so it is defined here rather than
//! borrowed. Four measures, each mapped onto 0-100 and combined by configured
//! weights:
//!
//! | Measure                | Default weight | 0 means          | 100 means        |
//! |------------------------|---------------:|------------------|------------------|
//! | Win rate               |           0.45 | never wins       | always wins      |
//! | Kill participation     |           0.20 | never involved   | every team kill  |
//! | KDA                    |           0.20 | 0.0              | 6.0 or better    |
//! | Deaths per 10 minutes  |           0.15 | 3.0 or worse     | none             |
//!
//! Every measure is role-neutral on purpose. Gold per minute is the obvious
//! candidate and the wrong one: comparing a hard support's economy with a
//! carry's would recommend the safe lane to everybody, which is advice about
//! Dota rather than about the player. The averages the score does *not* use are
//! still reported alongside it, because they are what a coach reads once a role
//! has been chosen.
//!
//! A measure the player has no data for — kill participation needs team totals
//! the provider does not always supply — is dropped and the remaining weights
//! renormalized, rather than counted as zero.
//!
//! # Sample size
//!
//! The raw score is then pulled toward the neutral midpoint in proportion to
//! how little evidence stands behind it:
//!
//! ```text
//! performance = 50 + (raw - 50) × n / (n + K)      K = 10
//! ```
//!
//! At n = 5 a raw 85 reports as 62; at n = 40 a raw 70 reports as 66. The
//! unadjusted figure travels alongside as `raw_performance`, so the adjustment
//! is visible rather than hidden inside the number.
//!
//! Shrinkage alone is not enough to keep a hot streak from winning — a role
//! with five straight wins scores high enough that the adjustment only narrows
//! the gap. So the recommendation also refuses to consider any role below
//! [`MIN_RECOMMENDABLE_MATCHES`], which is the rule that actually delivers
//! "five excellent games must not outweigh forty consistent ones". The two work
//! together: the floor decides what may be recommended, the shrinkage decides
//! the order among what survives.
//!
//! A role under the floor is still *reported*, with its score and its sample.
//! The player may pick it — the recommendation is advice, not a gate.

use std::collections::BTreeMap;

use crate::domain::role::{
    CoachableRole, RoleAnalysis, RolePerformance, RoleRecommendation, RoleScoreWeights, RoleTotals,
    ScoreComponent, ScoreComponentKey,
};
use crate::domain::scope::SampleConfidence;

/// Midpoint a thin sample is pulled toward: "no information" is average, not
/// zero.
const NEUTRAL_SCORE: f32 = 50.0;
/// The `K` above. Ten matches is where a role's reading starts to carry half
/// its own weight.
pub const SHRINKAGE_MATCHES: f32 = 10.0;
/// KDA that scores full marks on that component.
const KDA_REFERENCE: f32 = 6.0;
/// Deaths per ten minutes that scores zero on that component.
const DEATHS_PER_10_FLOOR: f32 = 3.0;
/// Below this many eligible matches in a role, it is not recommended.
///
/// Ten rather than five, and the difference matters: at five matches a perfect
/// run scores high enough to survive the sample-size adjustment and win, which
/// is exactly the outcome the product forbids. Ten is also where
/// [`SHRINKAGE_MATCHES`] has the role carrying half its own weight, so the two
/// thresholds mean the same thing about the same evidence.
pub const MIN_RECOMMENDABLE_MATCHES: i64 = 10;

/// Fold per-label sums onto the coachable roles and score each one.
///
/// `totals` are the rows the repository produced for one scope; anything whose
/// label does not map to a coachable role is counted as unclassified rather
/// than dropped, so the numbers add up to the window the caller asked for.
pub fn analyze(totals: &[RoleTotals], weights: RoleScoreWeights) -> RoleAnalysis {
    let mut merged: BTreeMap<u8, Merged> = BTreeMap::new();
    let mut unclassified = 0;

    for row in totals {
        match CoachableRole::from_stored(&row.role) {
            Some(role) => merged
                .entry(role.position())
                .or_insert_with(|| Merged::new(role))
                .absorb(row),
            None => unclassified += row.matches,
        }
    }

    let mut roles: Vec<RolePerformance> = merged
        .into_values()
        .map(|m| m.into_performance(weights))
        .collect();

    // Strongest first, and stable: equal scores fall back to the larger sample
    // and then to position order, so the list never reshuffles between reads.
    roles.sort_by(|a, b| {
        b.performance
            .partial_cmp(&a.performance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.matches.cmp(&a.matches))
            .then_with(|| a.position.cmp(&b.position))
    });

    let analyzed: i64 = roles.iter().map(|r| r.matches).sum::<i64>() + unclassified;
    let confidence = SampleConfidence::for_matches(analyzed);
    let recommendation = recommend(&roles);

    let note = match (&recommendation, analyzed, unclassified) {
        (Some(_), _, _) => None,
        (None, 0, _) => {
            Some("No eligible Ranked or public All Pick matches have been synced yet.".to_string())
        }
        (None, _, unclassified) if unclassified > 0 && roles.is_empty() => Some(format!(
            "None of your {unclassified} eligible {} could be attributed to a specific role — \
             without a parsed replay the data only shows whether you played a core or a support.",
            plural(unclassified, "match", "matches"),
        )),
        (None, _, _) => Some(format!(
            "No role has {MIN_RECOMMENDABLE_MATCHES} eligible matches yet, so there is nothing \
             solid enough to recommend. Pick the role you want to work on.",
        )),
    };

    RoleAnalysis {
        analyzed_matches: analyzed,
        confidence,
        confidence_label: confidence.label(),
        confidence_caveat: confidence.caveat(),
        roles,
        unclassified_matches: unclassified,
        min_recommendable_matches: MIN_RECOMMENDABLE_MATCHES,
        recommendation,
        note,
    }
}

/// The advisory pick.
///
/// Highest score among the roles with enough matches to justify one. The score
/// already carries the sample-size adjustment, so a thin role cannot win on a
/// hot streak; the hard floor on top of it is there because a role with four
/// games should not be *offered* at all, however it scored.
pub fn recommend(roles: &[RolePerformance]) -> Option<RoleRecommendation> {
    let mut eligible = roles
        .iter()
        .filter(|r| r.matches >= MIN_RECOMMENDABLE_MATCHES);

    // `roles` is already sorted strongest first, so the first survivor is the
    // pick and the next is the runner-up.
    let best = eligible.next()?;
    let runner_up = eligible.next();

    let why = match runner_up {
        Some(second) => format!(
            "Across your last {} eligible {} as {}, you score {:.0}/100 — ahead of {} at {:.0}/100 \
             over {} {}. You win {:.0}% of your {} games.",
            best.matches,
            plural(best.matches, "match", "matches"),
            best.role_label,
            best.performance,
            second.role_label,
            second.performance,
            second.matches,
            plural(second.matches, "match", "matches"),
            best.win_rate * 100.0,
            best.role_label,
        ),
        None => format!(
            "{} is the only role with enough eligible matches to judge: {} {}, {:.0}% won, \
             scoring {:.0}/100.",
            best.role_label,
            best.matches,
            plural(best.matches, "match", "matches"),
            best.win_rate * 100.0,
            best.performance,
        ),
    };

    Some(RoleRecommendation {
        role: best.role,
        role_label: best.role_label,
        why,
        runner_up: runner_up.map(|r| r.role),
        confidence: best.confidence,
    })
}

/// Sums for one coachable role, accumulated across every stored label that
/// belongs to it.
struct Merged {
    role: CoachableRole,
    matches: i64,
    wins: i64,
    kda: Sum,
    gpm: Sum,
    xpm: Sum,
    last_hits_per_min: Sum,
    deaths_per_10: Sum,
    kill_participation: Sum,
}

/// A running total and the number of matches behind it.
///
/// The count is per-measure rather than per-role because an optional input —
/// kill participation — is present in some matches and not others, and dividing
/// its sum by the role's match count would quietly understate it.
#[derive(Default, Clone, Copy)]
struct Sum {
    total: f64,
    count: i64,
}

impl Sum {
    fn add(&mut self, value: Option<f64>, matches: i64) {
        if let Some(value) = value {
            self.total += value;
            self.count += matches;
        }
    }

    /// Explicitly counted matches instead of the role's total, for a measure
    /// that reports its own sample.
    fn add_counted(&mut self, value: Option<f64>, count: i64) {
        if let Some(value) = value {
            self.total += value;
            self.count += count;
        }
    }

    fn average(self) -> Option<f32> {
        (self.count > 0).then(|| (self.total / self.count as f64) as f32)
    }
}

impl Merged {
    fn new(role: CoachableRole) -> Self {
        Self {
            role,
            matches: 0,
            wins: 0,
            kda: Sum::default(),
            gpm: Sum::default(),
            xpm: Sum::default(),
            last_hits_per_min: Sum::default(),
            deaths_per_10: Sum::default(),
            kill_participation: Sum::default(),
        }
    }

    fn absorb(&mut self, row: &RoleTotals) {
        self.matches += row.matches;
        self.wins += row.wins;
        self.kda.add(row.kda_sum, row.matches);
        self.gpm.add(row.gpm_sum, row.matches);
        self.xpm.add(row.xpm_sum, row.matches);
        self.last_hits_per_min
            .add(row.last_hits_per_min_sum, row.matches);
        self.deaths_per_10.add(row.deaths_per_10_sum, row.matches);
        self.kill_participation
            .add_counted(row.kill_participation_sum, row.kill_participation_matches);
    }

    fn into_performance(self, weights: RoleScoreWeights) -> RolePerformance {
        let win_rate = if self.matches > 0 {
            self.wins as f32 / self.matches as f32
        } else {
            0.0
        };

        let avg_kda = self.kda.average();
        let avg_deaths = self.deaths_per_10.average();
        let avg_kp = self.kill_participation.average();

        let measured = [
            (
                ScoreComponentKey::WinRate,
                Some(win_rate),
                (win_rate * 100.0).clamp(0.0, 100.0),
                self.matches,
            ),
            (
                ScoreComponentKey::KillParticipation,
                avg_kp,
                avg_kp.map_or(0.0, |kp| (kp * 100.0).clamp(0.0, 100.0)),
                self.kill_participation.count,
            ),
            (
                ScoreComponentKey::Kda,
                avg_kda,
                avg_kda.map_or(0.0, |kda| (kda / KDA_REFERENCE * 100.0).clamp(0.0, 100.0)),
                self.kda.count,
            ),
            (
                ScoreComponentKey::Deaths,
                avg_deaths,
                avg_deaths.map_or(0.0, |deaths| {
                    (100.0 - deaths / DEATHS_PER_10_FLOOR * 100.0).clamp(0.0, 100.0)
                }),
                self.deaths_per_10.count,
            ),
        ];

        // Renormalize around whatever could actually be measured, so a missing
        // input costs the player nothing.
        let total_weight: f32 = measured
            .iter()
            .filter(|(_, value, _, _)| value.is_some())
            .map(|(key, _, _, _)| weights.for_component(*key))
            .sum();

        let components: Vec<ScoreComponent> = measured
            .iter()
            .filter_map(|(key, value, normalized, sample)| {
                let value = (*value)?;
                let weight = weights.for_component(*key);
                (total_weight > 0.0 && weight > 0.0).then(|| ScoreComponent {
                    key: *key,
                    label: key.label(),
                    value,
                    normalized: *normalized,
                    weight: weight / total_weight,
                    sample: *sample,
                })
            })
            .collect();

        let raw: f32 = components
            .iter()
            .map(|c| c.normalized * c.weight)
            .sum::<f32>()
            .clamp(0.0, 100.0);

        RolePerformance {
            role: self.role,
            role_label: self.role.label(),
            position: self.role.position(),
            matches: self.matches,
            wins: self.wins,
            losses: self.matches - self.wins,
            win_rate,
            avg_kda,
            avg_gpm: self.gpm.average(),
            avg_xpm: self.xpm.average(),
            avg_last_hits_per_min: self.last_hits_per_min.average(),
            avg_deaths_per_10: avg_deaths,
            avg_kill_participation: avg_kp,
            kill_participation_sample: self.kill_participation.count,
            performance: shrink(raw, self.matches),
            raw_performance: raw,
            confidence: SampleConfidence::for_matches(self.matches),
            components,
        }
    }
}

/// Pull a score toward the neutral midpoint in proportion to how thin the
/// sample behind it is.
pub fn shrink(raw: f32, matches: i64) -> f32 {
    let n = matches.max(0) as f32;
    NEUTRAL_SCORE + (raw - NEUTRAL_SCORE) * (n / (n + SHRINKAGE_MATCHES))
}

fn plural(count: i64, one: &str, many: &str) -> String {
    if count == 1 { one } else { many }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn totals(role: &str, matches: i64, wins: i64, kda: f64, deaths: f64) -> RoleTotals {
        RoleTotals {
            role: role.into(),
            matches,
            wins,
            kda_sum: Some(kda * matches as f64),
            gpm_sum: Some(450.0 * matches as f64),
            xpm_sum: Some(520.0 * matches as f64),
            last_hits_per_min_sum: Some(4.0 * matches as f64),
            deaths_per_10_sum: Some(deaths * matches as f64),
            kill_participation_sum: Some(0.6 * matches as f64),
            kill_participation_matches: matches,
        }
    }

    fn weights() -> RoleScoreWeights {
        RoleScoreWeights::default()
    }

    #[test]
    fn stored_labels_are_folded_onto_the_five_coachable_roles() {
        let analysis = analyze(
            &[
                totals("Carry", 10, 5, 3.0, 1.5),
                totals("Support", 8, 4, 3.0, 1.5),
                totals("Hard Support", 6, 3, 3.0, 1.5),
            ],
            weights(),
        );

        let labels: Vec<&str> = analysis.roles.iter().map(|r| r.role_label).collect();
        assert!(labels.contains(&"Carry"));
        assert!(labels.contains(&"Soft Support"));
        assert!(labels.contains(&"Hard Support"));
        assert_eq!(analysis.analyzed_matches, 24);
        assert_eq!(analysis.unclassified_matches, 0);
    }

    #[test]
    fn unattributable_matches_are_counted_not_folded_into_a_lane() {
        let analysis = analyze(
            &[
                totals("Carry", 10, 6, 3.0, 1.5),
                // An unparsed replay: farm priority says core, nothing says which.
                totals("Core", 25, 12, 3.0, 1.5),
                totals("Unknown", 5, 2, 3.0, 1.5),
            ],
            weights(),
        );

        assert_eq!(analysis.unclassified_matches, 30);
        assert_eq!(analysis.analyzed_matches, 40);
        assert_eq!(analysis.roles.len(), 1, "only Carry is attributable");
        assert_eq!(analysis.roles[0].matches, 10);
    }

    #[test]
    fn a_thin_excellent_role_does_not_outrank_a_large_consistent_one() {
        // The spec's example: five perfect Carry games against forty solid
        // Support ones.
        let analysis = analyze(
            &[
                totals("Carry", 5, 5, 8.0, 0.5),
                totals("Support", 40, 26, 4.0, 1.4),
            ],
            weights(),
        );

        assert_eq!(
            analysis.recommendation.as_ref().unwrap().role,
            CoachableRole::SoftSupport,
            "40 consistent games must outweigh 5 hot ones",
        );

        let carry = analysis
            .roles
            .iter()
            .find(|r| r.role == CoachableRole::Carry)
            .unwrap();
        assert!(
            carry.raw_performance > carry.performance,
            "the thin sample must be visibly adjusted, not silently so",
        );
        // Still shown, with its real numbers: the player may choose it anyway.
        assert_eq!(carry.matches, 5);
        assert_eq!(carry.win_rate, 1.0);
    }

    #[test]
    fn shrinkage_alone_would_not_have_been_enough() {
        // Documents why the floor exists rather than only the adjustment: on
        // score alone, five perfect games still outrank forty good ones.
        let analysis = analyze(
            &[
                totals("Carry", 5, 5, 8.0, 0.5),
                totals("Support", 40, 26, 4.0, 1.4),
            ],
            weights(),
        );

        let carry = analysis
            .roles
            .iter()
            .find(|r| r.role == CoachableRole::Carry)
            .unwrap();
        let support = analysis
            .roles
            .iter()
            .find(|r| r.role == CoachableRole::SoftSupport)
            .unwrap();

        assert!(carry.performance > support.performance);
        assert!(carry.matches < MIN_RECOMMENDABLE_MATCHES);
    }

    #[test]
    fn shrinkage_follows_the_documented_formula() {
        // n / (n + 10) of the distance from 50.
        assert!((shrink(85.0, 5) - (50.0 + 35.0 * 5.0 / 15.0)).abs() < 0.01);
        assert!((shrink(70.0, 40) - (50.0 + 20.0 * 40.0 / 50.0)).abs() < 0.01);
        // No matches means no information, which is the midpoint.
        assert_eq!(shrink(90.0, 0), NEUTRAL_SCORE);
    }

    #[test]
    fn a_role_below_the_floor_is_not_recommended() {
        let analysis = analyze(&[totals("Carry", 4, 4, 9.0, 0.2)], weights());

        assert!(
            analysis.recommendation.is_none(),
            "four games is not a recommendation",
        );
        assert!(analysis.note.as_deref().unwrap().contains("nothing"));
        // The role is still reported — the player may pick it anyway.
        assert_eq!(analysis.roles.len(), 1);
    }

    #[test]
    fn a_missing_measure_is_dropped_and_the_rest_renormalized() {
        let mut row = totals("Mid", 20, 12, 3.0, 1.5);
        row.kill_participation_sum = None;
        row.kill_participation_matches = 0;

        let analysis = analyze(&[row], weights());
        let mid = &analysis.roles[0];

        assert_eq!(mid.avg_kill_participation, None);
        assert_eq!(mid.components.len(), 3, "kill participation drops out");
        let total: f32 = mid.components.iter().map(|c| c.weight).sum();
        assert!(
            (total - 1.0).abs() < 0.001,
            "remaining weights must renormalize, got {total}",
        );
    }

    #[test]
    fn the_score_is_built_only_from_role_neutral_measures() {
        // Identical on every scored measure, wildly different economies.
        let mut farming = totals("Carry", 20, 12, 3.0, 1.5);
        farming.gpm_sum = Some(700.0 * 20.0);
        let mut poor = totals("Hard Support", 20, 12, 3.0, 1.5);
        poor.gpm_sum = Some(250.0 * 20.0);

        let analysis = analyze(&[farming, poor], weights());
        let carry = analysis.roles.iter().find(|r| r.position == 1).unwrap();
        let support = analysis.roles.iter().find(|r| r.position == 5).unwrap();

        assert!(
            (carry.performance - support.performance).abs() < 0.001,
            "gold per minute must not decide which role a player is better at",
        );
        // But it is still reported, because a coach needs it once a role is chosen.
        assert_eq!(carry.avg_gpm.unwrap().round(), 700.0);
        assert_eq!(support.avg_gpm.unwrap().round(), 250.0);
    }

    #[test]
    fn the_strongest_role_leads_the_list_and_carries_the_recommendation() {
        let analysis = analyze(
            &[
                totals("Carry", 20, 8, 2.0, 2.2),
                totals("Mid", 20, 14, 5.0, 1.0),
            ],
            weights(),
        );

        assert_eq!(analysis.roles[0].role, CoachableRole::Mid);
        let pick = analysis.recommendation.unwrap();
        assert_eq!(pick.role, CoachableRole::Mid);
        assert_eq!(pick.runner_up, Some(CoachableRole::Carry));
        assert!(pick.why.contains("Mid"), "{}", pick.why);
        assert!(pick.why.contains("Carry"), "{}", pick.why);
    }

    #[test]
    fn an_empty_history_recommends_nothing_and_says_why() {
        let analysis = analyze(&[], weights());

        assert_eq!(analysis.analyzed_matches, 0);
        assert!(analysis.recommendation.is_none());
        assert_eq!(analysis.confidence, SampleConfidence::Limited);
        assert!(analysis.note.as_deref().unwrap().contains("All Pick"));
    }

    #[test]
    fn a_history_that_is_entirely_unattributable_says_so_specifically() {
        let analysis = analyze(&[totals("Core", 30, 15, 3.0, 1.5)], weights());

        assert!(analysis.roles.is_empty());
        assert_eq!(analysis.unclassified_matches, 30);
        assert!(
            analysis.note.as_deref().unwrap().contains("parsed replay"),
            "{:?}",
            analysis.note,
        );
    }
}
