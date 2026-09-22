//! Deterministic rank calibration.
//!
//! Same contract as [`crate::services::metrics`]: every function here is a
//! pure function of stored facts. Nothing calls a provider, nothing calls a
//! model, and running it twice on the same rows gives the same answer.
//!
//! # The one thing this module is careful about
//!
//! Valve stopped publishing per-match MMR deltas, and no public source can
//! recover them. A site that draws a precise MMR number under every match is
//! inventing it. This module does not: the only real points on a rank
//! trajectory are the `rank_snapshots` rows the sync wrote down, and
//! everything between two of them is explicitly flagged [`TrajectoryPoint::
//! estimated`]. The flag is not decoration and must never be defaulted —
//! stripping it turns a disclosed model into a false claim about Valve's
//! number.
//!
//! # Population
//!
//! Every figure on the calibrating screen counts **ranked matches only**.
//!
//! That is deliberately narrower than [`crate::domain::eligibility`]'s
//! competitive population, which also admits unranked public lobbies. Those
//! are the right games to *coach* on — same drafts, same economy — but they do
//! not move a medal. Counting them would report a rank confidence the ladder
//! does not share, and put unranked results in a streak the player reads as
//! ladder form. One population for the whole screen, so no two panels disagree
//! about which games they are describing.

use chrono::{DateTime, Duration, Utc};

use crate::config::CalibrationConfig;
use crate::domain::calibration::{
    rank_label, Methodology, RankConfidence, RankSnapshot, RolePreference, Streak, StreakKind,
    TrajectoryPoint,
};
use crate::domain::eligibility::{self, lobby_type};
use crate::domain::r#match::Match;
use crate::services::metrics;

/// Bump when any formula below changes, so a stored or cached result stays
/// identifiable. Mirrors `METRICS_VERSION`.
pub const CALIBRATION_VERSION: i32 = 1;

/// Ceiling on how far one match's performance may stretch or shrink its
/// modeled MMR delta.
///
/// The delta is already a model; letting a single high-KDA game swing it by an
/// arbitrary multiple would dress a guess up as a measurement. A quarter either
/// way is enough to give the line a shape that tracks how the player actually
/// played without pretending to know more than that.
const MAX_PERFORMANCE_SWING: f32 = 0.25;

/// How far a modeled point may stray outside the band its two real endpoints
/// define, in rank tiers (one tier = one star).
///
/// A losing run in the middle of a segment that still ended higher should be
/// allowed to dip, or the dashed line is a straight ramp that tells the player
/// nothing. It should not be allowed to dip four medals.
const TRAJECTORY_OVERSHOOT: f32 = 1.0;

/// Approximate MMR behind one rank tier (one star) — a medal spans five stars
/// and roughly 750 MMR.
///
/// A fixed scale rather than one derived per segment. Deriving it (net modeled
/// MMR maps onto the tiers actually gained) collapses whenever a window's wins
/// and losses nearly cancel: the derived scale explodes and a gentle path
/// becomes a flat line with a cliff at the end. A constant keeps the shape
/// proportional to what was actually played, and the residual — whatever the
/// match results cannot account for — is spread evenly so the segment still
/// lands on the next real reading.
const MMR_PER_TIER: f32 = 150.0;

/// Ranked matches only — see the module note on population.
fn is_ranked(m: &Match) -> bool {
    m.lobby_type == Some(lobby_type::RANKED) && eligibility::is_eligible(m.game_mode, m.lobby_type)
}

/// The ranked subset, newest first.
fn ranked_newest_first(matches: &[Match]) -> Vec<&Match> {
    let mut ranked: Vec<&Match> = matches.iter().filter(|m| is_ranked(m)).collect();
    ranked.sort_by_key(|m| std::cmp::Reverse(m.started_at));
    ranked
}

/// How settled the player's rank is.
///
/// Counts ranked matches back from the most recent one, stopping at the first
/// gap longer than `decay_days`: Valve lets an idle account's certainty decay,
/// and games from before a six-month break are not evidence about where the
/// player sits today. A player who has not queued ranked within that window at
/// all reads as zero rather than as their pre-break total.
pub fn rank_confidence(
    matches: &[Match],
    config: &CalibrationConfig,
    now: DateTime<Utc>,
) -> RankConfidence {
    let ranked = ranked_newest_first(matches);
    let decay = Duration::days(config.decay_days);

    let counted = match ranked.first() {
        // Nothing ranked at all, or nothing recent enough to still count.
        None => 0,
        Some(newest) if now.signed_duration_since(newest.started_at) > decay => 0,
        Some(newest) => {
            let mut counted = 1_i64;
            let mut previous = newest.started_at;

            for m in ranked.iter().skip(1) {
                if previous.signed_duration_since(m.started_at) > decay {
                    break;
                }
                counted += 1;
                previous = m.started_at;
            }
            counted
        }
    };

    let confidence_pct = (counted as f32 * config.confidence_per_match_pct).min(100.0);

    RankConfidence {
        confidence_pct,
        matches_counted: counted,
        is_calibrated: confidence_pct >= config.confidence_threshold_pct,
    }
}

