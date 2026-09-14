//! Choosing the one thing to work on, and checking whether it is working.
//!
//! Two rules from the spec shape everything here:
//!
//!   1. **One focus at a time.** A list of ten weaknesses is not coaching.
//!   2. **Not simply the lowest statistic.** The choice weighs benchmark gap,
//!      pattern history, recent performance, impact, confidence and recency
//!      together, and reports the parts alongside the total.
//!
//! And one rule that is not in the spec but follows from it: a focus has to be
//! *checkable*. Every focus carries a measure, a baseline and a target, so the
//! progress series is arithmetic over the same numbers the focus was chosen
//! from — not a second, looser opinion about whether things are going well.
//!
//! Pure. History and the clock come from the caller.

use crate::domain::benchmark::{BenchmarkMetric, BenchmarkResult, Confidence};
use crate::domain::player_model::{AnalyzedMatch, PatternStatus, RecurringPattern};
use crate::domain::training::{
    FocusMeasure, FocusScorePart, FocusSource, FocusStatus, FocusWeights, ProgressPoint,
    ProgressSeries, TrainingFocus,
};
use crate::services::benchmarks::percentile;
use crate::services::player_model::patterns::{self, Baselines};

/// Matches per point on the progress series, and the window "where you stand
/// now" is read over. Ten is the window the hero pool and recent form already
/// use, so "recent" means one thing across the product.
pub const WINDOW: usize = 10;

/// Below this many measurable matches in a window, no value is reported for
/// it: an average over two games is noise, and plotting it would draw a
/// trend out of nothing.
const MIN_WINDOW_SAMPLE: i64 = 3;
/// A target is only called met when a full-strength window says so.
const MIN_TARGET_SAMPLE: i64 = 5;

/// Where a pattern rate has to fall to count as beaten: comfortably under the
/// rate at which the detector flags it in the first place.
const PATTERN_TARGET_RATE: f32 = patterns::MIN_RATE - 0.1;

/// A percentile at or below this is a gap worth training.
const GAP_PERCENTILE: f32 = 45.0;

/// Days after which an observation stops counting as recent.
const RECENCY_HORIZON_DAYS: f32 = 30.0;

/// Read one match's contribution to a measure.
///
/// `None` means the match cannot answer — an unparsed replay, a missing team
/// total — never that the value was zero. A pattern reads as 1 or 0, so the
/// mean of a window *is* its rate and every measure shares one code path.
pub fn sample(
    measure: FocusMeasure,
    pattern_id: Option<&str>,
    m: &AnalyzedMatch,
    baselines: &Baselines,
) -> Option<f32> {
    match measure {
        FocusMeasure::DeathsPer10 => Some(m.deaths_per_10),
        FocusMeasure::KillParticipation => m.kill_participation,
        FocusMeasure::GoldPerMin => Some(m.gpm as f32),
        FocusMeasure::LastHitsAt10 => m.last_hits_at_10.map(|lh| lh as f32),
        FocusMeasure::PatternRate => {
            let id = pattern_id?;
            let detector = patterns::DETECTORS.iter().find(|d| d.id == id)?;
            (detector.measure)(m, baselines).map(|hit| if hit { 1.0 } else { 0.0 })
        }
    }
}

/// The measure across a set of matches, or `None` when too few can answer.
pub fn window_value(
    measure: FocusMeasure,
    pattern_id: Option<&str>,
    matches: &[&AnalyzedMatch],
    baselines: &Baselines,
    min_sample: i64,
) -> Option<(f32, i64)> {
    let values: Vec<f32> = matches
        .iter()
        .filter_map(|m| sample(measure, pattern_id, m, baselines))
        .collect();

    let count = values.len() as i64;
    if count < min_sample {
        return None;
    }

    Some((values.iter().sum::<f32>() / count as f32, count))
}

