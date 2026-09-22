//! Rank calibration: where a player actually stands, and how sure of it we are.
//!
//! At the centre of it is the rank snapshot — a single observation of a Dota
//! account's medal at a point in time. It is deliberately a *record of a
//! reading*, not a derived figure: `rank_tier` is whatever OpenDota reported
//! when the sync ran, and nothing here smooths, interpolates or carries a
//! value forward.
//!
//! That matters for what gets built on top. A trajectory drawn between two
//! snapshots is a model, because Valve stopped publishing per-match MMR deltas
//! and no public source can recover them. Snapshots are the only points on that
//! chart that are real, so they stay unambiguously distinguishable from
//! anything estimated later — see [`TrajectoryPoint::estimated`].

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

/// One reading of an account's rank.
///
/// Both rank fields are nullable, and a null is a measurement rather than a
/// hole: a private profile reports no medal, and recording "we looked and
/// there was nothing" is the honest row. A gap in the history is correct where
/// a repeated previous value would be a fabrication.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct RankSnapshot {
    /// Medal and stars encoded as `medal * 10 + stars` — 45 is Archon 5.
    /// `SMALLINT` in the database; the rest of the codebase already speaks
    /// `i32` for this, so the width is widened on read rather than leaking a
    /// second spelling of the same value.
    pub rank_tier: Option<i16>,
    /// Immortal ladder position, when the player is on it. `None` for everyone
    /// below Immortal and for Immortals outside the published leaderboard.
    pub leaderboard_rank: Option<i32>,
    pub captured_at: DateTime<Utc>,
}

/// How settled the player's rank is, on Valve's 0→100% scale.
///
/// Counted from **ranked** matches only. That is narrower than the
/// `eligibility` module's competitive population, which also admits unranked
/// public lobbies: those are the right games to *coach* on, but they do not
/// move a medal, so counting them would report a confidence the ladder does
/// not share.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, ToSchema)]
pub struct RankConfidence {
    pub confidence_pct: f32,
    /// Ranked matches behind that percentage. Published so a low confidence
    /// reads as "play more", not as a verdict on the player.
    pub matches_counted: i64,
    pub is_calibrated: bool,
}

/// One point on the rank trajectory.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct TrajectoryPoint {
    pub rank_tier: i16,
    /// `"Archon 5"`. Named here rather than in each client: the medal table is
    /// one thing, and a copy of it in TypeScript would drift the first time
    /// Valve adds a rank.
    pub label: Option<String>,
    pub at: DateTime<Utc>,
    /// `false` only for a point that came from a real `rank_snapshots` row.
    ///
    /// This flag is the honesty guarantee of the whole feature. Valve stopped
    /// publishing per-match MMR, so nothing between two snapshots is known —
    /// it is modeled. A consumer that renders an estimated point the same way
    /// it renders a measured one is presenting a guess as Valve's number, so
    /// the field is never defaulted and never dropped.
    pub estimated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StreakKind {
    Win,
    Loss,
}

/// The current unbroken run of results, newest-first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct Streak {
    pub count: i64,
    /// `None` only when there are no matches to read at all — a streak of
    /// zero has no direction, and inventing one would make "Win 0" appear.
    pub kind: Option<StreakKind>,
}

/// Share of matches played in one role.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct RolePreference {
    pub role: String,
    pub pct: f32,
    pub matches: i64,
}

/// One match on the momentum curve.
///
/// `cumulative` is a **relative** figure: the curve starts at zero and shows
/// how far the modeled model has moved since, never an absolute MMR. That
/// distinction is the whole reason this is shippable. The win/loss sequence it
/// is built from is real; the per-match value is ours (see
/// [`crate::config::CalibrationConfig`]), and Valve publishes nothing that
/// could confirm or refute it. Printing "your MMR is 4230" from these numbers
/// would be a specific claim about a figure nobody outside Valve can see —
/// printing "+85 over your last 20 ranked games" is arithmetic over results
/// the player actually got.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct MomentumPoint {
    /// 1-based position in the window, oldest first — the chart's x axis.
    pub index: i64,
    pub match_id: i64,
    pub hero_name: String,
    pub won: bool,
    /// This match's modeled movement.
    pub delta: f32,
    /// Running total since the start of the window. Starts from zero.
    pub cumulative: f32,
    pub started_at: DateTime<Utc>,
}

