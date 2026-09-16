//! The population a number was computed over.
//!
//! Three questions decide which matches an aggregate may read, and the product
//! needs different answers to them in different places:
//!
//!   * are ineligible modes excluded?  (always, for anything competitive)
//!   * is it narrowed to one role?     (only once the player has chosen one)
//!   * how far back does it go?        (the latest hundred, normally)
//!
//! [`MatchScope`] is those three answers travelling together, and it renders
//! itself into the one SQL window every scoped aggregate joins against. That is
//! what makes the ordering rule structural rather than a convention somebody
//! has to remember:
//!
//! ```text
//! filter (eligibility, role)  →  order by recency  →  take N
//! ```
//!
//! Never the other way round. Taking the latest hundred matches and *then*
//! removing Turbo gives a player with fifty Turbo games a fifty-game analysis
//! that claims to be a hundred-game one.

use crate::domain::eligibility;
use crate::domain::role::CoachableRole;
use serde::Serialize;
use utoipa::ToSchema;

/// The name the window carries in a query. Aggregates join `scoped`.
pub const SCOPE_CTE: &str = "scoped";

/// How much weight an analysis built on this many matches can bear.
///
/// Distinct from [`Confidence`](crate::domain::benchmark::Confidence), which
/// answers a narrower question — whether a *percentile* may be claimed against
/// a peer distribution. This one is about the size of the analysis window
/// itself, and it is what the dashboard shows beside "37 games analysed".
///
/// The thresholds: below twenty matches a role reading is one bad week away
/// from reversing, so it is `Limited`. Sixty is where the per-role samples
/// behind a hundred-game window stop being small in their own right, so that is
/// `Strong`. Everything between is `Moderate`. They are round numbers chosen to
/// be explainable rather than derived from a power calculation the data does
/// not support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SampleConfidence {
    Limited,
    Moderate,
    Strong,
}

/// Below this, `Limited`.
pub const MODERATE_SAMPLE: i64 = 20;
/// At or above this, `Strong`.
pub const STRONG_SAMPLE: i64 = 60;

impl SampleConfidence {
    pub fn for_matches(matches: i64) -> Self {
        if matches < MODERATE_SAMPLE {
            SampleConfidence::Limited
        } else if matches < STRONG_SAMPLE {
            SampleConfidence::Moderate
        } else {
            SampleConfidence::Strong
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            SampleConfidence::Limited => "limited",
            SampleConfidence::Moderate => "moderate",
            SampleConfidence::Strong => "strong",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SampleConfidence::Limited => "Limited",
            SampleConfidence::Moderate => "Moderate",
            SampleConfidence::Strong => "Strong",
        }
    }

    /// What the number beside it is worth, in a sentence.
    pub fn caveat(self) -> &'static str {
        match self {
            SampleConfidence::Limited => {
                "Too few eligible matches for a firm reading — treat this as a first impression."
            }
            SampleConfidence::Moderate => {
                "Enough eligible matches to see a direction, not enough to be sure of a small gap."
            }
            SampleConfidence::Strong => "Enough eligible matches for a dependable reading.",
        }
    }
}

/// Which matches an aggregate may read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchScope {
    /// Restrict to the competitive population — see [`eligibility`].
    pub eligible_only: bool,
    /// Restrict to one coachable role. `None` reads every role, including the
    /// matches no role could be attributed to.
    pub role: Option<CoachableRole>,
    /// Keep only the newest `n` matches that survive the filters above.
    pub limit: Option<i64>,
}

impl MatchScope {
    /// Everything the player has ever played, unfiltered.
    ///
    /// What the match list and the existing career statistics use. Kept so the
    /// pages that legitimately show a player their whole history — including
    /// the Turbo games they actually played — keep working unchanged.
    pub fn career() -> Self {
        Self {
            eligible_only: false,
            role: None,
            limit: None,
        }
    }

    /// The latest `limit` eligible matches, every role.
    pub fn competitive(limit: i64) -> Self {
        Self {
            eligible_only: true,
            role: None,
            limit: Some(limit),
        }
    }

    /// The latest `limit` eligible matches in one role: the coaching dataset.
    pub fn for_role(role: CoachableRole, limit: i64) -> Self {
        Self {
            eligible_only: true,
            role: Some(role),
            limit: Some(limit),
        }
    }