/// The measure over time, oldest bucket first.
///
/// Buckets are fixed-size slices of the player's history rather than calendar
/// weeks: a player who plays twice one week and thirty times the next would
/// otherwise get two points of wildly different weight plotted as equals.
pub fn series(
    measure: FocusMeasure,
    pattern_id: Option<&str>,
    history: &[AnalyzedMatch],
    target_value: Option<f32>,
) -> ProgressSeries {
    let baselines = Baselines::from(history);

    // Oldest first, so the buckets read left to right in time.
    let mut ordered: Vec<&AnalyzedMatch> = history.iter().collect();
    ordered.sort_by_key(|m| m.started_at);

    let points = ordered
        .chunks(WINDOW)
        .filter_map(|bucket| {
            let (value, matches) =
                window_value(measure, pattern_id, bucket, &baselines, MIN_WINDOW_SAMPLE)?;
            Some(ProgressPoint {
                matches,
                value,
                at: bucket.last()?.started_at,
            })
        })
        .collect();

    ProgressSeries {
        measure,
        label: measure.label(),
        higher_is_better: measure.higher_is_better(),
        points,
        window: WINDOW as i64,
        target_value,
    }
}

/// Where the player stands now: the measure over the most recent window.
pub fn current_value(
    measure: FocusMeasure,
    pattern_id: Option<&str>,
    history: &[AnalyzedMatch],
    min_sample: i64,
) -> Option<(f32, i64)> {
    let baselines = Baselines::from(history);

    let mut ordered: Vec<&AnalyzedMatch> = history.iter().collect();
    ordered.sort_by_key(|m| std::cmp::Reverse(m.started_at));
    let window: Vec<&AnalyzedMatch> = ordered.into_iter().take(WINDOW).collect();

    window_value(measure, pattern_id, &window, &baselines, min_sample)
}

/// A focus the selector is considering.
struct Candidate {
    key: String,
    title: String,
    why: String,
    source: FocusSource,
    measure: FocusMeasure,
    pattern_id: Option<String>,
    baseline_value: f32,
    target_value: f32,
    sample: i64,
    confidence: Confidence,
    /// 0-100. How far short of the standard this is.
    gap: f32,
    /// 0-100. Whether a detected pattern backs it.
    pattern_support: f32,
    /// Days since the evidence was last seen.
    days_since: f32,
}

/// Which benchmark metrics map onto a trainable measure.
///
/// Short on purpose. A benchmark metric only belongs here when the same
/// quantity can be read back out of a single stored match, because otherwise
/// there is no honest way to plot progress against it — and the units have to
/// line up, hence the scale.
const BENCHMARK_MEASURES: &[(BenchmarkMetric, FocusMeasure, f32)] = &[
    // The provider reports deaths per minute; matches store per 10 minutes.
    (
        BenchmarkMetric::DeathsPerMin,
        FocusMeasure::DeathsPer10,
        10.0,
    ),
    (BenchmarkMetric::GoldPerMin, FocusMeasure::GoldPerMin, 1.0),
];

/// Everything the selector needs.
pub struct SelectionInputs<'a> {
    pub history: &'a [AnalyzedMatch],
    pub benchmarks: &'a [BenchmarkResult],
    pub patterns: &'a [RecurringPattern],
    pub now: chrono::DateTime<chrono::Utc>,
}

/// Rank every candidate and return them, best first.
///
/// Returning the whole ranking rather than only the winner is deliberate: the
/// runner-up is what the next focus will be, and showing the parts is what
/// makes "why this one" answerable.
pub fn rank(inputs: &SelectionInputs<'_>, weights: FocusWeights) -> Vec<TrainingFocus> {
    let mut scored: Vec<TrainingFocus> = candidates(inputs)
        .into_iter()
        .map(|candidate| score(candidate, inputs, weights))
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.key.cmp(&b.key))
    });
    scored
}

/// The single focus to work on, or `None` when there is nothing to train.
pub fn select(inputs: &SelectionInputs<'_>, weights: FocusWeights) -> Option<TrainingFocus> {
    rank(inputs, weights).into_iter().next()
}