/// The current unbroken run of ranked results.
///
/// `kind` is `None` only when there is nothing to read. A zero-length streak
/// has no direction, and defaulting it to `Win` would render "Win 0".
pub fn streak(matches: &[Match]) -> Streak {
    let ranked = ranked_newest_first(matches);

    let Some(newest) = ranked.first() else {
        return Streak {
            count: 0,
            kind: None,
        };
    };

    let won = newest.won;
    let count = ranked.iter().take_while(|m| m.won == won).count() as i64;

    Streak {
        count,
        kind: Some(if won {
            StreakKind::Win
        } else {
            StreakKind::Loss
        }),
    }
}

/// Share of ranked matches played in each role, most-played first.
///
/// Reads `matches.role`, the label the sync already derived and stored. No
/// role detection happens here: a second implementation of that estimate would
/// eventually disagree with the one the rest of the product shows.
pub fn role_preference(matches: &[Match]) -> Vec<RolePreference> {
    let ranked = ranked_newest_first(matches);
    let total = ranked.len();
    if total == 0 {
        return Vec::new();
    }

    let mut counts: Vec<(String, i64)> = Vec::new();
    for m in &ranked {
        match counts.iter_mut().find(|(role, _)| role == &m.role) {
            Some((_, count)) => *count += 1,
            None => counts.push((m.role.clone(), 1)),
        }
    }

    // Most played first; ties broken by name so the order is stable across
    // requests rather than dependent on iteration order.
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    counts
        .into_iter()
        .map(|(role, matches)| RolePreference {
            role,
            pct: (matches as f32 / total as f32) * 100.0,
            matches,
        })
        .collect()
}

/// Rank over time: real snapshots, with a modeled path between them.
///
/// Real `rank_snapshots` rows are the only points carrying `estimated: false`.
/// Between each adjacent pair, the ranked matches played in that window shape
/// a path from the first tier to the second — win and loss base values from
/// configuration, stretched a little by how the player actually performed,
/// then scaled so the segment lands exactly on the next *real* reading.
///
/// Anchoring both ends to measurements is what keeps this a disclosed model
/// rather than a fabrication: the shape between two points is a guess, the
/// points themselves are not, and the line cannot drift somewhere the player
/// never was.
///
/// Two deliberate omissions:
///
///   * A snapshot with no rank (a private profile) **breaks** the chain. No
///     path is drawn across it, because nothing is known about that stretch.
///   * Matches after the newest snapshot produce nothing. There is no second
///     anchor yet, and a free-running tail is exactly the invented number this
///     module exists to avoid. The sync writes a snapshot a day, so the tail is
///     at most today.
pub fn trajectory(
    snapshots: &[RankSnapshot],
    matches: &[Match],
    config: &CalibrationConfig,
) -> Vec<TrajectoryPoint> {
    let ranked = {
        let mut r: Vec<&Match> = matches.iter().filter(|m| is_ranked(m)).collect();
        r.sort_by_key(|m| m.started_at);
        r
    };
    let median_kda = median_kda(&ranked);

    let mut points: Vec<TrajectoryPoint> = Vec::new();

    // Runs of consecutive readings that actually reported a rank. A null
    // reading ends the run it is in rather than being skipped over.
    for run in ranked_runs(snapshots) {
        for pair in run.windows(2) {
            let (from, to) = (pair[0], pair[1]);

            push_unique(
                &mut points,
                TrajectoryPoint {
                    rank_tier: from.0,
                    label: rank_label(i32::from(from.0)),
                    at: from.1,
                    estimated: false,
                },
            );

            let between: Vec<&&Match> = ranked
                .iter()
                .filter(|m| m.started_at > from.1 && m.started_at < to.1)
                .collect();

            for point in segment(from, to, &between, median_kda, config) {
                push_unique(&mut points, point);
            }
        }

        if let Some(last) = run.last() {
            push_unique(
                &mut points,
                TrajectoryPoint {
                    rank_tier: last.0,
                    label: rank_label(i32::from(last.0)),
                    at: last.1,
                    estimated: false,
                },
            );
        }
    }

    points
}

/// The disclosed model, straight from configuration.
pub fn methodology(config: &CalibrationConfig) -> Methodology {
    Methodology {
        win_base_mmr: config.win_base_mmr,
        loss_base_mmr: config.loss_base_mmr,
        confidence_per_match_pct: config.confidence_per_match_pct,
        confidence_threshold_pct: config.confidence_threshold_pct,
    }
}

