//! Recurring pattern detection.
//!
//! Each detector answers one question per match with three possible answers:
//! yes, no, or *not measurable here*. The third is the one that makes this
//! honest — last hits at ten minutes only exist on a parsed replay, so a
//! detector that read a missing value as "fine" would quietly report a clean
//! laning phase for a player nobody ever measured.
//!
//! A pattern is only reported when it clears three separate bars:
//!
//!   - measurable in at least [`MIN_MEASURED`] matches,
//!   - occurring in at least [`MIN_OCCURRENCES`] of them,
//!   - at a rate of at least [`MIN_RATE`].
//!
//! Two of those would not be enough on their own: 3 out of 3 is a rate of
//! 100% and no evidence at all, and 8 out of 40 is a solid sample of something
//! that is not a habit.
//!
//! Pure: no database, no clock. The caller supplies the history.

use std::collections::HashMap;

use crate::domain::player_model::{AnalyzedMatch, PatternStatus, RecurringPattern};
use crate::services::benchmarks::percentile;

/// Matches a detector must be able to check before it may report anything.
pub const MIN_MEASURED: i64 = 8;
/// And the condition must have actually happened this often.
pub const MIN_OCCURRENCES: i64 = 3;
/// And in at least this share of the measurable matches.
pub const MIN_RATE: f32 = 0.4;

/// The recent window used to tell "still happening" from "used to happen".
const RECENT_WINDOW: usize = 10;
/// The recent rate must fall this far below the overall rate before a pattern
/// is called improving rather than active.
const IMPROVEMENT_MARGIN: f32 = 0.2;

/// Figures derived from the player's own history, for detectors whose question
/// is about consistency rather than an absolute standard.
pub struct Baselines {
    pub median_gpm: Option<f32>,
    pub median_gold_at_10: Option<f32>,
}

impl Baselines {
    pub fn from(matches: &[AnalyzedMatch]) -> Self {
        Self {
            median_gpm: median(matches.iter().map(|m| m.gpm as f32).collect()),
            median_gold_at_10: median(
                matches
                    .iter()
                    .filter_map(|m| m.gold_at_10.map(|g| g as f32))
                    .collect(),
            ),
        }
    }
}

/// One thing worth noticing across many matches.
pub struct Detector {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// `None` when this match cannot answer the question.
    pub measure: fn(&AnalyzedMatch, &Baselines) -> Option<bool>,
}

/// Absolute thresholds, documented rather than tuned.
///
/// These are deliberately coarse: they are the line between "worth mentioning"
/// and "not worth mentioning", not a skill rating. Anything finer belongs to
/// the benchmark engine, which compares against real players.
mod thresholds {
    /// Deaths per 10 minutes above which a match is counted as a death-heavy
    /// game. Roughly a death every four minutes.
    pub const DEATHS_PER_10: f32 = 3.0;
    /// Share of team kills below which the player was largely absent.
    pub const KILL_PARTICIPATION: f32 = 0.55;
    /// And the tighter line used when pairing low participation with good farm.
    pub const ABSENT_FROM_FIGHTS: f32 = 0.5;
    /// Last hits at 10 minutes below which a core had a poor laning stage.
    pub const LAST_HITS_AT_10: i32 = 40;
    /// Seconds. A Black King Bar completed after this is a late one.
    pub const LATE_BKB_SECONDS: i32 = 1_800;
    /// Total tower damage below which a core contributed nothing to objectives.
    pub const TOWER_DAMAGE: i32 = 500;
    /// Matches shorter than this are excluded from the objective check: a
    /// twenty-minute game ends before most towers are contestable.
    pub const OBJECTIVE_MIN_MINUTES: f32 = 25.0;
}