fn candidates(inputs: &SelectionInputs<'_>) -> Vec<Candidate> {
    let baselines = Baselines::from(inputs.history);
    let mut found = Vec::new();

    for pattern in inputs
        .patterns
        .iter()
        .filter(|p| p.status != PatternStatus::Resolved)
    {
        // An improving pattern is still a pattern; it just scores lower.
        let support = match pattern.status {
            PatternStatus::Improving => 60.0,
            _ => 100.0,
        };

        found.push(Candidate {
            key: format!("pattern.{}", pattern.id),
            title: pattern.label.clone(),
            why: pattern.statement.clone(),
            source: FocusSource::Pattern,
            measure: FocusMeasure::PatternRate,
            pattern_id: Some(pattern.id.clone()),
            baseline_value: pattern.rate,
            target_value: PATTERN_TARGET_RATE,
            sample: pattern.measured,
            confidence: pattern.confidence,
            // How far above the flagging threshold it sits, as a share of the
            // room above it.
            gap: (((pattern.rate - patterns::MIN_RATE) / (1.0 - patterns::MIN_RATE)) * 100.0)
                .clamp(0.0, 100.0),
            pattern_support: support,
            days_since: pattern
                .last_seen_at
                .map(|at| (inputs.now - at).num_days() as f32)
                .unwrap_or(RECENCY_HORIZON_DAYS),
        });
    }

    for (metric, measure, scale) in BENCHMARK_MEASURES {
        let Some(result) = inputs.benchmarks.iter().find(|r| r.metric == *metric) else {
            continue;
        };
        // No percentile means the engine declined to rank the player. Turning
        // that into a training goal would undo the decision.
        let (Some(percentile_value), Some(median)) = (result.percentile, result.peer_median) else {
            continue;
        };
        if percentile_value > GAP_PERCENTILE || result.confidence == Confidence::Insufficient {
            continue;
        }

        // Where the player actually stands, in the units a match is stored in.
        let Some((baseline, sample)) =
            current_value(*measure, None, inputs.history, MIN_WINDOW_SAMPLE)
        else {
            continue;
        };

        // Reachable, not aspirational: the median first, and only then the
        // top-20% line for someone already past it.
        let median_scaled = median * scale;
        let target = if measure.higher_is_better() {
            if baseline < median_scaled {
                median_scaled
            } else {
                result.top_20_value.unwrap_or(median_scaled) * scale
            }
        } else if baseline > median_scaled {
            median_scaled
        } else {
            result.top_20_value.unwrap_or(median_scaled) * scale
        };

        found.push(Candidate {
            key: format!("benchmark.{}", metric.slug()),
            title: format!("Improve your {}", measure.label().to_lowercase()),
            why: format!(
                "You sit at the {percentile_value:.0}th percentile for {} on your most-played hero.",
                result.label.to_lowercase(),
            ),
            source: FocusSource::Benchmark,
            measure: *measure,
            pattern_id: None,
            baseline_value: baseline,
            target_value: target,
            sample,
            confidence: result.confidence,
            // Distance below the median, as a share of the way to the floor.
            gap: ((GAP_PERCENTILE - percentile_value) / GAP_PERCENTILE * 100.0).clamp(0.0, 100.0),
            // Raised when a detected pattern says the same thing.
            pattern_support: supporting_pattern(*measure, inputs.patterns, &baselines),
            days_since: 0.0,
        });
    }

    found
}

/// Whether a detected pattern corroborates a benchmark gap.
///
/// Two independent readings agreeing is worth more than either alone, which is
/// exactly what the spec means by weighing "benchmark gap" and "historical
/// pattern" as separate inputs.
fn supporting_pattern(
    measure: FocusMeasure,
    patterns: &[RecurringPattern],
    _baselines: &Baselines,
) -> f32 {
    let related = match measure {
        FocusMeasure::DeathsPer10 => "high_death_rate",
        FocusMeasure::KillParticipation => "low_kill_participation",
        FocusMeasure::LastHitsAt10 => "low_cs_at_10",
        _ => return 0.0,
    };

    patterns
        .iter()
        .find(|p| p.id == related && p.status != PatternStatus::Resolved)
        .map(|_| 100.0)
        .unwrap_or(0.0)
}

