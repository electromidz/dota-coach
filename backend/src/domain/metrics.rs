use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// Derived metrics for one match.
///
/// Every field is computed by `services::metrics` from stored facts. `None`
/// means the input was unavailable — an unparsed replay, or a match detail
/// that never arrived — never that the value was zero.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct MatchMetrics {
    pub match_id: Uuid,
    /// Which formula set produced these numbers.
    pub metrics_version: i32,

    pub kda: f32,
    pub kills_per_10: f32,
    pub deaths_per_10: f32,
    pub assists_per_10: f32,
    pub last_hits_per_min: f32,
    pub hero_damage_per_min: Option<f32>,
    pub tower_damage_per_min: Option<f32>,

    /// 0-1 share of the team's kills. Needs team totals.
    pub kill_participation: Option<f32>,
    /// Needs a parsed replay.
    pub gold_advantage_at_10: Option<f32>,
}

/// Aggregate over a set of matches.
///
/// Counts are reported alongside every average so a caller can tell a solid
/// number from one built on two games — the spec forbids presenting thin data
/// as though it were reliable.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PlayerStats {
    pub matches: i64,
    pub wins: i64,
    pub losses: i64,
    /// `None` when there are no matches to divide by.
    pub win_rate: Option<f32>,

    pub avg_kda: Option<f32>,
    pub avg_gpm: Option<f32>,
    pub avg_xpm: Option<f32>,
    pub avg_last_hits: Option<f32>,
    pub avg_deaths_per_10: Option<f32>,
    pub avg_kills_per_10: Option<f32>,
    pub avg_hero_damage: Option<f32>,

    /// Averaged only over matches that actually carry the input.
    pub avg_kill_participation: Option<f32>,
    pub kill_participation_sample: i64,

    /// How many of these matches came from a parsed replay, which is what
    /// gates the time-sliced metrics.
    pub parsed_matches: i64,
}

/// The player's own typical figures for one hero, as stored.
///
/// The yardstick behind a match rating: what this account usually does, so a
/// single game can be judged against it instead of against an absolute scale
/// nobody's account matches.
///
/// Turbo is a separate row rather than folded in. Its economy curve is a
/// different game, and rating a Turbo stomp against an All Pick average would
/// report a figure neither population supports.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MatchRatingBaseline {
    /// `None` on the player-wide fallback row, used when one hero has too few
    /// matches behind it to be a yardstick of its own.
    pub hero_id: Option<i32>,
    pub turbo: bool,
    pub sample: i64,
    /// The median rather than the mean: one 30-kill game should not move what
    /// counts as a normal game for this player.
    pub median_kda: Option<f32>,
    pub avg_gpm: Option<f32>,
    pub avg_xpm: Option<f32>,
    /// Averaged only over matches that carry hero damage at all — most public
    /// matches arrive without it.
    pub avg_hero_damage_per_min: Option<f32>,
}

/// Per-hero rollup.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct HeroStats {
    pub hero_id: i32,
    pub hero_name: String,
    pub matches: i64,
    pub wins: i64,
    pub win_rate: f32,
    pub avg_kda: f32,
    pub avg_gpm: f32,
    pub last_played_at: chrono::DateTime<chrono::Utc>,
}

/// Per-role rollup.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct RoleStats {
    pub role: String,
    pub matches: i64,
    pub wins: i64,
    pub win_rate: f32,
    pub avg_kda: f32,
    pub avg_gpm: f32,
}
