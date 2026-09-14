//! Hero Intelligence domain types.
//!
//! Three things live here, and the separation matters:
//!
//!   - **Hero meta** — what the wider player base is doing. Comes from a
//!     provider, is identical for every user in a bracket.
//!   - **Hero pool** — what *this* player has actually done. Comes from their
//!     own matches, never from a provider.
//!   - **Hero fit** — the two combined, deterministically.
//!
//! Nothing here knows about OpenDota, STRATZ or Dotabuff.

use serde::{Deserialize, Serialize};

use crate::domain::benchmark::Confidence;

/// A Dota rank bracket.
///
/// OpenDota encodes `rank_tier` as `medal * 10 + stars`, so the tens digit is
/// the bracket and the ones digit is the star inside it. Only the bracket is
/// meaningful for meta segmentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankBracket {
    Herald,
    Guardian,
    Crusader,
    Archon,
    Legend,
    Ancient,
    Divine,
    Immortal,
}

impl RankBracket {
    pub const ALL: [RankBracket; 8] = [
        RankBracket::Herald,
        RankBracket::Guardian,
        RankBracket::Crusader,
        RankBracket::Archon,
        RankBracket::Legend,
        RankBracket::Ancient,
        RankBracket::Divine,
        RankBracket::Immortal,
    ];

    /// `None` for an unranked or absent tier — an unranked player gets the
    /// all-bracket meta rather than being silently filed under Herald.
    pub fn from_rank_tier(rank_tier: i32) -> Option<Self> {
        Self::from_index(rank_tier / 10)
    }

    /// 1-8, the numbering every provider seen so far uses.
    pub fn from_index(index: i32) -> Option<Self> {
        Self::ALL
            .get(usize::try_from(index).ok()?.checked_sub(1)?)
            .copied()
    }

    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|b| *b == self).unwrap() as i32 + 1
    }

    pub fn label(self) -> &'static str {
        match self {
            RankBracket::Herald => "Herald",
            RankBracket::Guardian => "Guardian",
            RankBracket::Crusader => "Crusader",
            RankBracket::Archon => "Archon",
            RankBracket::Legend => "Legend",
            RankBracket::Ancient => "Ancient",
            RankBracket::Divine => "Divine",
            RankBracket::Immortal => "Immortal",
        }
    }
}

/// The window of play a meta figure describes.
///
/// Providers differ in what they can honour: OpenDota publishes one rolling
/// public-match window and cannot be asked for another, so it reports what it
/// actually used rather than echoing the request back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeWindow {
    /// The provider's own recent window, whatever length it publishes.
    Recent,
    /// Everything the provider has for the current data set.
    AllTime,
}

/// What meta was asked for. What was *delivered* is described by
/// [`HeroMetaSet::segmented_by`](crate::services::hero_meta::HeroMetaSet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroMetaContext {
    pub bracket: Option<RankBracket>,
    pub role: Option<String>,
    pub patch: Option<String>,
    pub time_window: TimeWindow,
}

impl HeroMetaContext {
    pub fn for_bracket(bracket: Option<RankBracket>) -> Self {
        Self {
            bracket,
            role: None,
            patch: None,
            time_window: TimeWindow::Recent,
        }
    }
}

/// One hero, as the wider player base is currently playing it.
///
/// Every field is something a provider genuinely reports. Notably absent: ban
/// rate. The only ban figure OpenDota publishes comes from professional
/// matches, which is a different population from the pubs this product coaches
/// — carrying it here would invite it into a pub meta score it has no business
/// in.
#[derive(Debug, Clone, Serialize)]
pub struct HeroMeta {
    pub hero_id: i32,
    pub hero_name: String,
    /// Provider-supplied role tags ("Carry", "Support", …). Not the player's
    /// role — that comes from their own matches.
    pub roles: Vec<String>,

    /// Matches this hero was picked in. This *is* the sample size behind
    /// `win_rate`; there is no second, larger cohort hiding behind it.
    pub picks: i64,
    pub wins: i64,
    pub win_rate: f32,
    /// Share of all picks in the same cohort, 0-1.
    pub pick_rate: f32,
    /// Signed change in win rate across the provider's trend window. `None`
    /// when the provider publishes no trend.
    pub trend: Option<f32>,

    /// 0-100. Deterministic, and deliberately not the win rate — see
    /// `services::hero_meta::strength`.
    pub meta_strength: f32,
    /// Which bracket these figures describe, when they are bracket-segmented.
    pub bracket: Option<RankBracket>,
}

/// How a hero sits in the player's own repertoire.
///
/// Based on played matches and results only. A hero is never labelled from a
/// provider's opinion of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeroTier {
    /// Deep experience and results above the player's own baseline.
    Signature,
    /// Played enough to be reliable, results around the baseline.
    Comfort,
    /// Too little history to judge — a hero they are still learning.
    Stretch,
    /// Real history, results below the baseline.
    Risk,
}

impl HeroTier {
    pub fn label(self) -> &'static str {
        match self {
            HeroTier::Signature => "Signature",
            HeroTier::Comfort => "Comfort",
            HeroTier::Stretch => "Stretch",
            HeroTier::Risk => "Risk",
        }
    }
}

/// One hero in the player's pool.
#[derive(Debug, Clone, Serialize)]
pub struct HeroPoolEntry {
    pub hero_id: i32,
    pub hero_name: String,
    /// The player's most frequent role on this hero.
    pub role: String,

