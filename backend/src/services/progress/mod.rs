//! Comparing two coaching sessions.
//!
//! Pure: two stored snapshots in, a set of judgements out. No database, no
//! clock, no provider — which is what makes every threshold below testable
//! against a fixed pair of sessions rather than against whatever the ladder
//! happened to do this week.
//!
//! The model never does this arithmetic. It is handed *"deaths per 10 minutes
//! improved by 1.4"* as a fact and asked to explain it; a model that subtracts
//! two numbers itself can subtract them wrong, cite both correctly, and leave
//! nothing downstream able to tell.
//!
//! # The thresholds, and why these ones
//!
//! A change has to clear a band before it is called a change, because two
//! twenty-game samples of the same player differ. The band is a documented
//! judgement, not a derivation — there is no sampling theory here, and
//! pretending otherwise would be worse than saying so:
//!
//! | Unit | Band | Why |
//! |---|---|---|
//! | Percentile, Score | **5 points** | Bounded 0-100. A relative change near either end is nonsense: the 2nd to the 4th percentile is a doubling and two points. |
//! | Proportion | **0.05** (5 points) | Bounded 0-1. Same argument, different scale — a win rate moving .50 → .55 is five points. |
//! | Ratio, PerMinute, Per10, Count | **5%** | Unbounded. "Ten percent more gold per minute" is what a player means; five percent of 512 GPM is about 26, which is inside the noise of twenty games. |
//!
//! Both are deliberately the same number so there is one thing to remember and
//! one thing to change.

use std::collections::HashMap;

use crate::domain::coaching_session::{CoachingSession, MetricSnapshot, MetricUnit};
use crate::domain::progress::{
    MetricProgress, MetricSeries, ProgressStatus, SeriesPoint, SessionProgress,
};

/// Movement smaller than this is called stable, on the bounded scales.
///
/// Points, on a 0-100 axis. See the module docs.
pub const STABLE_BAND_POINTS: f32 = 5.0;

/// The same band on a 0-1 proportion.
pub const STABLE_BAND_PROPORTION: f32 = 0.05;

/// Movement smaller than this fraction is called stable, on the unbounded
/// scales.
pub const STABLE_BAND_RATIO: f32 = 0.05;

/// Below this many matches behind a reading, no comparison is claimed.
///
/// Matches [`crate::services::benchmarks::percentile::MIN_SAMPLE`] rather than
/// being chosen separately: it is already this codebase's answer to "how few
/// games is too few to say anything", and having two different answers would
/// mean one of them is wrong.
pub const MIN_COMPARABLE_SAMPLE: i64 = crate::services::benchmarks::percentile::MIN_SAMPLE;

/// The key prefix whose presence and absence are themselves the finding.
///
/// A recurring pattern that stops firing is a resolved issue; a metric that
/// stops being measured is a gap. Telling them apart is what this prefix does.
const PATTERN_PREFIX: &str = "pattern.";

/// Compare two sessions, newest against the one before it.
///
/// `previous` must be the earlier session. The caller orders them; this does
/// not re-derive the order from timestamps, because a session's sequence is
/// the authority on which came first.
pub fn compare(previous: &CoachingSession, current: &CoachingSession) -> SessionProgress {
    let earlier: HashMap<&str, &MetricSnapshot> = previous
        .metrics
        .iter()
        .map(|m| (m.key.as_str(), m))
        .collect();
    let later: HashMap<&str, &MetricSnapshot> = current
        .metrics
        .iter()
        .map(|m| (m.key.as_str(), m))
        .collect();

    // Current session's order first, so the list reads the way the snapshot
    // does; anything only the earlier session had is appended rather than
    // dropped, because a metric that disappeared is a finding.
    let mut metrics: Vec<MetricProgress> = current
        .metrics
        .iter()
        .map(|now| compare_metric(earlier.get(now.key.as_str()).copied(), Some(now)))
        .collect();

    metrics.extend(
        previous
            .metrics
            .iter()
            .filter(|was| !later.contains_key(was.key.as_str()))
            .map(|was| compare_metric(Some(was), None)),
    );

    let performance = metrics
        .iter()
        .find(|m| m.key == "role.performance")
        .cloned();

    SessionProgress {
        role: current.role,
        role_label: current.role_label,
        previous_session_id: previous.id,
        previous_sequence: previous.sequence,
        previous_at: previous.created_at,
        current_session_id: current.id,
        current_sequence: current.sequence,
        current_at: current.created_at,
        headline: headline(&metrics),
        performance,
        metrics,
    }
}

