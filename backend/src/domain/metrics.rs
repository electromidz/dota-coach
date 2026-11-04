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

/// One headline number with its comparison and a short history.
///
/// `value` is the latest window, `previous` the window before it, and `delta`
/// is `value - previous` (percentage points for a rate). `None` when a window
/// had no matches; a delta is never invented from one side.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct KpiTrend {
    pub value: Option<f32>,
    pub previous: Option<f32>,
    pub delta: Option<f32>,
    /// Oldest first, for a sparkline.
    pub series: Vec<f32>,
}

/// The Overview's KPI row, computed over eligible matches only.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct OverviewTrends {
    /// Matches per window for the per-game KPIs ("last 20 vs the 20 before").
    pub window: i64,
    /// Win rate, 0-100. The series is a rolling win rate over `rolling` games.
    pub win_rate: KpiTrend,
    pub kda: KpiTrend,
    pub gpm: KpiTrend,
    /// Eligible matches in the last 30 days against the 30 before. The series
    /// is matches per week, last 8 weeks.
    pub matches_30d: KpiTrend,
    /// Games behind each point of the rolling win rate.
    pub rolling: i64,
}

/// The Matches page's summary strip, for whatever filter is applied.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct MatchListSummary {
    /// Matches the summary was computed over.
    pub matches: i64,
    /// True when the filter matched more than the summary reads.
    pub capped: bool,
    pub wins: i64,
    /// 0-100.
    pub win_rate: Option<f32>,
    pub avg_kda: Option<f32>,
    pub avg_gpm: Option<f32>,
    /// The newest results, oldest first.
    pub recent_results: Vec<bool>,
    /// Rolling win rate over `rolling` games, oldest first.
    pub rolling_win_rate: Vec<f32>,
    pub rolling: i64,
    /// The highest-GPM match in the filter, for a "best GPM" badge.
    pub best_gpm_match_id: Option<Uuid>,
}