    /// The window as a common table expression.
    ///
    /// `$1` is the player id, and stays the only bound parameter: everything
    /// else interpolated here is a compile-time constant — mode ids from
    /// [`eligibility`], role labels from [`CoachableRole::stored_labels`], and
    /// an `i64` limit from configuration. None of it is user input, and none of
    /// it can be.
    pub fn cte(&self) -> String {
        let mut sql = format!(
            "WITH {SCOPE_CTE} AS (
                 SELECT m.id
                   FROM matches m
                   -- Inner join: a match whose metrics have not been computed
                   -- cannot contribute to any aggregate, so counting it in the
                   -- window would report a sample the numbers do not have.
                   JOIN match_metrics mm ON mm.match_id = m.id
                  WHERE m.dota_player_id = $1"
        );

        if self.eligible_only {
            sql.push_str(&format!(
                "\n                    AND {}",
                eligibility::sql_predicate("m")
            ));
        }

        if let Some(role) = self.role {
            sql.push_str(&format!(
                "\n                    AND m.role = ANY(ARRAY[{}])",
                role.stored_labels()
                    .iter()
                    .map(|label| format!("'{label}'"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }

        // Recency last, so the limit applies to what survived the filters.
        sql.push_str("\n                  ORDER BY m.started_at DESC");

        if let Some(limit) = self.limit {
            sql.push_str(&format!("\n                  LIMIT {limit}"));
        }

        sql.push_str("\n             )");
        sql
    }

    /// The join that narrows a query to this window.
    pub fn join(&self) -> &'static str {
        "JOIN scoped s ON s.id = m.id"
    }

    /// True when this scope reads everything, which is the one case where the
    /// window is pure overhead.
    pub fn is_career(&self) -> bool {
        *self == Self::career()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_competitive_scope_filters_before_it_limits() {
        let sql = MatchScope::competitive(100).cte();

        let where_at = sql.find("WHERE").expect("a where clause");
        let eligible_at = sql.find("game_mode").expect("the eligibility predicate");
        let order_at = sql.find("ORDER BY").expect("an ordering");
        let limit_at = sql.find("LIMIT").expect("a limit");

        assert!(
            where_at < eligible_at && eligible_at < order_at && order_at < limit_at,
            "the limit must apply to the filtered set, not the raw history:\n{sql}",
        );
        assert!(sql.contains("LIMIT 100"));
    }

    #[test]
    fn a_role_scope_filters_on_the_labels_the_estimator_writes() {
        let sql = MatchScope::for_role(CoachableRole::SoftSupport, 100).cte();

        // Position four is stored as "Support", not "Soft Support".
        assert!(sql.contains("m.role = ANY(ARRAY['Support'])"), "{sql}");
        // And it is still competitive-only.
        assert!(sql.contains("game_mode"));

        let role_at = sql.find("m.role").unwrap();
        let limit_at = sql.find("LIMIT").unwrap();
        assert!(role_at < limit_at, "role filter must precede the limit");
    }

    #[test]
    fn every_role_renders_a_label_that_needs_no_escaping() {
        for role in CoachableRole::ALL {
            let sql = MatchScope::for_role(role, 10).cte();
            for label in role.stored_labels() {
                assert!(sql.contains(&format!("'{label}'")));
                assert!(
                    !label.contains('\'') && !label.contains('\\'),
                    "stored labels are interpolated, so they must stay plain",
                );
            }
        }
    }

    #[test]
    fn a_career_scope_filters_nothing() {
        let sql = MatchScope::career().cte();

        assert!(
            !sql.contains("game_mode"),
            "career reads every mode:\n{sql}"
        );
        assert!(!sql.contains("m.role"));
        assert!(!sql.contains("LIMIT"));
        assert!(MatchScope::career().is_career());
        assert!(!MatchScope::competitive(100).is_career());
    }

    #[test]
    fn confidence_follows_the_documented_thresholds() {
        assert_eq!(SampleConfidence::for_matches(0), SampleConfidence::Limited);
        assert_eq!(SampleConfidence::for_matches(19), SampleConfidence::Limited);
        assert_eq!(
            SampleConfidence::for_matches(20),
            SampleConfidence::Moderate
        );
        assert_eq!(
            SampleConfidence::for_matches(59),
            SampleConfidence::Moderate
        );
        assert_eq!(SampleConfidence::for_matches(60), SampleConfidence::Strong);
        assert_eq!(SampleConfidence::for_matches(100), SampleConfidence::Strong);
    }
}