/// One metric's two readings, judged.
fn compare_metric(
    previous: Option<&MetricSnapshot>,
    current: Option<&MetricSnapshot>,
) -> MetricProgress {
    // The identity comes from whichever reading exists, preferring the
    // current one: a label that was reworded should read as it does now.
    let identity = current.or(previous).expect("at least one reading exists");

    let mut out = MetricProgress {
        key: identity.key.clone(),
        label: identity.label.clone(),
        unit: identity.unit,
        higher_is_better: identity.higher_is_better,
        previous: previous.map(|m| m.value),
        current: current.map(|m| m.value),
        delta: None,
        direction_delta: None,
        percent_change: None,
        previous_sample: previous.map(|m| m.sample),
        current_sample: current.map(|m| m.sample),
        status: ProgressStatus::InsufficientData,
        status_label: ProgressStatus::InsufficientData.label(),
        note: None,
    };

    let is_pattern = identity.key.starts_with(PATTERN_PREFIX);

    let (was, now) = match (previous, current) {
        (Some(was), Some(now)) => (was, now),

        // A pattern that has started firing is the finding itself. Any other
        // metric appearing is a measurement that was not available before.
        (None, Some(_)) => {
            out.status = if is_pattern {
                ProgressStatus::NewIssue
            } else {
                out.note = Some("Not measured in the previous session.".into());
                ProgressStatus::InsufficientData
            };
            out.status_label = out.status.label();
            return out;
        }

        // A pattern that has stopped firing is resolved — the whole point is
        // that it is no longer in the data. Anything else has simply gone
        // unmeasured, which is not an achievement.
        (Some(_), None) => {
            out.status = if is_pattern {
                ProgressStatus::ResolvedIssue
            } else {
                out.note = Some("No longer measured.".into());
                ProgressStatus::InsufficientData
            };
            out.status_label = out.status.label();
            return out;
        }

        (None, None) => unreachable!("compare_metric is only called with a reading"),
    };

    // Two readings of *different things*. A unit or direction that changed
    // means the metric was redefined between sessions, and subtracting across
    // that would produce a number with no meaning.
    if was.unit != now.unit || was.higher_is_better != now.higher_is_better {
        out.note = Some("Measured differently in the previous session.".into());
        out.status_label = out.status.label();
        return out;
    }

    if was.sample < MIN_COMPARABLE_SAMPLE || now.sample < MIN_COMPARABLE_SAMPLE {
        out.note = Some(format!(
            "Needs {MIN_COMPARABLE_SAMPLE} matches on each side to compare; \
             this has {} and {}.",
            was.sample, now.sample
        ));
        out.status_label = out.status.label();
        return out;
    }

    let delta = now.value - was.value;
    // Signed so positive is always better, including for deaths and pattern
    // rates where the raw delta runs the other way. Done once, here, so no
    // reader downstream has to know which metrics are inverted.
    let direction_delta = if now.higher_is_better { delta } else { -delta };

    out.delta = Some(delta);
    out.direction_delta = Some(direction_delta);
    out.percent_change = relative_change(was.value, delta, now.unit);
    out.status = classify(was.value, delta, direction_delta, now.unit);
    out.status_label = out.status.label();
    out
}