/// Turn a candidate into a scored focus.
fn score(
    candidate: Candidate,
    inputs: &SelectionInputs<'_>,
    weights: FocusWeights,
) -> TrainingFocus {
    let recent = recent_score(&candidate, inputs);
    let confidence_score = match candidate.confidence {
        Confidence::Insufficient => 0.0,
        Confidence::Low => 50.0,
        Confidence::Adequate => 100.0,
    };
    let recency = (1.0 - (candidate.days_since / RECENCY_HORIZON_DAYS).clamp(0.0, 1.0)) * 100.0;
    let impact = candidate.measure.impact();

    let parts = vec![
        FocusScorePart {
            key: "gap",
            label: "Benchmark gap",
            score: candidate.gap,
            weight: weights.gap,
            detail: candidate.why.clone(),
        },
        FocusScorePart {
            key: "pattern",
            label: "Historical pattern",
            score: candidate.pattern_support,
            weight: weights.pattern,
            detail: if candidate.pattern_support > 0.0 {
                "A recurring pattern across your history says the same thing.".into()
            } else {
                "No recurring pattern corroborates this yet.".into()
            },
        },
        FocusScorePart {
            key: "recent",
            label: "Recent performance",
            score: recent,
            weight: weights.recent,
            detail: format!(
                "Measured over your last {} matches.",
                WINDOW.min(inputs.history.len())
            ),
        },
        FocusScorePart {
            key: "impact",
            label: "Impact",
            score: impact,
            weight: weights.impact,
            detail: format!(
                "How much moving {} tends to change games.",
                candidate.measure.label().to_lowercase()
            ),
        },
        FocusScorePart {
            key: "confidence",
            label: "Confidence",
            score: confidence_score,
            weight: weights.confidence,
            detail: format!(
                "{} {} behind the figure.",
                candidate.sample,
                if candidate.sample == 1 {
                    "match"
                } else {
                    "matches"
                }
            ),
        },
        FocusScorePart {
            key: "recency",
            label: "Recency",
            score: recency,
            weight: weights.recency,
            detail: if candidate.days_since <= 1.0 {
                "Seen in your most recent matches.".into()
            } else {
                format!("Last seen {:.0} days ago.", candidate.days_since)
            },
        },
    ];

    let total_weight = weights.total();
    let score = if total_weight > 0.0 {
        parts
            .iter()
            .map(|part| part.score * part.weight)
            .sum::<f32>()
            / total_weight
    } else {
        0.0
    };

    let current = current_value(
        candidate.measure,
        candidate.pattern_id.as_deref(),
        inputs.history,
        MIN_WINDOW_SAMPLE,
    );

    TrainingFocus {
        id: None,
        key: candidate.key,
        title: candidate.title,
        why: candidate.why,
        source: candidate.source,
        measure: candidate.measure,
        measure_label: candidate.measure.label(),
        pattern_id: candidate.pattern_id,
        higher_is_better: candidate.measure.higher_is_better(),
        baseline_value: candidate.baseline_value,
        target_value: candidate.target_value,
        current_value: current.map(|(value, _)| value),
        progress: current.map(|(value, _)| {
            progress_toward(candidate.baseline_value, candidate.target_value, value)
        }),
        target_met: current.is_some_and(|(value, sample)| {
            sample >= MIN_TARGET_SAMPLE
                && met(
                    candidate.measure.higher_is_better(),
                    value,
                    candidate.target_value,
                )
        }),
        status: FocusStatus::Active,
        status_label: FocusStatus::Active.label(),
        score: score.clamp(0.0, 100.0),
        score_parts: parts,
        confidence: candidate.confidence,
        sample: candidate.sample,
        started_at: None,
        ended_at: None,
    }
}

/// How much worse the recent window is than the history as a whole.
///
/// A problem that is getting worse outranks one the player is already fixing —
/// which is the spec's "recent performance" input, and the reason an improving
/// pattern does not hold the focus hostage.
fn recent_score(candidate: &Candidate, inputs: &SelectionInputs<'_>) -> f32 {
    let baselines = Baselines::from(inputs.history);
    let all: Vec<&AnalyzedMatch> = inputs.history.iter().collect();

    let overall = window_value(
        candidate.measure,
        candidate.pattern_id.as_deref(),
        &all,
        &baselines,
        MIN_WINDOW_SAMPLE,
    );
    let recent = current_value(
        candidate.measure,
        candidate.pattern_id.as_deref(),
        inputs.history,
        MIN_WINDOW_SAMPLE,
    );

    let (Some((overall, _)), Some((recent, _))) = (overall, recent) else {
        // Nothing to compare: neutral rather than a guess in either direction.
        return 50.0;
    };

    // Relative change, so the scale of the measure does not matter.
    let denominator = overall.abs().max(0.001);
    let change = (recent - overall) / denominator;
    let worse = if candidate.measure.higher_is_better() {
        -change
    } else {
        change
    };

    // A 20% move in either direction saturates the component.
    (50.0 + worse * 250.0).clamp(0.0, 100.0)
}

