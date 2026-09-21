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

    /// The countable thing this metric measures per minute, when a whole-game
    /// total says more to a player than a rate does.
    ///
    /// "About two fewer deaths" is advice; "0.05 fewer deaths per minute" is
    /// arithmetic. Gold and damage stay rates because their whole-game totals
    /// are large, noisy and not something a player steers directly.
    pub fn countable_unit(self) -> Option<&'static str> {
        match self {
            BenchmarkMetric::KillsPerMin => Some("kills"),
            BenchmarkMetric::DeathsPerMin => Some("deaths"),
            BenchmarkMetric::AssistsPerMin => Some("assists"),
            BenchmarkMetric::LastHitsPerMin => Some("last hits"),
            _ => None,
        }
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

impl Segment {
    pub const ALL: [Segment; 4] = [
        Segment::Hero,
        Segment::Role,
        Segment::RankBracket,
        Segment::Patch,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Segment::Hero => "hero",
            Segment::Role => "role",
            Segment::RankBracket => "rank_bracket",
            Segment::Patch => "patch",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Segment::Hero => "Hero",
            Segment::Role => "Role",
            Segment::RankBracket => "Rank",
            Segment::Patch => "Patch",
        }
    }
}

/// Which bracket a comparison asked for, and which one it got.
///
/// These differ more often than is comfortable. OpenDota publishes no
/// distribution at all for some hero/bracket pairs — every percentile comes
/// back `null` — and an unranked player has no bracket to ask for. Both cases
/// fall back to the all-ranks distribution, which is a *different* peer group,
/// so the response carries the fact rather than letting the UI assume the
/// request was honoured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct ResolvedBracket {
    /// What the player's rank implied. `None` when they are unranked.
    pub requested: Option<crate::domain::hero::RankBracket>,
    /// What the distribution actually covers. `None` means all ranks.
    pub used: Option<crate::domain::hero::RankBracket>,
    pub label: &'static str,
    /// True when a bracket was asked for and a broader cohort came back.
    pub fell_back: bool,
}

impl ResolvedBracket {
    /// The bracket was asked for and the provider had data for it.
    pub fn exact(bracket: crate::domain::hero::RankBracket) -> Self {
        Self {
            requested: Some(bracket),
            used: Some(bracket),
            label: bracket.label(),
            fell_back: false,
        }
    }

    /// No bracket was asked for — an unranked player, or a caller that does not
    /// know the rank. Not a fallback: nothing was promised and nothing lost.
    pub fn all_ranks() -> Self {
        Self {
            requested: None,
            used: None,
            label: "All ranks",
            fell_back: false,
        }
    }

    /// A bracket was asked for and the provider had nothing there.
    pub fn fell_back_from(bracket: crate::domain::hero::RankBracket) -> Self {
        Self {
            requested: Some(bracket),
            used: None,
            label: "All ranks",
            fell_back: true,
        }
    }

    /// Resolve from a player's `rank_tier` before any provider call. The
    /// optimistic answer; the provider downgrades it if the data is missing.
    pub fn requested_for(rank_tier: Option<i32>) -> Self {
        match rank_tier.and_then(crate::domain::hero::RankBracket::from_rank_tier) {
            Some(bracket) => Self::exact(bracket),
            None => Self::all_ranks(),
        }
    }

    /// True when the peer group really is the player's own bracket.
    pub fn is_rank_segmented(self) -> bool {
        self.used.is_some()
    }
}

/// A dimension the product asked to compare on and could not.
///
/// The spec asks for hero/role/rank/patch segmentation. Rank is available —
/// OpenDota's `/benchmarks` honours a `bracket` parameter — but role and patch
/// are not, and rank itself degrades to all-ranks for an unranked player or a
/// hero the provider has no bracket data for.
///
/// Reporting the gap per dimension, with the reason, is the difference between
/// a comparison a user can weigh and one that quietly implies a peer group it
/// never had.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UnavailableSegment {
    pub segment: Segment,
    pub label: &'static str,
    /// Why it is missing, in words a user can act on. Owned rather than
    /// static: the rank reason names the bracket that was asked for.
    pub reason: String,
}

/// Which matches sit on each side of a comparison.
///
/// Both halves are stated because they are still not the same population. Ours
/// is exact and narrow — this player's eligible matches, in one role, on one
/// hero. The provider's is broader: rank now matches when a bracket was used,
/// but OpenDota does not publish which game modes or patch its distribution
/// covers, so this says that rather than guessing.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PopulationScope {
    /// What the player's own figures were averaged over.
    pub player: String,
    /// What the peer distribution covers, as far as the provider states it.
    pub peers: String,
    /// True only when both sides are known to describe the same population.
    /// Still false: rank lines up now, game mode and patch do not.
    pub comparable: bool,
    pub note: &'static str,
}

/// What a comparison was asked for, and what it could actually deliver.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BenchmarkContextInfo {
    pub hero_id: i32,
    pub hero_name: String,
    /// The role the player's own figures were restricted to, when there is one.
    pub role: Option<crate::domain::role::CoachableRole>,
    pub role_label: Option<&'static str>,
    /// The player's medal, as the provider reports it.
    pub rank_tier: Option<i32>,
    /// Which bracket the peer distribution actually covers, and whether that
    /// is the one the player's rank asked for.
    pub bracket: ResolvedBracket,
    /// Dimensions the product asks to compare on.
    pub requested: Vec<Segment>,
    /// Dimensions the peer distribution genuinely covers.
    pub segmented_by: Vec<Segment>,
    /// Requested minus delivered, each with its reason.
    pub unavailable: Vec<UnavailableSegment>,
    pub population: PopulationScope,
}

/// What the caller asked to be compared against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkContext {
    pub hero_id: i32,
    pub role: Option<String>,
    pub rank_tier: Option<i32>,
    /// A bracket the *caller* asked for, overriding the one `rank_tier`
    /// implies.
    ///
    /// Separate from `rank_tier` rather than derived from it because they are
    /// different facts: the rank is who the player is, this is who they asked
    /// to be measured against. A player looking at the bracket above their own
    /// is asking a progression question, and flattening the two would lose the
    /// ability to say which bracket the numbers on screen actually describe.
    pub bracket: Option<crate::domain::hero::RankBracket>,
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