/// Relative change, for the units where it means something.
///
/// `None` on the bounded scales, and when the earlier reading was zero —
/// dividing by it would report an infinite improvement from nothing.
fn relative_change(previous: f32, delta: f32, unit: MetricUnit) -> Option<f32> {
    if unit.compares_absolutely() || previous.abs() < f32::EPSILON {
        return None;
    }
    Some(delta / previous.abs())
}

/// Improved, declined, or too small to call.
fn classify(previous: f32, delta: f32, direction_delta: f32, unit: MetricUnit) -> ProgressStatus {
    let moved = if unit.compares_absolutely() {
        let band = match unit {
            MetricUnit::Proportion => STABLE_BAND_PROPORTION,
            _ => STABLE_BAND_POINTS,
        };
        delta.abs() >= band
    } else if previous.abs() < f32::EPSILON {
        // No baseline to be a fraction of. Any movement off zero is real, and
        // no movement is stable.
        delta.abs() > f32::EPSILON
    } else {
        (delta / previous.abs()).abs() >= STABLE_BAND_RATIO
    };

    if !moved {
        ProgressStatus::Stable
    } else if direction_delta > 0.0 {
        ProgressStatus::Improved
    } else {
        ProgressStatus::Declined
    }
}

/// The one movement worth leading with.
///
/// A new issue outranks any amount of improvement — something that started
/// going wrong is the more useful thing to say. Otherwise the largest genuine
/// movement wins, measured in the unit's own band so a percentile point and a
/// percent of gold are weighed on the same scale.
fn headline(metrics: &[MetricProgress]) -> Option<String> {
    if let Some(issue) = metrics
        .iter()
        .find(|m| m.status == ProgressStatus::NewIssue)
    {
        return Some(format!("{} has started showing up.", issue.label));
    }

    let biggest = metrics
        .iter()
        .filter(|m| {
            matches!(
                m.status,
                ProgressStatus::Improved | ProgressStatus::Declined
            )
        })
        .max_by(|a, b| significance(a).total_cmp(&significance(b)))?;

    let direction = if biggest.status == ProgressStatus::Improved {
        "improved"
    } else {
        "declined"
    };

    Some(match biggest.percent_change {
        Some(pct) => format!(
            "{} {direction} by {}%.",
            biggest.label,
            (pct.abs() * 100.0).round() as i32
        ),
        None => format!(
            "{} {direction} by {} points.",
            biggest.label,
            biggest.delta.map(|d| d.abs().round() as i32).unwrap_or(0)
        ),
    })
}

/// How far a movement cleared its own band, so units can be ranked together.
fn significance(m: &MetricProgress) -> f32 {
    let Some(delta) = m.delta else { return 0.0 };

    match (m.unit.compares_absolutely(), m.unit, m.percent_change) {
        (true, MetricUnit::Proportion, _) => delta.abs() / STABLE_BAND_PROPORTION,
        (true, _, _) => delta.abs() / STABLE_BAND_POINTS,
        (false, _, Some(pct)) => pct.abs() / STABLE_BAND_RATIO,
        (false, _, None) => 0.0,
    }
}

/// One metric's readings across a run of sessions, oldest first.
///
/// `sessions` arrives newest first, the order the history API serves. A
/// session missing the metric contributes no point rather than a zero — a gap
/// in a line is honest, a zero is a lie shaped like data.
pub fn series(sessions: &[CoachingSession], key: &str) -> Option<MetricSeries> {
    let mut identity = None;
    let mut points = Vec::new();

    for session in sessions.iter().rev() {
        let Some(metric) = session.metric(key) else {
            continue;
        };
        identity.get_or_insert((metric.label.clone(), metric.unit, metric.higher_is_better));

        points.push(SeriesPoint {
            session_id: session.id,
            sequence: session.sequence,
            at: session.created_at,
            value: metric.value,
        });
    }

    let (label, unit, higher_is_better) = identity?;

    Some(MetricSeries {
        key: key.to_string(),
        label,
        unit,
        higher_is_better,
        points,
    })
}