pub const DETECTORS: &[Detector] = &[
    Detector {
        id: "high_death_rate",
        label: "Dies too often",
        description: "Matches where you died more than three times per 10 minutes.",
        measure: |m, _| Some(m.deaths_per_10 > thresholds::DEATHS_PER_10),
    },
    Detector {
        id: "low_kill_participation",
        label: "Low teamfight participation",
        description: "Matches where you took part in under 55% of your team's kills.",
        // Needs team totals, which only arrive with the full match detail.
        measure: |m, _| {
            m.kill_participation
                .map(|kp| kp < thresholds::KILL_PARTICIPATION)
        },
    },
    Detector {
        id: "farms_but_does_not_fight",
        label: "Farms well, fights rarely",
        description:
            "Matches where your farm beat your own median but you were absent from fights.",
        measure: |m, baselines| {
            // Both halves must be knowable, or the pairing means nothing.
            let kp = m.kill_participation?;
            let median = baselines.median_gpm?;
            Some(m.gpm as f32 >= median && kp < thresholds::ABSENT_FROM_FIGHTS)
        },
    },
    Detector {
        id: "low_cs_at_10",
        label: "Weak laning stage",
        description: "Core matches where you had under 40 last hits at 10 minutes.",
        measure: |m, _| {
            if !m.is_core() {
                // Not a failing for a support; the question does not apply.
                return None;
            }
            m.last_hits_at_10.map(|lh| lh < thresholds::LAST_HITS_AT_10)
        },
    },
    Detector {
        id: "loses_a_won_lane",
        label: "Loses a won lane",
        description:
            "Matches you lost after coming out of the laning stage ahead of your own median.",
        measure: |m, baselines| {
            let gold = m.gold_at_10? as f32;
            let median = baselines.median_gold_at_10?;
            // Only ask the question about games that started well.
            (gold >= median).then_some(!m.won)
        },
    },
    Detector {
        id: "late_bkb",
        label: "Late Black King Bar",
        description: "Matches where your Black King Bar was finished after 30 minutes.",
        measure: |m, _| {
            m.bkb_seconds
                .map(|seconds| seconds > thresholds::LATE_BKB_SECONDS)
        },
    },
    Detector {
        id: "low_objective_damage",
        label: "Little objective damage",
        description: "Long core matches where you did almost no tower damage.",
        measure: |m, _| {
            if !m.is_core() || m.minutes() < thresholds::OBJECTIVE_MIN_MINUTES {
                return None;
            }
            m.tower_damage
                .map(|damage| damage < thresholds::TOWER_DAMAGE)
        },
    },
];

/// Run every detector over a player's history.
///
/// Returns only what cleared the thresholds, worst rate first. A detector with
/// a thin sample produces nothing at all rather than a low-confidence pattern:
/// the spec asks for evidence, and "measurable twice" is not evidence.
pub fn detect(matches: &[AnalyzedMatch]) -> Vec<RecurringPattern> {
    let baselines = Baselines::from(matches);

    let mut found: Vec<RecurringPattern> = DETECTORS
        .iter()
        .filter_map(|detector| evaluate(detector, matches, &baselines))
        .collect();

    found.sort_by(|a, b| {
        b.rate
            .partial_cmp(&a.rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.occurrences.cmp(&a.occurrences))
            .then(a.id.cmp(&b.id))
    });
    found
}

/// Apply one detector, newest match first.
fn evaluate(
    detector: &Detector,
    matches: &[AnalyzedMatch],
    baselines: &Baselines,
) -> Option<RecurringPattern> {
    // Measurable matches, newest first, so "recent" means recent.
    let mut measurable: Vec<(&AnalyzedMatch, bool)> = matches
        .iter()
        .filter_map(|m| (detector.measure)(m, baselines).map(|hit| (m, hit)))
        .collect();
    measurable.sort_by_key(|(m, _)| std::cmp::Reverse(m.started_at));

    let measured = measurable.len() as i64;
    let hits: Vec<&AnalyzedMatch> = measurable
        .iter()
        .filter(|(_, hit)| *hit)
        .map(|(m, _)| *m)
        .collect();
    let occurrences = hits.len() as i64;

    if measured < MIN_MEASURED || occurrences < MIN_OCCURRENCES {
        return None;
    }

    let rate = occurrences as f32 / measured as f32;
    if rate < MIN_RATE {
        return None;
    }

    let recent: Vec<&(&AnalyzedMatch, bool)> = measurable.iter().take(RECENT_WINDOW).collect();
    let recent_measured = recent.len() as i64;
    // Only worth reporting when the window is nearly full; three of the last
    // three is not a trend either.
    let recent_rate = (recent_measured >= MIN_MEASURED)
        .then(|| recent.iter().filter(|(_, hit)| *hit).count() as f32 / recent_measured as f32);

    let status = match recent_rate {
        Some(recent) if recent + IMPROVEMENT_MARGIN <= rate => PatternStatus::Improving,
        _ => PatternStatus::Active,
    };

    Some(RecurringPattern {
        id: detector.id.to_string(),
        label: detector.label.to_string(),
        description: detector.description.to_string(),
        occurrences,
        measured,
        rate,
        recent_rate,
        recent_measured,
        status,
        status_label: status.label().to_string(),
        confidence: percentile::confidence_for(measured),
        statement: statement(detector, occurrences, measured, rate, recent_rate, status),
        examples: hits.iter().take(3).map(|m| m.match_id).collect(),
        first_seen_at: hits.last().map(|m| m.started_at),
        last_seen_at: hits.first().map(|m| m.started_at),
        // Filled by the repository, which knows when this was first stored.
        first_detected_at: None,
    })
}