/// Modeled movement across a recent window of ranked matches.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct Momentum {
    /// Oldest first. Empty when the window holds no ranked matches.
    pub points: Vec<MomentumPoint>,
    /// Where the curve ends: the net modeled movement across the window.
    pub net: f32,
    pub wins: i64,
    pub losses: i64,
    /// How many matches the window was allowed to hold, so a short curve reads
    /// as "not enough games yet" rather than as a flat stretch.
    pub window: i64,
}

/// The player's current medal, as the game spells it.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct EstablishedRank {
    /// `medal * 10 + stars`, straight from the provider. `None` for an
    /// unranked or private account.
    pub rank_tier: Option<i32>,
    /// `"Archon 5"`, or `None` when there is no tier to name. Never a
    /// placeholder: an account with no medal has no medal, and inventing
    /// "Unranked 0" would read as a rank.
    pub label: Option<String>,
    pub leaderboard_rank: Option<i32>,
    /// The MMR band the medal implies. An **estimate** — see [`MmrEstimate`].
    /// `None` when there is no medal to derive one from.
    pub mmr: Option<MmrEstimate>,
}

/// MMR per star. A medal spans five of them, so 770 MMR per medal.
///
/// From the published medal/MMR table — the same one every rank site uses,
/// and the reason a *band* is derivable where a per-match delta is not: Valve
/// does not say what a given win was worth, but the boundaries a medal sits
/// between are well established.
pub const MMR_PER_STAR: i32 = 154;

/// Where the bottom of each medal starts, Herald through Immortal.
const MEDAL_FLOORS: [i32; 8] = [0, 770, 1540, 2310, 3080, 3850, 4620, 5421];

/// The MMR range a medal implies.
///
/// An estimate, and labelled as one everywhere it is rendered. What makes it
/// defensible is that it is a *range*: the medal is a real reading from Valve,
/// and the band is what that reading actually pins down. `midpoint` is offered
/// for a headline figure, but it is the middle of a 154-point window, not a
/// measurement of where inside it the player sits — nothing public can say
/// that.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, ToSchema)]
pub struct MmrEstimate {
    pub low: i32,
    /// `None` for Immortal, which has no ceiling.
    pub high: Option<i32>,
    pub midpoint: i32,
}

/// The MMR band a `rank_tier` implies.
///
/// `None` for an unranked or unrecognised tier — a player with no medal has no
/// band, and a midpoint invented for them would be exactly the fabrication
/// this screen otherwise avoids.
pub fn estimate_mmr(rank_tier: i32) -> Option<MmrEstimate> {
    let bracket = crate::domain::hero::RankBracket::from_rank_tier(rank_tier)?;
    let floor = *MEDAL_FLOORS.get((bracket.index() - 1) as usize)?;

    // Immortal is reported as a flat 80 with no stars, and has no ceiling.
    if bracket == crate::domain::hero::RankBracket::Immortal {
        return Some(MmrEstimate {
            low: floor,
            high: None,
            midpoint: floor,
        });
    }

    // Stars are 1-5; a medal reported without one describes the whole medal.
    let stars = rank_tier % 10;
    let (low, high) = if (1..=5).contains(&stars) {
        let low = floor + (stars - 1) * MMR_PER_STAR;
        (low, low + MMR_PER_STAR - 1)
    } else {
        (floor, floor + MMR_PER_STAR * 5 - 1)
    };

    Some(MmrEstimate {
        low,
        high: Some(high),
        midpoint: (low + high) / 2,
    })
}

/// `45` -> `"Archon 5"`.
///
/// `rank_tier` packs the medal in the tens digit and the stars in the units.
/// Immortal is the exception the encoding does not spell out: it has no stars,
/// and Valve reports it as a flat `80`, so appending one would invent a
/// distinction the game does not make.
pub fn rank_label(rank_tier: i32) -> Option<String> {
    let bracket = crate::domain::hero::RankBracket::from_rank_tier(rank_tier)?;

    if bracket == crate::domain::hero::RankBracket::Immortal {
        return Some(bracket.label().to_string());
    }

    match rank_tier % 10 {
        // A medal with no star reported. Naming the medal alone is still true.
        0 => Some(bracket.label().to_string()),
        stars => Some(format!("{} {stars}", bracket.label())),
    }
}

