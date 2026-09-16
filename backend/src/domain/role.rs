//! The five roles a player can ask to be coached for.
//!
//! [`Role`](crate::domain::r#match::Role) is what the *estimator* can say about
//! a match: nine labels, several of which exist precisely because the data was
//! not good enough to be more specific. `CoachableRole` is what the *product*
//! offers: Carry, Mid, Offlane, Soft Support, Hard Support, and nothing else.
//!
//! The mapping between them is deliberately lossy in one direction, and the
//! loss is the point:
//!
//! ```text
//! Carry         → Carry
//! Mid           → Mid
//! Offlane       → Offlane
//! Support       → Soft Support
//! Hard Support  → Hard Support
//! Core          → (unclassified)
//! Roamer        → (unclassified)
//! Jungle        → (unclassified)
//! Unknown       → (unclassified)
//! ```
//!
//! `Core` is the interesting one. It comes from an unparsed replay, where farm
//! priority says "this player was one of the three cores" and nothing says
//! which. Folding it into Carry would put Mid and Offlane games into a Carry
//! player's coaching dataset — exactly the mixing the role scope exists to
//! prevent — so it stays unclassified and is counted and reported instead.
//!
//! That has a cost, and it should be stated plainly rather than discovered: a
//! player whose replays are mostly unparsed will see a large unclassified
//! count and small per-role samples. The alternative is coaching someone's
//! Carry on their Offlane games, which is worse.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::r#match::Role;
use crate::domain::scope::SampleConfidence;

/// A role the player can select for coaching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoachableRole {
    Carry,
    Mid,
    Offlane,
    SoftSupport,
    HardSupport,
}

impl CoachableRole {
    /// Position order, safe lane to hard support. Every list the product shows
    /// is in this order, because it is the order players think in.
    pub const ALL: [CoachableRole; 5] = [
        CoachableRole::Carry,
        CoachableRole::Mid,
        CoachableRole::Offlane,
        CoachableRole::SoftSupport,
        CoachableRole::HardSupport,
    ];

    /// URL- and storage-safe identifier. Persisted in the coaching profile, so
    /// it must not change casually.
    pub fn slug(self) -> &'static str {
        match self {
            CoachableRole::Carry => "carry",
            CoachableRole::Mid => "mid",
            CoachableRole::Offlane => "offlane",
            CoachableRole::SoftSupport => "soft_support",
            CoachableRole::HardSupport => "hard_support",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CoachableRole::Carry => "Carry",
            CoachableRole::Mid => "Mid",
            CoachableRole::Offlane => "Offlane",
            CoachableRole::SoftSupport => "Soft Support",
            CoachableRole::HardSupport => "Hard Support",
        }
    }

    /// The conventional position number. Display only — nothing keys off it.
    pub fn position(self) -> u8 {
        match self {
            CoachableRole::Carry => 1,
            CoachableRole::Mid => 2,
            CoachableRole::Offlane => 3,
            CoachableRole::SoftSupport => 4,
            CoachableRole::HardSupport => 5,
        }
    }

    /// Parsed forgivingly, because this arrives from a URL and from a client.
    /// A label ("Soft Support"), a slug, and the obvious spellings in between
    /// all resolve; anything else is rejected rather than guessed at.
    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().to_lowercase().replace([' ', '-'], "_");
        Self::ALL
            .into_iter()
            .find(|role| role.slug() == normalized)
            // "support" alone is ambiguous in Dota conversation but unambiguous
            // here: the estimator's `Support` label is position four.
            .or_else(|| (normalized == "support").then_some(CoachableRole::SoftSupport))
    }

    /// Which of the estimator's labels belong to this role.
    ///
    /// A slice rather than a single string so the coupling to
    /// [`Role::as_str`] is declared in one place; the SQL scope builder reads
    /// it to filter `matches.role`, which stores exactly those strings.
    pub fn stored_labels(self) -> &'static [&'static str] {
        match self {
            CoachableRole::Carry => &["Carry"],
            CoachableRole::Mid => &["Mid"],
            CoachableRole::Offlane => &["Offlane"],
            CoachableRole::SoftSupport => &["Support"],
            CoachableRole::HardSupport => &["Hard Support"],
        }
    }

    /// The coachable role an estimated one belongs to, if any.
    pub fn from_role(role: Role) -> Option<Self> {
        match role {
            Role::Carry => Some(CoachableRole::Carry),
            Role::Mid => Some(CoachableRole::Mid),
            Role::Offlane => Some(CoachableRole::Offlane),
            Role::Support => Some(CoachableRole::SoftSupport),
            Role::HardSupport => Some(CoachableRole::HardSupport),
            // Deliberately unclassified: see the module comment.
            Role::Core | Role::Roamer | Role::Jungle | Role::Unknown => None,
        }
    }

    /// The same mapping from the stored `matches.role` text.
    pub fn from_stored(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|role| role.stored_labels().contains(&label))
    }
}

