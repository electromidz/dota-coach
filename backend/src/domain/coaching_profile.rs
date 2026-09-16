//! What the player chose to work on.
//!
//! The profile is the boundary between two things the product must never
//! confuse:
//!
//! ```text
//! Overall analysis   — every eligible match, every role
//! Role coaching      — every eligible match in ONE role, the chosen one
//! ```
//!
//! It holds the choice and the circumstances it was made in, and nothing that
//! can be recomputed. In particular the coaching dataset — which matches the
//! advice is actually about — is resolved from the role on every read, because
//! it changes every time the player syncs.
//!
//! The recommendation is recorded beside the selection precisely so the two can
//! disagree. A player who is advised to work on Support and chooses Carry is
//! coached on Carry, and the profile still remembers what was suggested.

use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::role::CoachableRole;

/// A stored coaching profile.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CoachingProfile {
    pub id: Uuid,
    /// The role every piece of coaching is scoped to. The player's choice,
    /// never the system's.
    pub selected_role: CoachableRole,
    pub selected_role_label: &'static str,
    /// What was recommended when the choice was made. `None` when nothing
    /// cleared the evidence bar — a real state, not a missing value.
    pub recommended_role: Option<CoachableRole>,
    pub recommended_role_label: Option<&'static str>,
    /// True when the player was advised one role and picked another. Recorded
    /// because it is a fact about the choice, and because a coach that
    /// second-guesses it would be the wrong product.
    pub overrode_recommendation: bool,
    /// Eligible matches the recommendation rested on at selection time.
    pub analyzed_matches: i64,
    pub selected_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// The row as stored, before the labels are resolved.
///
/// Roles are persisted as slugs, so a row written by an older build with a role
/// this one no longer knows reads back as `None` rather than panicking.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredProfile {
    pub id: Uuid,
    pub selected_role: String,
    pub recommended_role: Option<String>,
    pub analyzed_matches: i32,
    pub selected_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl StoredProfile {
    /// Resolve a stored row into the domain type.
    ///
    /// `None` when the stored role is not one this build recognises. Dropping
    /// the profile is the right failure: it makes the player choose again,
    /// which is a small annoyance, where guessing a role for them would scope
    /// their coaching to matches they never asked about.
    pub fn hydrate(self) -> Option<CoachingProfile> {
        let selected_role = CoachableRole::parse(&self.selected_role)?;
        let recommended_role = self
            .recommended_role
            .as_deref()
            .and_then(CoachableRole::parse);

        Some(CoachingProfile {
            id: self.id,
            selected_role,
            selected_role_label: selected_role.label(),
            recommended_role,
            recommended_role_label: recommended_role.map(CoachableRole::label),
            overrode_recommendation: recommended_role
                .is_some_and(|recommended| recommended != selected_role),
            analyzed_matches: self.analyzed_matches as i64,
            selected_at: self.selected_at,
            updated_at: self.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(selected: &str, recommended: Option<&str>) -> StoredProfile {
        StoredProfile {
            id: Uuid::nil(),
            selected_role: selected.into(),
            recommended_role: recommended.map(str::to_string),
            analyzed_matches: 40,
            selected_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn a_choice_that_matches_the_advice_is_not_an_override() {
        let profile = stored("carry", Some("carry")).hydrate().unwrap();

        assert_eq!(profile.selected_role, CoachableRole::Carry);
        assert!(!profile.overrode_recommendation);
    }

    #[test]
    fn choosing_against_the_advice_is_recorded_as_such_and_still_wins() {
        let profile = stored("carry", Some("soft_support")).hydrate().unwrap();

        assert_eq!(
            profile.selected_role,
            CoachableRole::Carry,
            "the player's choice is the coaching role, always",
        );
        assert_eq!(profile.recommended_role, Some(CoachableRole::SoftSupport));
        assert!(profile.overrode_recommendation);
    }

    #[test]
    fn a_choice_made_with_no_advice_on_offer_is_not_an_override() {
        let profile = stored("mid", None).hydrate().unwrap();

        assert_eq!(profile.recommended_role, None);
        assert_eq!(profile.recommended_role_label, None);
        assert!(!profile.overrode_recommendation);
    }

    #[test]
    fn an_unrecognised_stored_role_drops_the_profile_rather_than_guessing() {
        assert!(stored("jungle", None).hydrate().is_none());
        assert!(stored("", None).hydrate().is_none());
    }

    #[test]
    fn an_unrecognised_recommendation_does_not_drop_a_valid_selection() {
        // The selection is the part that scopes coaching; a stale advisory
        // value is not worth discarding it over.
        let profile = stored("carry", Some("roamer")).hydrate().unwrap();

        assert_eq!(profile.selected_role, CoachableRole::Carry);
        assert_eq!(profile.recommended_role, None);
        assert!(!profile.overrode_recommendation);
    }
}
