//! Which of the player's matches a page reads, from its `?role=` parameter.
//!
//! Benchmark, Rank and Heroes all answer this the same way, so it lives here
//! once. Two copies drifting apart is how one page ends up quoting a role
//! percentile and another an all-roles one for the "same" filter.
//!
//! The rule: an explicit role slug wins, `all` reads every role, and no value
//! means the role the player is being coached on (every role if none is set).
//! Every answer is competitive-only and uses the same match window.

use crate::domain::player::DotaPlayer;
use crate::domain::role::CoachableRole;
use crate::domain::scope::MatchScope;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::state::AppState;

/// The resolved population, plus what a page needs to describe it.
#[derive(Debug, Clone)]
pub(crate) struct RoleScope {
    pub matches: MatchScope,
    /// The role being read, `None` for every role.
    pub role: Option<CoachableRole>,
    /// The role being coached, offered by the UI as the default filter.
    pub coaching_role: Option<CoachableRole>,
    /// The newest this-many eligible matches.
    pub window: i64,
}

pub(crate) async fn resolve(
    state: &AppState,
    player: &DotaPlayer,
    requested: Option<&str>,
) -> AppResult<RoleScope> {
    let window = state.config.roles.analysis_match_limit;
    let coaching_role = repositories::coaching_profile::find(&state.db, player.id)
        .await?
        .map(|profile| profile.selected_role);
    let role = pick_role(requested, coaching_role)?;
    let matches = match role {
        Some(role) => MatchScope::for_role(role, window),
        None => MatchScope::competitive(window),
    };

    Ok(RoleScope {
        matches,
        role,
        coaching_role,
        window,
    })
}

/// The role a request asks for. Pure, so the rule is testable without a
/// database.
fn pick_role(
    requested: Option<&str>,
    coaching: Option<CoachableRole>,
) -> AppResult<Option<CoachableRole>> {
    match requested.map(str::trim) {
        Some("all") => Ok(None),
        Some(slug) if !slug.is_empty() => CoachableRole::parse(slug).map(Some).ok_or_else(|| {
            AppError::BadRequest(format!(
                "'{slug}' is not a role. Use one of: {}, or 'all'.",
                CoachableRole::ALL
                    .iter()
                    .map(|r| r.slug())
                    .collect::<Vec<_>>()
                    .join(", "),
            ))
        }),
        _ => Ok(coaching),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COACHED: Option<CoachableRole> = Some(CoachableRole::Mid);

    #[test]
    fn no_role_follows_the_coaching_role() {
        assert_eq!(pick_role(None, COACHED).unwrap(), COACHED);
        assert_eq!(pick_role(Some("  "), COACHED).unwrap(), COACHED);
        assert_eq!(pick_role(None, None).unwrap(), None);
    }

    #[test]
    fn all_reads_every_role_even_when_coaching_one() {
        assert_eq!(pick_role(Some("all"), COACHED).unwrap(), None);
    }

    #[test]
    fn an_explicit_role_overrides_the_coaching_role() {
        assert_eq!(
            pick_role(Some("hard_support"), COACHED).unwrap(),
            Some(CoachableRole::HardSupport)
        );
        assert_eq!(
            pick_role(Some("Soft Support"), COACHED).unwrap(),
            Some(CoachableRole::SoftSupport)
        );
    }

    #[test]
    fn an_unknown_role_is_rejected_not_guessed() {
        assert!(matches!(
            pick_role(Some("core"), COACHED),
            Err(AppError::BadRequest(_))
        ));
    }
}