/// Per-role sums, straight from SQL.
///
/// Sums rather than averages because the stored role labels are finer-grained
/// than the coachable ones — merging two groups into one average needs their
/// totals, and re-deriving them from averages and counts loses precision for
/// no reason. `None` means the column had no value to sum anywhere in the
/// group, never that the value was zero.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RoleTotals {
    /// The estimator's label, as stored on the match row.
    pub role: String,
    pub matches: i64,
    pub wins: i64,
    pub kda_sum: Option<f64>,
    pub gpm_sum: Option<f64>,
    pub xpm_sum: Option<f64>,
    pub last_hits_per_min_sum: Option<f64>,
    pub deaths_per_10_sum: Option<f64>,
    /// Summed only over matches that carry team totals.
    pub kill_participation_sum: Option<f64>,
    pub kill_participation_matches: i64,
}

/// How much each measure counts toward a role's performance score.
///
/// Configuration rather than code, on the same reasoning as
/// [`FitWeights`](crate::domain::hero::FitWeights): these are a starting point
/// to be revised against real usage, and revising them must not mean editing
/// the scoring engine.
#[derive(Debug, Clone, Copy)]
pub struct RoleScoreWeights {
    pub win_rate: f32,
    pub kill_participation: f32,
    pub kda: f32,
    pub deaths: f32,
}

impl Default for RoleScoreWeights {
    fn default() -> Self {
        Self {
            // Winning is the outcome the other three only approximate, so it
            // carries the most weight — but not all of it, because win rate
            // alone cannot tell a player carrying their games from one being
            // carried.
            win_rate: 0.45,
            kill_participation: 0.20,
            kda: 0.20,
            deaths: 0.15,
        }
    }
}

impl RoleScoreWeights {
    pub fn for_component(&self, component: ScoreComponentKey) -> f32 {
        match component {
            ScoreComponentKey::WinRate => self.win_rate,
            ScoreComponentKey::KillParticipation => self.kill_participation,
            ScoreComponentKey::Kda => self.kda,
            ScoreComponentKey::Deaths => self.deaths,
        }
    }
}

/// The measures a role score is built from.
///
/// Deliberately all role-neutral. Gold per minute is the obvious thing to
/// reach for and the wrong one: a hard support out-farming their own average
/// is not thereby a better support than a carry is a carry, and a score that
/// compared them on economy would recommend the safe lane to everybody.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScoreComponentKey {
    WinRate,
    KillParticipation,
    Kda,
    Deaths,
}

impl ScoreComponentKey {
    pub const ALL: [ScoreComponentKey; 4] = [
        ScoreComponentKey::WinRate,
        ScoreComponentKey::KillParticipation,
        ScoreComponentKey::Kda,
        ScoreComponentKey::Deaths,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            ScoreComponentKey::WinRate => "win_rate",
            ScoreComponentKey::KillParticipation => "kill_participation",
            ScoreComponentKey::Kda => "kda",
            ScoreComponentKey::Deaths => "deaths_per_10",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ScoreComponentKey::WinRate => "Win rate",
            ScoreComponentKey::KillParticipation => "Kill participation",
            ScoreComponentKey::Kda => "KDA",
            ScoreComponentKey::Deaths => "Deaths per 10 minutes",
        }
    }
}

/// One measure's contribution to a role score, kept so the number can be
/// explained rather than merely displayed.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ScoreComponent {
    pub key: ScoreComponentKey,
    pub label: &'static str,
    /// The measured figure, in its own units.
    pub value: f32,
    /// That figure mapped onto 0-100, direction corrected.
    pub normalized: f32,
    /// The weight actually applied, after renormalizing around any measure
    /// this role had no data for.
    pub weight: f32,
    /// Matches the figure was averaged over.
    pub sample: i64,
}

/// One role, as measured across the eligible window.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RolePerformance {
    pub role: CoachableRole,
    pub role_label: &'static str,
    pub position: u8,

    pub matches: i64,
    pub wins: i64,
    pub losses: i64,
    pub win_rate: f32,

    pub avg_kda: Option<f32>,
    pub avg_gpm: Option<f32>,
    pub avg_xpm: Option<f32>,
    pub avg_last_hits_per_min: Option<f32>,
    pub avg_deaths_per_10: Option<f32>,
    pub avg_kill_participation: Option<f32>,
    /// Matches behind `avg_kill_participation`, which needs team totals.
    pub kill_participation_sample: i64,

    /// 0-100, after the sample-size adjustment. This is the number to compare
    /// roles on.
    pub performance: f32,
    /// The same score before that adjustment, so the effect of a thin sample
    /// is visible rather than baked in silently.
    pub raw_performance: f32,
    pub confidence: SampleConfidence,
    pub components: Vec<ScoreComponent>,
}