/// Every metric any of these sessions recorded, as a series each.
///
/// Ordered by the newest session's metric order, so the list reads the way the
/// current snapshot does.
pub fn all_series(sessions: &[CoachingSession]) -> Vec<MetricSeries> {
    let mut seen: Vec<&str> = Vec::new();

    for session in sessions {
        for metric in &session.metrics {
            if !seen.contains(&metric.key.as_str()) {
                seen.push(&metric.key);
            }
        }
    }

    seen.into_iter()
        .filter_map(|key| series(sessions, key))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::role::CoachableRole;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn metric(
        key: &str,
        value: f32,
        unit: MetricUnit,
        higher_is_better: bool,
        sample: i64,
    ) -> MetricSnapshot {
        MetricSnapshot {
            key: key.to_string(),
            label: key.to_string(),
            value,
            sample,
            unit,
            higher_is_better,
        }
    }

    fn session(sequence: i32, metrics: Vec<MetricSnapshot>) -> CoachingSession {
        CoachingSession {
            id: Uuid::new_v4(),
            role: CoachableRole::Carry,
            role_label: "Carry",
            sequence,
            analyzed_match_count: 20,
            analyzed_match_ids: Vec::new(),
            newest_match_at: None,
            performance: None,
            metrics,
            strengths: Vec::new(),
            weaknesses: Vec::new(),
            benchmarks: Vec::new(),
            heroes: Vec::new(),
            training_focus_id: None,
            analysis_id: None,
            created_at: Utc
                .with_ymd_and_hms(2026, 1, sequence as u32, 12, 0, 0)
                .unwrap(),
        }
    }

    fn only(progress: &SessionProgress, key: &str) -> MetricProgress {
        progress
            .metrics
            .iter()
            .find(|m| m.key == key)
            .expect("metric present")
            .clone()
    }

    #[test]
    fn a_rate_improving_past_the_band_is_improved() {
        // 512 → 575 is 12%, well past 5%.
        let before = session(
            1,
            vec![metric(
                "overall.gold_per_min",
                512.0,
                MetricUnit::PerMinute,
                true,
                20,
            )],
        );
        let after = session(
            2,
            vec![metric(
                "overall.gold_per_min",
                575.0,
                MetricUnit::PerMinute,
                true,
                20,
            )],
        );

        let gpm = only(&compare(&before, &after), "overall.gold_per_min");
        assert_eq!(gpm.status, ProgressStatus::Improved);
        assert_eq!(gpm.delta, Some(63.0));
        assert_eq!(gpm.direction_delta, Some(63.0));
        assert!((gpm.percent_change.unwrap() - 0.123).abs() < 0.01);
    }

    #[test]
    fn deaths_falling_is_an_improvement_not_a_decline() {
        // The direction correction the whole comparison rests on.
        let before = session(
            1,
            vec![metric("overall.deaths", 7.2, MetricUnit::Per10, false, 20)],
        );
        let after = session(
            2,
            vec![metric("overall.deaths", 5.8, MetricUnit::Per10, false, 20)],
        );

        let deaths = only(&compare(&before, &after), "overall.deaths");
        assert_eq!(deaths.status, ProgressStatus::Improved);
        // The raw delta is negative; the direction-corrected one is not.
        assert!((deaths.delta.unwrap() + 1.4).abs() < 0.001);
        assert!((deaths.direction_delta.unwrap() - 1.4).abs() < 0.001);
    }

    #[test]
    fn deaths_rising_is_a_decline() {
        let before = session(
            1,
            vec![metric("overall.deaths", 5.8, MetricUnit::Per10, false, 20)],
        );
        let after = session(
            2,
            vec![metric("overall.deaths", 7.2, MetricUnit::Per10, false, 20)],
        );

        assert_eq!(
            only(&compare(&before, &after), "overall.deaths").status,
            ProgressStatus::Declined
        );
    }

    #[test]
    fn a_movement_inside_the_band_is_stable_not_unknown() {
        // 512 → 520 is 1.6%. Stable is an answer; InsufficientData would be a
        // different and wrong one.
        let before = session(
            1,
            vec![metric(
                "overall.gold_per_min",
                512.0,
                MetricUnit::PerMinute,
                true,
                20,
            )],
        );
        let after = session(
            2,
            vec![metric(
                "overall.gold_per_min",
                520.0,
                MetricUnit::PerMinute,
                true,
                20,
            )],
        );

        let gpm = only(&compare(&before, &after), "overall.gold_per_min");
        assert_eq!(gpm.status, ProgressStatus::Stable);
        assert!(gpm.note.is_none());
    }

    #[test]
    fn a_bounded_scale_moves_in_points_not_percentages() {
        // 2nd to 4th percentile: a doubling, and two points. Relative change
        // would call this a spectacular improvement.
        let before = session(
            1,
            vec![metric(
                "benchmark.gold_per_min",
                2.0,
                MetricUnit::Percentile,
                true,
                20,
            )],
        );
        let after = session(
            2,
            vec![metric(
                "benchmark.gold_per_min",
                4.0,
                MetricUnit::Percentile,
                true,
                20,
            )],
        );

        let p = only(&compare(&before, &after), "benchmark.gold_per_min");
        assert_eq!(p.status, ProgressStatus::Stable);
        assert_eq!(p.percent_change, None, "a percentile has no useful ratio");

        // Six points is past the band.
        let after = session(
            2,
            vec![metric(
                "benchmark.gold_per_min",
                8.0,
                MetricUnit::Percentile,
                true,
                20,
            )],
        );
        assert_eq!(
            only(&compare(&before, &after), "benchmark.gold_per_min").status,
            ProgressStatus::Improved
        );
    }

    #[test]
    fn a_proportion_moves_in_points_too() {
        // A win rate .50 → .54 is four points: inside the band, even though it
        // is an 8% relative change.
        let before = session(
            1,
            vec![metric(
                "role.win_rate",
                0.50,
                MetricUnit::Proportion,
                true,
                20,
            )],
        );
        let after = session(
            2,
            vec![metric(
                "role.win_rate",
                0.54,
                MetricUnit::Proportion,
                true,
                20,
            )],
        );

        assert_eq!(
            only(&compare(&before, &after), "role.win_rate").status,
            ProgressStatus::Stable
        );
    }

    #[test]
    fn a_pattern_that_starts_firing_is_a_new_issue() {
        let before = session(1, Vec::new());
        let after = session(
            2,
            vec![metric(
                "pattern.high_death_rate",
                0.6,
                MetricUnit::Proportion,
                false,
                20,
            )],
        );

        let p = only(&compare(&before, &after), "pattern.high_death_rate");
        assert_eq!(p.status, ProgressStatus::NewIssue);
        assert_eq!(p.previous, None);
        assert_eq!(p.current, Some(0.6));
    }

    #[test]
    fn a_pattern_that_stops_firing_is_resolved() {
        let before = session(
            1,
            vec![metric(
                "pattern.high_death_rate",
                0.6,
                MetricUnit::Proportion,
                false,
                20,
            )],
        );
        let after = session(2, Vec::new());

        let p = only(&compare(&before, &after), "pattern.high_death_rate");
        assert_eq!(p.status, ProgressStatus::ResolvedIssue);
        assert_eq!(p.current, None);
    }

    #[test]
    fn an_ordinary_metric_appearing_is_not_an_achievement() {
        // Kill participation needs team totals the provider does not always
        // supply. Its arrival is a measurement, not an improvement.
        let before = session(1, Vec::new());
        let after = session(
            2,
            vec![metric(
                "overall.kill_participation",
                0.62,
                MetricUnit::Proportion,
                true,
                20,
            )],
        );

        let kp = only(&compare(&before, &after), "overall.kill_participation");
        assert_eq!(kp.status, ProgressStatus::InsufficientData);
        assert!(kp.note.as_deref().unwrap().contains("previous session"));
    }

    #[test]
    fn a_thin_sample_on_either_side_is_not_compared() {
        let before = session(
            1,
            vec![metric("overall.kda", 3.0, MetricUnit::Ratio, true, 2)],
        );
        let after = session(
            2,
            vec![metric("overall.kda", 5.0, MetricUnit::Ratio, true, 20)],
        );

        let kda = only(&compare(&before, &after), "overall.kda");
        assert_eq!(kda.status, ProgressStatus::InsufficientData);
        assert_eq!(kda.delta, None, "no arithmetic on a sample that thin");
        // The readings themselves are still shown; only the claim is withheld.
        assert_eq!(kda.previous, Some(3.0));
        assert_eq!(kda.current, Some(5.0));
    }

    #[test]
    fn a_metric_redefined_between_sessions_is_not_subtracted() {
        // Same key, different unit: two readings of different things.
        let before = session(
            1,
            vec![metric("overall.deaths", 7.2, MetricUnit::Per10, false, 20)],
        );
        let after = session(
            2,
            vec![metric(
                "overall.deaths",
                0.72,
                MetricUnit::PerMinute,
                false,
                20,
            )],
        );

        let deaths = only(&compare(&before, &after), "overall.deaths");
        assert_eq!(deaths.status, ProgressStatus::InsufficientData);
        assert_eq!(deaths.delta, None);
        assert!(deaths.note.as_deref().unwrap().contains("differently"));
    }

    #[test]
    fn a_direction_that_flipped_is_also_a_redefinition() {
        let before = session(
            1,
            vec![metric("overall.deaths", 7.2, MetricUnit::Per10, false, 20)],
        );
        let after = session(
            2,
            vec![metric("overall.deaths", 5.8, MetricUnit::Per10, true, 20)],
        );

        assert_eq!(
            only(&compare(&before, &after), "overall.deaths").status,
            ProgressStatus::InsufficientData
        );
    }

    #[test]
    fn a_new_issue_leads_the_headline_over_any_improvement() {
        let before = session(
            1,
            vec![metric(
                "overall.gold_per_min",
                400.0,
                MetricUnit::PerMinute,
                true,
                20,
            )],
        );
        let after = session(
            2,
            vec![
                metric(
                    "overall.gold_per_min",
                    600.0,
                    MetricUnit::PerMinute,
                    true,
                    20,
                ),
                metric(
                    "pattern.high_death_rate",
                    0.6,
                    MetricUnit::Proportion,
                    false,
                    20,
                ),
            ],
        );

        let headline = compare(&before, &after).headline.unwrap();
        assert!(
            headline.contains("started showing up"),
            "a fifty percent farm gain should not bury a new problem: {headline}"
        );
    }

    #[test]
    fn the_headline_otherwise_names_the_biggest_movement() {
        let before = session(
            1,
            vec![
                metric(
                    "overall.gold_per_min",
                    500.0,
                    MetricUnit::PerMinute,
                    true,
                    20,
                ),
                metric("overall.kda", 3.0, MetricUnit::Ratio, true, 20),
            ],
        );
        let after = session(
            2,
            vec![
                // 6% — just past the band.
                metric(
                    "overall.gold_per_min",
                    530.0,
                    MetricUnit::PerMinute,
                    true,
                    20,
                ),
                // 40% — much bigger.
                metric("overall.kda", 4.2, MetricUnit::Ratio, true, 20),
            ],
        );

        let headline = compare(&before, &after).headline.unwrap();
        assert!(headline.contains("overall.kda"), "{headline}");
        assert!(headline.contains("improved"), "{headline}");
    }

    #[test]
    fn nothing_moving_produces_no_headline() {
        let before = session(
            1,
            vec![metric("overall.kda", 3.0, MetricUnit::Ratio, true, 20)],
        );
        let after = session(
            2,
            vec![metric("overall.kda", 3.02, MetricUnit::Ratio, true, 20)],
        );

        assert_eq!(compare(&before, &after).headline, None);
    }

    #[test]
    fn the_performance_headline_is_pulled_out_separately() {
        let before = session(
            1,
            vec![metric(
                "role.performance",
                54.0,
                MetricUnit::Score,
                true,
                20,
            )],
        );
        let after = session(
            2,
            vec![metric(
                "role.performance",
                61.0,
                MetricUnit::Score,
                true,
                20,
            )],
        );

        let progress = compare(&before, &after);
        let performance = progress
            .performance
            .expect("the role score is the headline");
        assert_eq!(performance.status, ProgressStatus::Improved);
        assert_eq!(performance.previous, Some(54.0));
        assert_eq!(performance.current, Some(61.0));
    }

    #[test]
    fn a_metric_only_the_earlier_session_had_is_still_reported() {
        let before = session(
            1,
            vec![
                metric("overall.kda", 3.0, MetricUnit::Ratio, true, 20),
                metric(
                    "overall.kill_participation",
                    0.6,
                    MetricUnit::Proportion,
                    true,
                    20,
                ),
            ],
        );
        let after = session(
            2,
            vec![metric("overall.kda", 3.1, MetricUnit::Ratio, true, 20)],
        );

        let progress = compare(&before, &after);
        assert_eq!(
            progress.metrics.len(),
            2,
            "a metric that vanished is a finding"
        );
        let kp = only(&progress, "overall.kill_participation");
        assert_eq!(kp.status, ProgressStatus::InsufficientData);
        assert!(kp.note.as_deref().unwrap().contains("No longer measured"));
    }

    #[test]
    fn a_series_reads_oldest_first_across_sessions() {
        // The history API serves newest first; the chart reads left to right.
        let sessions = vec![
            session(
                3,
                vec![metric(
                    "role.performance",
                    61.0,
                    MetricUnit::Score,
                    true,
                    20,
                )],
            ),
            session(
                2,
                vec![metric(
                    "role.performance",
                    57.0,
                    MetricUnit::Score,
                    true,
                    20,
                )],
            ),
            session(
                1,
                vec![metric(
                    "role.performance",
                    54.0,
                    MetricUnit::Score,
                    true,
                    20,
                )],
            ),
        ];

        let s = series(&sessions, "role.performance").unwrap();
        let values: Vec<f32> = s.points.iter().map(|p| p.value).collect();
        assert_eq!(values, vec![54.0, 57.0, 61.0]);
        assert_eq!(s.points[0].sequence, 1);
    }

    #[test]
    fn a_session_missing_the_metric_leaves_a_gap_rather_than_a_zero() {
        let sessions = vec![
            session(
                3,
                vec![metric(
                    "role.performance",
                    61.0,
                    MetricUnit::Score,
                    true,
                    20,
                )],
            ),
            session(2, Vec::new()),
            session(
                1,
                vec![metric(
                    "role.performance",
                    54.0,
                    MetricUnit::Score,
                    true,
                    20,
                )],
            ),
        ];

        let s = series(&sessions, "role.performance").unwrap();
        assert_eq!(s.points.len(), 2, "a zero here would invent a collapse");
        assert_eq!(s.points[0].sequence, 1);
        assert_eq!(s.points[1].sequence, 3);
    }

    #[test]
    fn a_metric_nobody_recorded_has_no_series() {
        let sessions = vec![session(1, Vec::new())];
        assert!(series(&sessions, "role.performance").is_none());
    }

    #[test]
    fn every_recorded_metric_gets_a_series() {
        let sessions = vec![
            session(
                2,
                vec![
                    metric("role.performance", 61.0, MetricUnit::Score, true, 20),
                    metric("overall.kda", 3.1, MetricUnit::Ratio, true, 20),
                ],
            ),
            session(
                1,
                vec![metric(
                    "role.performance",
                    54.0,
                    MetricUnit::Score,
                    true,
                    20,
                )],
            ),
        ];

        let all = all_series(&sessions);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].key, "role.performance");
        assert_eq!(all[0].points.len(), 2);
        assert_eq!(all[1].points.len(), 1);
    }
}
