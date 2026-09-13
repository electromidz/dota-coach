use serde::Serialize;
use uuid::Uuid;

/// Derived metrics for one match.
///
/// Every field is computed by `services::metrics` from stored facts. `None`
/// means the input was unavailable — an unparsed replay, or a match detail
/// that never arrived — never that the value was zero.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
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
#[derive(Debug, Clone, Serialize)]
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

/// Per-hero rollup.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
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
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RoleStats {
    pub role: String,
    pub matches: i64,
    pub wins: i64,
    pub win_rate: f32,
    pub avg_kda: f32,
    pub avg_gpm: f32,
}