/// Position along the road from baseline to target, 0-1.
pub fn progress_toward(baseline: f32, target: f32, current: f32) -> f32 {
    let span = target - baseline;
    if span.abs() < f32::EPSILON {
        // Nowhere to travel: either already there, or a degenerate target.
        return 1.0;
    }
    ((current - baseline) / span).clamp(0.0, 1.0)
}

fn met(higher_is_better: bool, value: f32, target: f32) -> bool {
    if higher_is_better {
        value >= target
    } else {
        value <= target
    }
}

/// Whether a stored focus still deserves to be the focus.
///
/// Stability matters: a focus that changed every time a match landed would be
/// a feed, not a training plan. It is replaced only when it is finished, or
/// when the evidence behind it has gone.
pub enum FocusVerdict {
    Keep,
    Achieved,
    Retired,
}

pub fn review(focus: &TrainingFocus, inputs: &SelectionInputs<'_>) -> FocusVerdict {
    let current = current_value(
        focus.measure,
        focus.pattern_id.as_deref(),
        inputs.history,
        MIN_TARGET_SAMPLE,
    );

    if let Some((value, _)) = current {
        if met(focus.higher_is_better, value, focus.target_value) {
            return FocusVerdict::Achieved;
        }
    }

    // The pattern behind a pattern-focus stopped being detected at all: the
    // goal is moot, but it was not met either.
    if focus.measure == FocusMeasure::PatternRate {
        let still_detected = focus.pattern_id.as_ref().is_some_and(|id| {
            inputs
                .patterns
                .iter()
                .any(|p| &p.id == id && p.status != PatternStatus::Resolved)
        });
        if !still_detected {
            return FocusVerdict::Retired;
        }
    }

    FocusVerdict::Keep
}

/// How well each hero suits the current focus, -1 to 1.
///
/// The spec asks for training-focus compatibility as a *modifier* on hero fit
/// rather than another weighted component, and this is the honest version of
/// it: no claim about what a hero teaches, just whether the player's own
/// figures on that hero are better or worse than their overall figures for the
/// thing they are working on.
///
/// Heroes with too little history are absent rather than neutral — a hero
/// played twice says nothing about anything.
pub fn hero_alignment(
    measure: FocusMeasure,
    pattern_id: Option<&str>,
    history: &[AnalyzedMatch],
) -> std::collections::HashMap<i32, f32> {
    let baselines = Baselines::from(history);
    let all: Vec<&AnalyzedMatch> = history.iter().collect();

    let Some((overall, _)) = window_value(measure, pattern_id, &all, &baselines, MIN_TARGET_SAMPLE)
    else {
        return std::collections::HashMap::new();
    };
    let denominator = overall.abs().max(0.001);

    let mut by_hero: std::collections::HashMap<i32, Vec<&AnalyzedMatch>> =
        std::collections::HashMap::new();
    for m in history {
        by_hero.entry(m.hero_id).or_default().push(m);
    }

    by_hero
        .into_iter()
        .filter_map(|(hero_id, matches)| {
            let (value, _) =
                window_value(measure, pattern_id, &matches, &baselines, MIN_TARGET_SAMPLE)?;

            let better = if measure.higher_is_better() {
                (value - overall) / denominator
            } else {
                (overall - value) / denominator
            };
            // A 20% difference saturates, so one unusual hero cannot dominate.
            Some((hero_id, (better * 5.0).clamp(-1.0, 1.0)))
        })
        .collect()
}

/// Confidence for a focus, from the sample behind it.
pub fn confidence_for(sample: i64) -> Confidence {
    percentile::confidence_for(sample)
}

/// Fill a focus's live figures from the current history.
///
/// A stored focus keeps its baseline and target — those are the promise made
/// when it was set — and everything else is recomputed, because "where do I
/// stand now" is a question about today's matches.
pub fn hydrate(mut focus: TrainingFocus, inputs: &SelectionInputs<'_>) -> TrainingFocus {
    let current = current_value(
        focus.measure,
        focus.pattern_id.as_deref(),
        inputs.history,
        MIN_WINDOW_SAMPLE,
    );

    focus.current_value = current.map(|(value, _)| value);
    focus.progress =
        current.map(|(value, _)| progress_toward(focus.baseline_value, focus.target_value, value));
    focus.target_met = current.is_some_and(|(value, sample)| {
        sample >= MIN_TARGET_SAMPLE && met(focus.higher_is_better, value, focus.target_value)
    });
    focus.sample = current.map(|(_, sample)| sample).unwrap_or(0);
    focus.confidence = confidence_for(focus.sample);
    focus
}

