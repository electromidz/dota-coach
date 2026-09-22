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