// ---------------------------------------------------------------------------
// Trajectory internals
// ---------------------------------------------------------------------------

/// A reading that reported a rank: `(tier, when)`.
type Reading = (i16, DateTime<Utc>);

/// Split readings into runs of consecutive non-null ranks.
///
/// A null rank is not a missing value to be skipped — it is the measurement
/// "this profile reported no medal that day". Treating it as a break is what
/// keeps the chart from drawing a confident line across a stretch nobody
/// observed.
fn ranked_runs(snapshots: &[RankSnapshot]) -> Vec<Vec<Reading>> {
    let mut runs: Vec<Vec<Reading>> = Vec::new();
    let mut current: Vec<Reading> = Vec::new();

    for s in snapshots {
        match s.rank_tier {
            Some(tier) => current.push((tier, s.captured_at)),
            None => {
                if !current.is_empty() {
                    runs.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }

    runs
}

/// The modeled points strictly between two real readings.
fn segment(
    from: Reading,
    to: Reading,
    between: &[&&Match],
    median_kda: f32,
    config: &CalibrationConfig,
) -> Vec<TrajectoryPoint> {
    if between.is_empty() {
        return Vec::new();
    }

    // Cumulative modeled MMR across the window, one entry per match.
    let mut cumulative: Vec<f32> = Vec::with_capacity(between.len());
    let mut running = 0.0_f32;
    for m in between {
        running += modeled_delta(m, median_kda, config);
        cumulative.push(running);
    }

    let (start, end) = (from.0 as f32, to.0 as f32);
    let count = between.len();

    // The shape the played matches account for, in tiers.
    let shape: Vec<f32> = cumulative.iter().map(|c| c / MMR_PER_TIER).collect();

    // Whatever the results cannot explain. A real reading that moved further
    // than the modeled games account for — a calibration adjustment, a
    // recalibration, matches this account never synced — is still a fact, so
    // the difference is distributed evenly rather than argued with.
    let residual = end - start - shape.last().copied().unwrap_or(0.0);

    let low = start.min(end) - TRAJECTORY_OVERSHOOT;
    let high = start.max(end) + TRAJECTORY_OVERSHOOT;

    between
        .iter()
        .zip(shape.iter())
        .enumerate()
        .map(|(i, (m, offset))| {
            let corrected = start + offset + residual * ((i + 1) as f32 / count as f32);

            let rank_tier = corrected.clamp(low, high).round() as i16;

            TrajectoryPoint {
                rank_tier,
                label: rank_label(i32::from(rank_tier)),
                at: m.started_at,
                estimated: true,
            }
        })
        .collect()
}

/// One match's modeled MMR movement.
///
/// The base values are configuration and are *ours*, not Valve's — see
/// [`CalibrationConfig`]. The performance stretch is bounded by
/// [`MAX_PERFORMANCE_SWING`] and is measured against the player's own median,
/// not a peer distribution: a per-match peer percentile would need either a
/// provider call per historical match or a benchmark table that does not
/// exist, and neither is worth adding to give a disclosed estimate a more
/// elaborate shape.
fn modeled_delta(m: &Match, median_kda: f32, config: &CalibrationConfig) -> f32 {
    let base = if m.won {
        config.win_base_mmr
    } else {
        -config.loss_base_mmr
    };

    base * performance_factor(m, median_kda)
}

/// How this match compares to the player's own typical game, as a multiplier
/// in `[1 - MAX_PERFORMANCE_SWING, 1 + MAX_PERFORMANCE_SWING]`.
fn performance_factor(m: &Match, median_kda: f32) -> f32 {
    if median_kda <= 0.0 || !median_kda.is_finite() {
        return 1.0;
    }

    let ratio = metrics::kda(m.kills, m.deaths, m.assists) / median_kda;
    if !ratio.is_finite() {
        return 1.0;
    }

    ratio.clamp(1.0 - MAX_PERFORMANCE_SWING, 1.0 + MAX_PERFORMANCE_SWING)
}

/// The player's median KDA across the window, or 0 when there is nothing to
/// take a median of (which `performance_factor` reads as "no adjustment").
fn median_kda(matches: &[&Match]) -> f32 {
    if matches.is_empty() {
        return 0.0;
    }

    let mut values: Vec<f32> = matches
        .iter()
        .map(|m| metrics::kda(m.kills, m.deaths, m.assists))
        .collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

/// Adjacent segments share an endpoint — the `to` of one pair is the `from` of
/// the next — so the real reading between them would otherwise be emitted
/// twice.
fn push_unique(points: &mut Vec<TrajectoryPoint>, point: TrajectoryPoint) {
    if points.last().map(|p| p.at) == Some(point.at) {
        return;
    }
    points.push(point);
}

#[cfg(test)]
mod tests;
