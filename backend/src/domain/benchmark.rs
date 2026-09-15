use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A metric that can be benchmarked against other players.
///
/// Each variant knows its provider key and whether more is better — the second
/// matters more than it looks: sitting in the 90th percentile for deaths is a
/// bad result, and a percentile that ignored direction would report it as a
/// strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkMetric {
    GoldPerMin,
    XpPerMin,
    LastHitsPerMin,
    KillsPerMin,
    DeathsPerMin,
    AssistsPerMin,
    HeroDamagePerMin,
    TowerDamage,
}

impl BenchmarkMetric {
    /// Every metric the engine supports, in dashboard order.
    pub const ALL: [BenchmarkMetric; 8] = [
        BenchmarkMetric::GoldPerMin,
        BenchmarkMetric::XpPerMin,
        BenchmarkMetric::LastHitsPerMin,
        BenchmarkMetric::KillsPerMin,
        BenchmarkMetric::AssistsPerMin,
        BenchmarkMetric::DeathsPerMin,
        BenchmarkMetric::HeroDamagePerMin,
        BenchmarkMetric::TowerDamage,
    ];

    /// The key this metric carries in the provider's payload.
    pub fn provider_key(self) -> &'static str {
        match self {
            BenchmarkMetric::GoldPerMin => "gold_per_min",
            BenchmarkMetric::XpPerMin => "xp_per_min",
            BenchmarkMetric::LastHitsPerMin => "last_hits_per_min",
            BenchmarkMetric::KillsPerMin => "kills_per_min",
            BenchmarkMetric::DeathsPerMin => "deaths_per_min",
            BenchmarkMetric::AssistsPerMin => "assists_per_min",
            BenchmarkMetric::HeroDamagePerMin => "hero_damage_per_min",
            BenchmarkMetric::TowerDamage => "tower_damage",
        }
    }

    /// URL-safe identifier, used by `GET /api/benchmark/:metric`.
    pub fn slug(self) -> &'static str {
        self.provider_key()
    }

    pub fn parse(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.slug() == slug)
    }

    pub fn label(self) -> &'static str {
        match self {
            BenchmarkMetric::GoldPerMin => "Gold per minute",
            BenchmarkMetric::XpPerMin => "XP per minute",
            BenchmarkMetric::LastHitsPerMin => "Last hits per minute",
            BenchmarkMetric::KillsPerMin => "Kills per minute",
            BenchmarkMetric::DeathsPerMin => "Deaths per minute",
            BenchmarkMetric::AssistsPerMin => "Assists per minute",
            BenchmarkMetric::HeroDamagePerMin => "Hero damage per minute",
            BenchmarkMetric::TowerDamage => "Tower damage",
        }
    }

    /// False when a lower value is the better result.
    pub fn higher_is_better(self) -> bool {
        !matches!(self, BenchmarkMetric::DeathsPerMin)
    }
}

/// Which dimensions a benchmark was actually segmented on.
///
/// The spec asks for hero/role/rank/patch segmentation, but a provider may not
/// offer all four. Reporting what was *really* used stops the UI claiming a
/// rank-aware comparison that the data behind it cannot support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Segment {
    Hero,
    Role,
    RankBracket,
    Patch,
}

/// What the caller asked to be compared against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkContext {
    pub hero_id: i32,
    pub role: Option<String>,
    pub rank_tier: Option<i32>,
    pub patch: Option<String>,
}

/// How much weight the player's own figure can bear.
///
/// This is about the *player's* sample, not the peer group: an average over
/// three games is noise however good the reference distribution is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Below the floor. No percentile is claimed.
    Insufficient,
    /// Enough to show, with the sample size beside it.
    Low,
    Adequate,
}

/// One metric, compared.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BenchmarkResult {
    pub metric: BenchmarkMetric,
    pub label: &'static str,
    pub higher_is_better: bool,

    pub player_value: f32,
    /// How many of the player's matches went into `player_value`.
    pub player_sample: i64,

    /// The provider's 50th percentile. Named a median because that is what it
    /// is — calling it an average would misdescribe the source.
    pub peer_median: Option<f32>,
    /// The 80th percentile: the "top 20%" line.
    pub top_20_value: Option<f32>,
    /// 0-100, direction-corrected. `None` when confidence is insufficient.
    pub percentile: Option<f32>,
    /// Distance to `top_20_value`, signed so positive always means "work to do".
    pub gap_to_top_20: Option<f32>,

    /// The peer sample behind the distribution, when the provider reports one.
    pub peer_sample_size: Option<i64>,
    pub confidence: Confidence,
    /// Dimensions genuinely segmented on — not the ones requested.
    pub segmented_by: Vec<Segment>,
    /// Present when a value is withheld, so the UI can explain the gap.
    pub note: Option<String>,
}

/// A percentile bucket from the provider: "p80 = 684".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bucket {
    /// 0-1, as the provider expresses it.
    pub percentile: f32,
    pub value: f32,
}