/// The player's current focus, selecting or retiring one as needed.
///
/// The order is the product rule: keep what is already being worked on unless
/// it is finished or its evidence has gone. A focus that changed whenever a
/// match landed would be a feed rather than a plan.
pub async fn ensure(
    pool: &sqlx::PgPool,
    dota_player_id: uuid::Uuid,
    inputs: &SelectionInputs<'_>,
    weights: FocusWeights,
) -> Result<Option<TrainingFocus>, sqlx::Error> {
    let ranked = rank(inputs, weights);
    let mut just_closed: Option<String> = None;

    if let Some(stored) = crate::repositories::training::active(pool, dota_player_id).await? {
        let live = hydrate(stored, inputs);

        match review(&live, inputs) {
            FocusVerdict::Keep => {
                // Reattach the parts from this run's ranking, so "why this
                // one" stays answerable rather than frozen at selection time.
                let parts = ranked
                    .iter()
                    .find(|c| c.key == live.key)
                    .map(|c| c.score_parts.clone())
                    .unwrap_or_default();

                return Ok(Some(TrainingFocus {
                    score_parts: parts,
                    ..live
                }));
            }
            FocusVerdict::Achieved => {
                crate::repositories::training::close(pool, dota_player_id, FocusStatus::Achieved)
                    .await?;
                just_closed = Some(live.key);
            }
            FocusVerdict::Retired => {
                crate::repositories::training::close(pool, dota_player_id, FocusStatus::Retired)
                    .await?;
                just_closed = Some(live.key);
            }
        }
    }

    // Nothing active, or the last one just closed. The one just finished is
    // excluded: a pattern whose recent rate has fallen below target is often
    // still detected across the whole history, and re-selecting it would hand
    // the player back the goal they just met.
    let Some(next) = ranked
        .into_iter()
        .find(|candidate| just_closed.as_deref() != Some(candidate.key.as_str()))
    else {
        return Ok(None);
    };

    let (id, started_at) =
        crate::repositories::training::start(pool, dota_player_id, &next).await?;
    Ok(Some(TrainingFocus {
        id: Some(id),
        started_at: Some(started_at),
        ..next
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::benchmark::Segment;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn now() -> chrono::DateTime<chrono::Utc> {
        Utc.timestamp_opt(1_800_000_000, 0).unwrap()
    }

    fn match_at(index: i64) -> AnalyzedMatch {
        AnalyzedMatch {
            match_id: Uuid::new_v4(),
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

    fn pattern(id: &str, rate: f32, status: PatternStatus) -> RecurringPattern {
        RecurringPattern {
            id: id.into(),
            label: "Dies too often".into(),
            description: "…".into(),
            occurrences: (rate * 20.0) as i64,
            measured: 20,
            rate,
            recent_rate: Some(rate),
            recent_measured: 10,
            status,
            status_label: status.label(),
            confidence: Confidence::Adequate,
            statement: "Dies too often in 12 of the 20 matches this could be measured in (60%)."
                .into(),
            examples: Vec::new(),
            first_seen_at: Some(Utc.timestamp_opt(1_700_000_000, 0).unwrap()),
            last_seen_at: Some(now()),
            first_detected_at: None,
        }
    }

    fn benchmark(metric: BenchmarkMetric, percentile: f32, median: f32) -> BenchmarkResult {
        BenchmarkResult {
            metric,
            label: metric.label(),
            higher_is_better: metric.higher_is_better(),
            player_value: 400.0,
            player_sample: 20,
            peer_median: Some(median),
            top_20_value: Some(median * 1.3),
            percentile: Some(percentile),
            gap_to_top_20: Some(100.0),
            peer_sample_size: None,
            confidence: Confidence::Adequate,
            segmented_by: vec![Segment::Hero],
            note: None,
        }
    }

    fn inputs<'a>(
        history: &'a [AnalyzedMatch],
        benchmarks: &'a [BenchmarkResult],
        patterns: &'a [RecurringPattern],
    ) -> SelectionInputs<'a> {
        SelectionInputs {
            history,
            benchmarks,
            patterns,
            now: now(),
        }
    }

    // --- Selection --------------------------------------------------------

    #[test]
    fn exactly_one_focus_is_returned() {
        let history = history(30);
        let patterns = [pattern("high_death_rate", 0.7, PatternStatus::Active)];
        let benchmarks = [benchmark(BenchmarkMetric::GoldPerMin, 20.0, 600.0)];

        let focus = select(
            &inputs(&history, &benchmarks, &patterns),
            FocusWeights::default(),
        )
        .expect("a focus");

        // Both a pattern and a benchmark gap exist; the player gets one thing.
        assert!(!focus.key.is_empty());
        assert_eq!(focus.status, FocusStatus::Active);
    }

    #[test]
    fn the_focus_is_not_simply_the_worst_number() {
        let history = history(30);
        // A big benchmark gap on a low-impact measure, against a mid-sized
        // pattern on a high-impact one.
        let benchmarks = [benchmark(BenchmarkMetric::GoldPerMin, 5.0, 900.0)];
        let patterns = [pattern("high_death_rate", 0.55, PatternStatus::Active)];

        let focus = select(
            &inputs(&history, &benchmarks, &patterns),
            FocusWeights::default(),
        )
        .unwrap();

        assert_eq!(
            focus.key, "pattern.high_death_rate",
            "impact and corroboration outrank a raw percentile"
        );
    }

    #[test]
    fn the_score_reports_the_parts_that_produced_it() {
        let history = history(30);
        let patterns = [pattern("high_death_rate", 0.7, PatternStatus::Active)];

        let focus = select(&inputs(&history, &[], &patterns), FocusWeights::default()).unwrap();
        let keys: Vec<&str> = focus.score_parts.iter().map(|p| p.key).collect();

        assert_eq!(
            keys,
            vec![
                "gap",
                "pattern",
                "recent",
                "impact",
                "confidence",
                "recency"
            ],
            "every input the spec names is reported"
        );
        let recomputed: f32 = focus
            .score_parts
            .iter()
            .map(|p| p.score * p.weight)
            .sum::<f32>()
            / FocusWeights::default().total();
        assert!((recomputed - focus.score).abs() < 0.01);
    }

    #[test]
    fn a_withheld_percentile_never_becomes_a_goal() {
        let history = history(30);
        let mut unranked = benchmark(BenchmarkMetric::GoldPerMin, 10.0, 600.0);
        unranked.percentile = None;
        unranked.confidence = Confidence::Insufficient;

        assert!(select(&inputs(&history, &[unranked], &[]), FocusWeights::default()).is_none());
    }

    #[test]
    fn a_healthy_player_gets_no_focus_rather_than_a_manufactured_one() {
        let history = history(30);
        let fine = [benchmark(BenchmarkMetric::GoldPerMin, 80.0, 400.0)];

        assert!(select(&inputs(&history, &fine, &[]), FocusWeights::default()).is_none());
    }

    #[test]
    fn a_resolved_pattern_is_not_a_candidate() {
        let history = history(30);
        let patterns = [pattern("high_death_rate", 0.7, PatternStatus::Resolved)];

        assert!(select(&inputs(&history, &[], &patterns), FocusWeights::default()).is_none());
    }

    #[test]
    fn an_improving_pattern_scores_below_an_active_one() {
        let history = history(30);
        let active = [pattern("high_death_rate", 0.7, PatternStatus::Active)];
        let improving = [pattern("high_death_rate", 0.7, PatternStatus::Improving)];

        let a = select(&inputs(&history, &[], &active), FocusWeights::default()).unwrap();
        let b = select(&inputs(&history, &[], &improving), FocusWeights::default()).unwrap();

        assert!(a.score > b.score);
    }

    #[test]
    fn a_pattern_target_sits_below_the_flagging_threshold() {
        let history = history(30);
        let patterns = [pattern("high_death_rate", 0.7, PatternStatus::Active)];

        let focus = select(&inputs(&history, &[], &patterns), FocusWeights::default()).unwrap();

        assert!(focus.target_value < patterns::MIN_RATE);
        assert_eq!(focus.baseline_value, 0.7);
        assert!(!focus.higher_is_better);
    }

    // --- Progress ---------------------------------------------------------

    #[test]
    fn the_series_buckets_history_oldest_first() {
        let mut history = history(30);
        // The oldest ten are terrible, the newest ten are clean.
        for m in history.iter_mut().take(10) {
            m.deaths_per_10 = 5.0;
        }

        let plotted = series(FocusMeasure::DeathsPer10, None, &history, Some(2.0));

        assert_eq!(plotted.points.len(), 3);
        assert_eq!(plotted.window, 10);
        assert!(
            plotted.points[0].value > plotted.points[2].value,
            "improvement reads left to right"
        );
        assert!(!plotted.higher_is_better);
    }

    #[test]
    fn a_bucket_nobody_can_measure_is_omitted_rather_than_zeroed() {
        // Nothing is parsed, so last hits at 10 cannot be read anywhere.
        let history = history(30);
        let plotted = series(FocusMeasure::LastHitsAt10, None, &history, None);

        assert!(plotted.points.is_empty());
    }

    #[test]
    fn a_pattern_series_plots_the_rate() {
        let mut history = history(20);
        for m in history.iter_mut().take(10) {
            m.deaths_per_10 = 5.0;
        }

        let plotted = series(
            FocusMeasure::PatternRate,
            Some("high_death_rate"),
            &history,
            Some(0.3),
        );

        assert_eq!(plotted.points.len(), 2);
        assert_eq!(plotted.points[0].value, 1.0, "the older bucket was all bad");
        assert_eq!(plotted.points[1].value, 0.0);
    }

    #[test]
    fn progress_runs_from_baseline_to_target_in_either_direction() {
        // Lower is better: 4.0 -> 2.0, currently 3.0.
        assert!((progress_toward(4.0, 2.0, 3.0) - 0.5).abs() < 0.01);
        // Higher is better: 400 -> 600, currently 500.
        assert!((progress_toward(400.0, 600.0, 500.0) - 0.5).abs() < 0.01);
        // Past the target clamps rather than exceeding 1.
        assert_eq!(progress_toward(4.0, 2.0, 1.0), 1.0);
        // Backsliding clamps at 0 rather than going negative.
        assert_eq!(progress_toward(4.0, 2.0, 6.0), 0.0);
    }

    #[test]
    fn a_degenerate_target_does_not_divide_by_zero() {
        assert_eq!(progress_toward(3.0, 3.0, 3.0), 1.0);
    }

    // --- Review -----------------------------------------------------------

    #[test]
    fn a_focus_is_kept_while_it_is_unfinished() {
        let history = history(30);
        let patterns = [pattern("high_death_rate", 0.7, PatternStatus::Active)];
        let mut focus = select(&inputs(&history, &[], &patterns), FocusWeights::default()).unwrap();
        // The history is clean, so the rate is 0 — force an unmet target.
        focus.target_value = -1.0;

        assert!(matches!(
            review(&focus, &inputs(&history, &[], &patterns)),
            FocusVerdict::Keep
        ));
    }

    #[test]
    fn a_focus_whose_target_is_met_is_achieved() {
        let history = history(30);
        let patterns = [pattern("high_death_rate", 0.7, PatternStatus::Active)];
        let focus = select(&inputs(&history, &[], &patterns), FocusWeights::default()).unwrap();

        // Every match in this history is clean, so the rate is already 0.
        assert!(matches!(
            review(&focus, &inputs(&history, &[], &patterns)),
            FocusVerdict::Achieved
        ));
    }

    #[test]
    fn a_focus_whose_pattern_vanished_is_retired_not_claimed_as_success() {
        let mut history = history(30);
        for m in history.iter_mut() {
            m.deaths_per_10 = 5.0; // the target is not met
        }
        let patterns = [pattern("high_death_rate", 0.7, PatternStatus::Active)];
        let focus = select(&inputs(&history, &[], &patterns), FocusWeights::default()).unwrap();

        // The detector no longer reports it at all.
        assert!(matches!(
            review(&focus, &inputs(&history, &[], &[])),
            FocusVerdict::Retired
        ));
    }
}