/// Where the player's measured numbers sit against one bracket's peers.
///
/// A **measurement**, not a prediction. `percentile` is the player's own
/// figures compared against the real distribution OpenDota reports for that
/// medal — 62 means "better than 62% of Archon players on these metrics", and
/// nothing more. It is deliberately not a probability of calibrating into the
/// bracket: that would need a model fitted against real calibration outcomes,
/// which Valve does not publish and this product has no data to fit.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct BracketFit {
    pub bracket: crate::domain::hero::RankBracket,
    pub label: &'static str,
    /// 0-100, direction-corrected and averaged across the metrics that had a
    /// usable comparison. `None` when the provider had nothing for this
    /// bracket, or the sample was too thin to rank against — never defaulted.
    pub percentile: Option<f32>,
    /// How many metrics contributed. A fit built from one metric is a weaker
    /// statement than one built from four, and the caller can say so.
    pub metrics_used: i64,
    /// The peer sample the provider reported, where it reported one.
    pub sample_size: Option<i64>,
    /// The bracket this player's own medal falls in.
    pub is_player_bracket: bool,
}

/// How much the player resembles one bracket, as a share of 100.
///
/// The same underlying measurement as [`BracketFit`], turned into the shape a
/// distribution chart needs. A percentile cannot be sorted descending without
/// inverting its meaning — beating 98% of Heralds puts Herald *first* in a
/// sorted list while meaning the player is nowhere near Herald. So resemblance
/// scores each bracket by how close the player sits to the middle of it, and
/// those shares are what get sorted and drawn.
///
/// Still not a probability of calibrating into the bracket. It is "your
/// numbers look like this bracket's players, by this share of the total
/// resemblance across all brackets" — a normalised similarity over real peer
/// distributions, with nothing fitted against calibration outcomes because
/// Valve publishes none.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct BracketResemblance {
    pub bracket: crate::domain::hero::RankBracket,
    pub label: &'static str,
    /// 0-100. The shares across all placed brackets sum to 100.
    pub percentage: f32,
    /// The strongest match, for a chart that calls one bar out.
    pub is_highest: bool,
    pub is_player_bracket: bool,
}

/// How steady the player's per-match output is.
///
/// Deliberately `Option` at the call site rather than a default: a player with
/// four games has no measurable consistency, and answering "75%" for them —
/// as some tools do — is a fabricated measurement wearing a real one's
/// clothes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, ToSchema)]
pub struct Consistency {
    /// 0-100. High means their good games and bad games look alike.
    pub percentage: f32,
    /// Matches behind it.
    pub matches: i64,
}

/// The player's performance placed against every medal.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RankDistribution {
    /// The hero these comparisons are drawn from — peer distributions are
    /// per-hero, so this is not a career-wide statement and does not pretend
    /// to be.
    pub hero_id: i32,
    pub hero_name: String,
    /// Matches behind the player's own figures.
    pub sample: i64,
    /// One entry per medal, Herald first.
    pub fits: Vec<BracketFit>,
    /// The medal whose peers this player most resembles: the bracket whose
    /// percentile sits closest to the middle of the pack. `None` when nothing
    /// could be compared.
    pub closest: Option<crate::domain::hero::RankBracket>,
    /// The same placements as shares, sorted strongest first — what the
    /// distribution chart draws.
    pub resemblance: Vec<BracketResemblance>,
    /// Per-metric placement against the player's **own** bracket: where they
    /// sit among the peers they are actually ranked with.
    pub own_bracket_metrics: Vec<crate::domain::benchmark::BenchmarkResult>,
    /// `None` when there are too few matches to measure it.
    pub consistency: Option<Consistency>,
    /// Why a comparison is thin or missing, when it is.
    pub note: Option<String>,
}