    pub matches: i64,
    pub wins: i64,
    pub losses: i64,
    pub win_rate: f32,

    /// Matches inside the recent window, and the win rate across them. The
    /// count is reported so a 100% from two games is visibly two games.
    pub recent_matches: i64,
    pub recent_win_rate: Option<f32>,

    pub avg_kda: f32,
    pub avg_gpm: f32,
    pub last_played_at: chrono::DateTime<chrono::Utc>,

    pub tier: HeroTier,
    pub tier_label: &'static str,
    /// How much weight this hero's figures can bear.
    pub confidence: Confidence,
}

/// A weighted input to the fit score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FitComponent {
    PlayerPerformance,
    MetaStrength,
    Experience,
    Benchmark,
    RecentForm,
}

impl FitComponent {
    pub fn label(self) -> &'static str {
        match self {
            FitComponent::PlayerPerformance => "Your performance",
            FitComponent::MetaStrength => "Meta strength",
            FitComponent::Experience => "Experience",
            FitComponent::Benchmark => "Benchmark",
            FitComponent::RecentForm => "Recent form",
        }
    }
}

/// One component's contribution, kept alongside the score so the number is
/// always explainable — the spec forbids an opaque fit score.
#[derive(Debug, Clone, Serialize)]
pub struct FitPart {
    pub component: FitComponent,
    pub label: &'static str,
    /// 0-100.
    pub score: f32,
    /// The share of the final score this part carried, after absent
    /// components were removed and the rest renormalized.
    pub weight: f32,
    pub detail: String,
}

/// How strongly a hero is being put forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationLevel {
    Recommended,
    Consider,
    AvoidForNow,
}

impl RecommendationLevel {
    pub fn label(self) -> &'static str {
        match self {
            RecommendationLevel::Recommended => "Recommended",
            RecommendationLevel::Consider => "Consider",
            RecommendationLevel::AvoidForNow => "Avoid for now",
        }
    }
}

/// A hero, scored for this player.
#[derive(Debug, Clone, Serialize)]
pub struct HeroFit {
    pub hero_id: i32,
    pub hero_name: String,
    /// 0-100.
    pub fit_score: f32,
    pub level: RecommendationLevel,
    pub level_label: &'static str,

    pub parts: Vec<FitPart>,
    /// Why this hero scores as it does. Deterministic sentences; the coaching
    /// layer may rewrite them, but it never computes them.
    pub reasons: Vec<String>,
    /// What the score cannot see, or is standing on thin evidence for.
    pub caveats: Vec<String>,

    pub matches: i64,
    pub tier: Option<HeroTier>,
    pub meta_strength: Option<f32>,
    /// Points added or removed for training-focus compatibility. A modifier
    /// rather than a weighted component, per `PRODUCT_SPEC.md` §19, and
    /// reported so the adjustment is never invisible.
    pub focus_adjustment: f32,
}

/// Relative weights of the fit components.
///
/// The initial values come from `PRODUCT_SPEC.md` §19. They are data, not
/// structure: nothing downstream assumes a particular weighting, and a
/// component whose input is missing is dropped and the rest renormalized.
#[derive(Debug, Clone, Copy)]
pub struct FitWeights {
    pub player_performance: f32,
    pub meta_strength: f32,
    pub experience: f32,
    pub benchmark: f32,
    pub recent_form: f32,
}

impl Default for FitWeights {
    fn default() -> Self {
        Self {
            player_performance: 0.30,
            meta_strength: 0.25,
            experience: 0.20,
            benchmark: 0.15,
            recent_form: 0.10,
        }
    }
}

impl FitWeights {
    pub fn for_component(&self, component: FitComponent) -> f32 {
        match component {
            FitComponent::PlayerPerformance => self.player_performance,
            FitComponent::MetaStrength => self.meta_strength,
            FitComponent::Experience => self.experience,
            FitComponent::Benchmark => self.benchmark,
            FitComponent::RecentForm => self.recent_form,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rank_tier_maps_to_its_medal_not_its_stars() {
        // Legend 5.
        assert_eq!(RankBracket::from_rank_tier(55), Some(RankBracket::Legend));
        // Legend 1 is the same bracket.
        assert_eq!(RankBracket::from_rank_tier(51), Some(RankBracket::Legend));
        assert_eq!(RankBracket::from_rank_tier(80), Some(RankBracket::Immortal));
    }

    #[test]
    fn an_unranked_tier_has_no_bracket_rather_than_the_lowest_one() {
        assert_eq!(RankBracket::from_rank_tier(0), None);
        assert_eq!(RankBracket::from_rank_tier(9), None);
        assert_eq!(RankBracket::from_index(99), None);
    }

    #[test]
    fn bracket_indexes_round_trip() {
        for bracket in RankBracket::ALL {
            assert_eq!(RankBracket::from_index(bracket.index()), Some(bracket));
        }
    }

    #[test]
    fn the_default_weights_are_the_spec_weights_and_sum_to_one() {
        let w = FitWeights::default();
        let total =
            w.player_performance + w.meta_strength + w.experience + w.benchmark + w.recent_form;
        assert!((total - 1.0).abs() < f32::EPSILON * 4.0);
        assert_eq!(w.player_performance, 0.30);
    }
}