/// The sentence the coach and the UI both show.
///
/// Always states both numbers. "You die too much" is an opinion; "17 of the 30
/// matches we could check" is a fact with a denominator.
fn statement(
    detector: &Detector,
    occurrences: i64,
    measured: i64,
    rate: f32,
    recent_rate: Option<f32>,
    status: PatternStatus,
) -> String {
    let base = format!(
        "{} in {} of the {} {} this could be measured in ({:.0}%).",
        detector.label,
        occurrences,
        measured,
        if measured == 1 { "match" } else { "matches" },
        rate * 100.0,
    );

    match (status, recent_rate) {
        (PatternStatus::Improving, Some(recent)) => format!(
            "{base} It is becoming rarer: {:.0}% across your most recent measurable matches.",
            recent * 100.0
        ),
        (_, Some(recent)) => format!("{base} Recently: {:.0}%.", recent * 100.0),
        _ => base,
    }
}

/// Middle value, or the mean of the middle two. `None` for an empty set.
fn median(mut values: Vec<f32>) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    })
}

/// Detectors that could not report, and why.
///
/// Shown to the user rather than hidden: "we cannot check your laning stage
/// because your replays are not parsed" is useful, and its absence would read
/// as "your laning stage is fine".
pub fn unmeasurable(matches: &[AnalyzedMatch]) -> HashMap<&'static str, i64> {
    let baselines = Baselines::from(matches);

    DETECTORS
        .iter()
        .map(|detector| {
            let measured = matches
                .iter()
                .filter(|m| (detector.measure)(m, &baselines).is_some())
                .count() as i64;
            (detector.id, measured)
        })
        .filter(|(_, measured)| *measured < MIN_MEASURED)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::benchmark::Confidence;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn match_at(index: i64) -> AnalyzedMatch {
        AnalyzedMatch {
            match_id: Uuid::new_v4(),
            // Ascending, so a higher index is a more recent match.
            started_at: Utc.timestamp_opt(1_700_000_000 + index * 3_600, 0).unwrap(),
            hero_id: 35,
            hero_name: "Luna".into(),
            role: "Carry".into(),
            won: true,
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

    fn history(count: i64) -> Vec<AnalyzedMatch> {
        (0..count).map(match_at).collect()
    }

    fn find<'a>(patterns: &'a [RecurringPattern], id: &str) -> Option<&'a RecurringPattern> {
        patterns.iter().find(|p| p.id == id)
    }

    #[test]
    fn a_habit_across_a_real_sample_is_reported() {
        let mut matches = history(20);
        for m in matches.iter_mut().take(12) {
            m.deaths_per_10 = 4.0;
        }

        let deaths = find(&detect(&matches), "high_death_rate").unwrap().clone();

        assert_eq!(deaths.occurrences, 12);
        assert_eq!(deaths.measured, 20);
        assert!((deaths.rate - 0.6).abs() < 0.01);
        assert_eq!(deaths.confidence, Confidence::Adequate);
        assert!(deaths.statement.contains("12 of the 20 matches"));
    }

    #[test]
    fn one_bad_match_is_never_a_pattern() {
        let mut matches = history(20);
        matches[0].deaths_per_10 = 9.0;

        assert!(find(&detect(&matches), "high_death_rate").is_none());
    }

    #[test]
    fn a_perfect_rate_over_a_thin_sample_is_not_a_pattern() {
        // Three for three is a 100% rate and no evidence.
        let mut matches = history(3);
        for m in matches.iter_mut() {
            m.deaths_per_10 = 5.0;
        }

        assert!(detect(&matches).is_empty());
    }

    #[test]
    fn a_solid_sample_below_the_rate_floor_is_not_a_pattern() {
        // Measurable 40 times, happened 8: real, but not a habit.
        let mut matches = history(40);
        for m in matches.iter_mut().take(8) {
            m.deaths_per_10 = 5.0;
        }

        assert!(find(&detect(&matches), "high_death_rate").is_none());
    }

    #[test]
    fn an_unmeasurable_signal_is_not_read_as_a_passing_one() {
        // Nothing is parsed, so the laning detector cannot speak.
        let matches = history(30);
        let patterns = detect(&matches);

        assert!(find(&patterns, "low_cs_at_10").is_none());
        assert!(unmeasurable(&matches).contains_key("low_cs_at_10"));
    }

    #[test]
    fn the_denominator_is_what_could_be_checked_not_the_career() {
        let mut matches = history(40);
        // Only ten matches carry a parsed laning stage; nine of them are poor.
        for m in matches.iter_mut().take(10) {
            m.last_hits_at_10 = Some(20);
        }
        matches[0].last_hits_at_10 = Some(80);

        let laning = find(&detect(&matches), "low_cs_at_10").unwrap().clone();

        assert_eq!(laning.measured, 10, "not 40");
        assert_eq!(laning.occurrences, 9);
        assert!(laning.statement.contains("9 of the 10 matches"));
    }

    #[test]
    fn a_pattern_the_player_is_fixing_is_reported_as_improving() {
        let mut matches = history(30);
        // The oldest twenty are bad; the ten most recent are clean.
        for m in matches.iter_mut().take(20) {
            m.deaths_per_10 = 5.0;
        }

        let deaths = find(&detect(&matches), "high_death_rate").unwrap().clone();

        assert_eq!(deaths.status, PatternStatus::Improving);
        assert_eq!(deaths.recent_rate, Some(0.0));
        assert!(deaths.statement.contains("becoming rarer"));
    }

    #[test]
    fn a_pattern_still_happening_is_active() {
        let mut matches = history(30);
        for m in matches.iter_mut() {
            m.deaths_per_10 = 5.0;
        }

        let deaths = find(&detect(&matches), "high_death_rate").unwrap().clone();
        assert_eq!(deaths.status, PatternStatus::Active);
        assert_eq!(deaths.recent_rate, Some(1.0));
    }

    #[test]
    fn a_support_is_not_judged_on_a_cores_laning_stage() {
        let mut matches = history(20);
        for m in matches.iter_mut() {
            m.role = "Hard Support".into();
            m.last_hits_at_10 = Some(5);
        }

        assert!(find(&detect(&matches), "low_cs_at_10").is_none());
    }

    #[test]
    fn the_paired_detector_needs_both_halves_to_be_knowable() {
        let mut matches = history(20);
        for m in matches.iter_mut() {
            m.kill_participation = None;
            m.gpm = 900;
        }

        assert!(find(&detect(&matches), "farms_but_does_not_fight").is_none());
    }

    #[test]
    fn farming_well_while_absent_from_fights_is_one_pattern_not_two_readings() {
        let mut matches = history(20);
        for m in matches.iter_mut() {
            m.gpm = 600;
            m.kill_participation = Some(0.3);
        }

        let pattern = find(&detect(&matches), "farms_but_does_not_fight")
            .unwrap()
            .clone();
        assert_eq!(pattern.occurrences, 20);
    }

    #[test]
    fn examples_point_at_real_matches_newest_first() {
        let mut matches = history(20);
        for m in matches.iter_mut().take(12) {
            m.deaths_per_10 = 4.0;
        }

        let deaths = find(&detect(&matches), "high_death_rate").unwrap().clone();

        assert_eq!(deaths.examples.len(), 3);
        assert!(deaths.last_seen_at >= deaths.first_seen_at);
        // The newest offending match is the eleventh by index.
        assert_eq!(deaths.examples[0], matches[11].match_id);
    }

    #[test]
    fn patterns_come_back_worst_first() {
        let mut matches = history(30);
        for m in matches.iter_mut() {
            m.deaths_per_10 = 5.0; // every match
        }
        for m in matches.iter_mut().take(15) {
            m.kill_participation = Some(0.1); // half of them
        }

        let patterns = detect(&matches);
        assert_eq!(patterns[0].id, "high_death_rate");
        assert!(patterns.len() >= 2);
    }

    #[test]
    fn an_empty_history_produces_nothing() {
        assert!(detect(&[]).is_empty());
    }

    #[test]
    fn the_median_handles_both_parities_and_the_empty_case() {
        assert_eq!(median(vec![]), None);
        assert_eq!(median(vec![5.0]), Some(5.0));
        assert_eq!(median(vec![1.0, 3.0]), Some(2.0));
        assert_eq!(median(vec![3.0, 1.0, 2.0]), Some(2.0));
    }
}