/// The disclosed model, shipped with every response.
///
/// Present so no consumer has to hardcode the formula it tells the player
/// about: change the environment variable and the number in the UI changes
/// with it. A methodology note that has drifted from the model it describes is
/// worse than none.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, ToSchema)]
pub struct Methodology {
    pub win_base_mmr: f32,
    pub loss_base_mmr: f32,
    pub confidence_per_match_pct: f32,
    pub confidence_threshold_pct: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rank_tier_is_named_the_way_the_game_names_it() {
        assert_eq!(rank_label(45).as_deref(), Some("Archon 5"));
        assert_eq!(rank_label(11).as_deref(), Some("Herald 1"));
        assert_eq!(rank_label(55).as_deref(), Some("Legend 5"));
    }

    #[test]
    fn immortal_has_no_stars_to_name() {
        assert_eq!(
            rank_label(80).as_deref(),
            Some("Immortal"),
            "Valve reports a flat 80; appending a star would invent a distinction"
        );
    }

    #[test]
    fn a_medal_reported_without_stars_is_still_named() {
        assert_eq!(rank_label(40).as_deref(), Some("Archon"));
    }

    #[test]
    fn an_unrankable_tier_gets_no_invented_label() {
        assert_eq!(rank_label(0), None, "no medal is not 'Unranked 0'");
        assert_eq!(rank_label(99), None);
    }
}

#[cfg(test)]
mod mmr_tests {
    use super::*;

    /// Against the published table: Archon spans 2310-3079, five stars of 154.
    #[test]
    fn a_medal_and_star_map_to_their_published_band() {
        let archon5 = estimate_mmr(45).expect("Archon 5 has a band");
        assert_eq!(archon5.low, 2926);
        assert_eq!(archon5.high, Some(3079), "the top of Archon");

        let archon1 = estimate_mmr(41).expect("Archon 1 has a band");
        assert_eq!(archon1.low, 2310, "the bottom of Archon");

        let herald1 = estimate_mmr(11).expect("Herald 1 has a band");
        assert_eq!(herald1.low, 0);
    }

    #[test]
    fn the_midpoint_sits_inside_its_own_band() {
        for tier in [11, 25, 33, 45, 52, 64, 71] {
            let e = estimate_mmr(tier).expect("a real tier has a band");
            assert!(
                e.midpoint >= e.low && e.midpoint <= e.high.unwrap(),
                "tier {tier}: {} not within {}..{:?}",
                e.midpoint,
                e.low,
                e.high
            );
        }
    }

    #[test]
    fn stars_step_by_one_star_of_mmr() {
        let a = estimate_mmr(41).unwrap();
        let b = estimate_mmr(42).unwrap();

        assert_eq!(b.low - a.low, MMR_PER_STAR);
    }

    /// Immortal has no stars and no ceiling. Inventing an upper bound would
    /// put a number on the one rank that genuinely has none.
    #[test]
    fn immortal_has_a_floor_and_no_ceiling() {
        let e = estimate_mmr(80).expect("Immortal has a floor");

        assert_eq!(e.low, 5421);
        assert_eq!(e.high, None);
    }

    /// A medal reported without a star still names a band — the whole medal.
    #[test]
    fn a_starless_medal_spans_its_whole_range() {
        let e = estimate_mmr(40).expect("Archon with no star");

        assert_eq!(e.low, 2310);
        assert_eq!(e.high, Some(3079));
    }

    #[test]
    fn no_medal_is_no_band_rather_than_a_guess() {
        assert_eq!(estimate_mmr(0), None);
        assert_eq!(estimate_mmr(99), None);
    }

    /// The bands must tile the ladder without gaps or overlaps, or a player
    /// sitting on a boundary belongs to two medals at once.
    #[test]
    fn consecutive_medals_abut_exactly() {
        for (lower, upper) in [(15, 21), (25, 31), (35, 41), (45, 51), (55, 61), (65, 71)] {
            let below = estimate_mmr(lower).unwrap();
            let above = estimate_mmr(upper).unwrap();

            assert_eq!(
                below.high.unwrap() + 1,
                above.low,
                "gap between tier {lower} and {upper}"
            );
        }
    }
}