/// The role the system would pick, and why.
///
/// Advisory in the strictest sense: nothing downstream reads it to decide what
/// to coach. The player's selection does that.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RoleRecommendation {
    pub role: CoachableRole,
    pub role_label: &'static str,
    /// Composed from the measured figures above — no model wrote this.
    pub why: String,
    pub runner_up: Option<CoachableRole>,
    pub confidence: SampleConfidence,
}

/// Role performance across the eligible window, with the advisory pick.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RoleAnalysis {
    /// Eligible matches read, after filtering and after the window limit.
    pub analyzed_matches: i64,
    pub confidence: SampleConfidence,
    pub confidence_label: &'static str,
    pub confidence_caveat: &'static str,
    /// Every role with at least one eligible match, strongest first.
    pub roles: Vec<RolePerformance>,
    /// Eligible matches whose role could not be attributed to one of the five
    /// — almost always an unparsed replay that could only be read as `Core`.
    pub unclassified_matches: i64,
    /// Eligible matches a role needs before it can be recommended. Sent so a
    /// client can say "three more games" instead of leaving a role silently
    /// absent from the advice.
    pub min_recommendable_matches: i64,
    /// `None` when no role has enough eligible matches to justify a pick.
    pub recommendation: Option<RoleRecommendation>,
    /// Set when there is nothing to recommend, saying which of the two reasons
    /// it is: no eligible matches at all, or none with enough of a sample.
    pub note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_estimated_role_maps_or_is_explicitly_unclassified() {
        assert_eq!(
            CoachableRole::from_role(Role::Carry),
            Some(CoachableRole::Carry)
        );
        assert_eq!(
            CoachableRole::from_role(Role::Mid),
            Some(CoachableRole::Mid)
        );
        assert_eq!(
            CoachableRole::from_role(Role::Offlane),
            Some(CoachableRole::Offlane)
        );
        assert_eq!(
            CoachableRole::from_role(Role::Support),
            Some(CoachableRole::SoftSupport)
        );
        assert_eq!(
            CoachableRole::from_role(Role::HardSupport),
            Some(CoachableRole::HardSupport)
        );

        for role in [Role::Core, Role::Roamer, Role::Jungle, Role::Unknown] {
            assert_eq!(
                CoachableRole::from_role(role),
                None,
                "{} must not be attributed to a lane it may not have played",
                role.as_str(),
            );
        }
    }

    /// The mapping is only correct if `stored_labels` really are the strings
    /// the estimator writes into the column.
    #[test]
    fn stored_labels_match_what_the_estimator_writes() {
        for role in [
            Role::Carry,
            Role::Mid,
            Role::Offlane,
            Role::Support,
            Role::HardSupport,
        ] {
            let coachable = CoachableRole::from_role(role).unwrap();
            assert!(
                coachable.stored_labels().contains(&role.as_str()),
                "{} is stored as {:?} but mapped from {:?}",
                role.as_str(),
                role.as_str(),
                coachable.stored_labels(),
            );
            assert_eq!(CoachableRole::from_stored(role.as_str()), Some(coachable));
        }
    }

    #[test]
    fn unclassified_labels_have_no_stored_mapping_either() {
        for role in [Role::Core, Role::Roamer, Role::Jungle, Role::Unknown] {
            assert_eq!(CoachableRole::from_stored(role.as_str()), None);
        }
    }

    #[test]
    fn roles_round_trip_through_their_slug() {
        for role in CoachableRole::ALL {
            assert_eq!(CoachableRole::parse(role.slug()), Some(role));
            assert_eq!(CoachableRole::parse(role.label()), Some(role));
        }
    }

    #[test]
    fn parsing_is_forgiving_about_shape_but_not_about_meaning() {
        assert_eq!(
            CoachableRole::parse(" Soft-Support "),
            Some(CoachableRole::SoftSupport)
        );
        // The estimator's own word for position four.
        assert_eq!(
            CoachableRole::parse("support"),
            Some(CoachableRole::SoftSupport)
        );
        // Not a coachable role, and not coerced into the nearest one.
        assert_eq!(CoachableRole::parse("core"), None);
        assert_eq!(CoachableRole::parse("jungle"), None);
        assert_eq!(CoachableRole::parse(""), None);
    }
}
