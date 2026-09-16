//! Which matches may be used for competitive analysis.
//!
//! Everything the coach says about a player — overall statistics, role
//! performance, the role recommendation, the coaching dataset, the evidence
//! the model is shown — is a statement about *standard All Pick matchmaking*.
//! Turbo is a different game with different economy curves, and mixing it into
//! those numbers does not make the sample bigger, it makes it wrong.
//!
//! So eligibility is defined exactly once, here, and every caller reads it from
//! this module. Two renderings exist — [`classify`] for Rust and
//! [`sql_predicate`] for a query — and both are generated from the same
//! constant lists, with a test asserting they cannot drift apart.
//!
//! The rule is a *read* rule, not a sync rule. Synchronization deliberately
//! stores every mode the player actually played (see `DotaConfig::
//! significant_only`), because a match history that silently omits most of a
//! player's games is worse than one that needs a caveat. Filtering belongs at
//! the point where a number is computed, not at the point where a fact is
//! recorded.

use serde::Serialize;
use utoipa::ToSchema;

/// OpenDota `game_mode` ids, verified against `/api/constants/game_mode`.
///
/// Only the ones this module names are listed. Everything else — Captains Mode
/// (2), Random Draft (3), Ability Draft (18), Custom (15), Event (19) and the
/// rest — is excluded by not appearing in [`ELIGIBLE_GAME_MODES`], which is
/// deliberate: a new mode Valve ships is ineligible until somebody decides
/// otherwise, rather than silently joining the coaching dataset.
pub mod game_mode {
    /// `game_mode_all_pick` — the classic All Pick draft.
    pub const ALL_PICK: i32 = 1;
    /// `game_mode_all_draft` — what ranked and modern public All Pick report.
    pub const ALL_DRAFT: i32 = 22;
    /// `game_mode_turbo`. Named because it is the one exclusion the product
    /// cares about by name.
    pub const TURBO: i32 = 23;
}

/// OpenDota `lobby_type` ids, verified against `/api/constants/lobby_type`.
pub mod lobby_type {
    /// `lobby_type_normal` — unranked public matchmaking.
    pub const NORMAL: i32 = 0;
    /// `lobby_type_ranked`.
    pub const RANKED: i32 = 7;
}

/// The draft formats that count as All Pick.
pub const ELIGIBLE_GAME_MODES: [i32; 2] = [game_mode::ALL_DRAFT, game_mode::ALL_PICK];

/// The lobbies that count as public matchmaking: ranked, and unranked public.
///
/// Practice lobbies, tournaments, Battle Cup and bot games are excluded — they
/// are All Pick often enough, but they are not the ladder the player is asking
/// to be coached for.
pub const ELIGIBLE_LOBBY_TYPES: [i32; 2] = [lobby_type::RANKED, lobby_type::NORMAL];

/// Why a match is not part of the competitive population.
///
/// Reported rather than swallowed. A player whose history is two thirds Turbo
/// should be told that in those words, not left to wonder why the coach can
/// only see thirty of their ninety games.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionReason {
    /// Turbo. Its own reason because it is the one players ask about.
    Turbo,
    /// A draft format that is not All Pick — Captains Mode, Random Draft,
    /// Ability Draft, Custom, an event mode.
    OtherGameMode,
    /// All Pick, but not public matchmaking: a practice lobby, a tournament,
    /// Battle Cup, bots.
    NonPublicLobby,
    /// The provider never reported the mode for this match, so it cannot be
    /// placed either way. Excluded rather than assumed — a guess here would
    /// quietly put Turbo back into the numbers.
    ModeUnknown,
}

impl ExclusionReason {
    pub const ALL: [ExclusionReason; 4] = [
        ExclusionReason::Turbo,
        ExclusionReason::OtherGameMode,
        ExclusionReason::NonPublicLobby,
        ExclusionReason::ModeUnknown,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            ExclusionReason::Turbo => "turbo",
            ExclusionReason::OtherGameMode => "other_game_mode",
            ExclusionReason::NonPublicLobby => "non_public_lobby",
            ExclusionReason::ModeUnknown => "mode_unknown",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExclusionReason::Turbo => "Turbo",
            ExclusionReason::OtherGameMode => "Other game mode",
            ExclusionReason::NonPublicLobby => "Not public matchmaking",
            ExclusionReason::ModeUnknown => "Game mode unknown",
        }
    }

    /// One sentence a UI can render beside the count.
    pub fn description(self) -> &'static str {
        match self {
            ExclusionReason::Turbo => {
                "Turbo games are excluded: the economy is different, so they cannot be read against All Pick benchmarks."
            }
            ExclusionReason::OtherGameMode => {
                "Only All Pick drafts are analysed — Captains Mode, Random Draft, Ability Draft, custom and event games are excluded."
            }
            ExclusionReason::NonPublicLobby => {
                "Only public matchmaking counts. Practice lobbies, tournaments, Battle Cup and bot games are excluded."
            }
            ExclusionReason::ModeUnknown => {
                "The provider never reported a game mode for these matches, so they cannot be placed either way."
            }
        }
    }
}

/// The single authoritative eligibility rule.
///
/// `Ok(())` means the match belongs to the competitive population. The order of
/// the checks is what makes the reason useful rather than merely true: an
/// unreported mode is unknown before it is anything else, and a Turbo game is
/// Turbo before it is "some other mode".
pub fn classify(game_mode: Option<i32>, lobby_type: Option<i32>) -> Result<(), ExclusionReason> {
    let (Some(mode), Some(lobby)) = (game_mode, lobby_type) else {
        return Err(ExclusionReason::ModeUnknown);
    };

    if mode == game_mode::TURBO {
        return Err(ExclusionReason::Turbo);
    }
    if !ELIGIBLE_GAME_MODES.contains(&mode) {
        return Err(ExclusionReason::OtherGameMode);
    }
    // Checked after the mode, so an All Pick game in a tournament lobby reports
    // the lobby as the reason rather than being lumped in with Ability Draft.
    if !ELIGIBLE_LOBBY_TYPES.contains(&lobby) {
        return Err(ExclusionReason::NonPublicLobby);
    }

    Ok(())
}

/// A short name for what this match was, for a UI that lists every mode.
///
/// Only the modes this module names get a name of their own; everything else
/// is described by the reason it was excluded. That is deliberate — inventing
/// a label for an id we do not track would be the same mistake as inventing a
/// number, and "Other mode" is both true and enough for a list row.
pub fn mode_label(game_mode: Option<i32>, lobby_type: Option<i32>) -> &'static str {
    match classify(game_mode, lobby_type) {
        Ok(()) if lobby_type == Some(lobby_type::RANKED) => "Ranked All Pick",
        Ok(()) => "All Pick",
        Err(ExclusionReason::Turbo) => "Turbo",
        Err(ExclusionReason::OtherGameMode) => "Other mode",
        Err(ExclusionReason::NonPublicLobby) => "Private lobby",
        Err(ExclusionReason::ModeUnknown) => "Unknown mode",
    }
}

/// Convenience over [`classify`] for callers that only need the verdict.
pub fn is_eligible(game_mode: Option<i32>, lobby_type: Option<i32>) -> bool {
    classify(game_mode, lobby_type).is_ok()
}

/// The same rule as a SQL fragment, for the aggregate queries.
///
/// Generated from the constants above rather than written out, because two
/// hand-maintained copies of this rule is precisely the failure the module
/// exists to prevent: the day they disagree, one endpoint starts coaching on
/// Turbo and nothing announces it.
///
/// `NULL` needs no special case. `NULL = ANY(ARRAY[...])` is `NULL`, which is
/// not `TRUE`, so a match with no recorded mode fails the predicate — the same
/// answer [`classify`] gives it.
pub fn sql_predicate(alias: &str) -> String {
    format!(
        "{alias}.game_mode = ANY(ARRAY[{modes}]) AND {alias}.lobby_type = ANY(ARRAY[{lobbies}])",
        modes = list(&ELIGIBLE_GAME_MODES),
        lobbies = list(&ELIGIBLE_LOBBY_TYPES),
    )
}

/// Stored matches grouped by the two fields eligibility reads.
///
/// Deliberately not classified in SQL: the grouping is a cheap count, and the
/// verdict is then produced by [`classify`] — the same function every other
/// caller uses — rather than by a second rule expressed in a `CASE`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ModeCount {
    pub game_mode: Option<i32>,
    pub lobby_type: Option<i32>,
    pub matches: i64,
}

/// One reason, and how many matches it accounts for.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExcludedGroup {
    pub reason: ExclusionReason,
    pub label: &'static str,
    pub description: &'static str,
    pub matches: i64,
}

/// What the eligibility rule did to a player's stored history.
///
/// Shown to the user rather than kept internal: "we read 37 of your 112
/// matches" invites the obvious question, and this is the answer to it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EligibilitySummary {
    /// Every stored match, whatever mode.
    pub total_matches: i64,
    /// Matches in the competitive population, before any window limit.
    pub eligible_matches: i64,
    /// Largest group first, so the dominant reason leads.
    pub excluded: Vec<ExcludedGroup>,
}

/// Fold mode counts into the summary. Pure.
pub fn summarize(counts: &[ModeCount]) -> EligibilitySummary {
    let mut eligible = 0;
    let mut excluded: Vec<(ExclusionReason, i64)> = Vec::new();

    for row in counts {
        match classify(row.game_mode, row.lobby_type) {
            Ok(()) => eligible += row.matches,
            Err(reason) => match excluded.iter_mut().find(|(r, _)| *r == reason) {
                Some((_, matches)) => *matches += row.matches,
                None => excluded.push((reason, row.matches)),
            },
        }
    }

    // Largest first, then a stable tiebreak so the list does not reshuffle.
    excluded.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.slug().cmp(b.0.slug())));

    EligibilitySummary {
        total_matches: eligible + excluded.iter().map(|(_, n)| n).sum::<i64>(),
        eligible_matches: eligible,
        excluded: excluded
            .into_iter()
            .map(|(reason, matches)| ExcludedGroup {
                reason,
                label: reason.label(),
                description: reason.description(),
                matches,
            })
            .collect(),
    }
}

fn list(values: &[i32]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const RANKED: Option<i32> = Some(lobby_type::RANKED);
    const PUBLIC: Option<i32> = Some(lobby_type::NORMAL);

    #[test]
    fn ranked_and_public_all_pick_are_the_competitive_population() {
        // Ranked All Pick, as OpenDota reports it today.
        assert_eq!(classify(Some(game_mode::ALL_DRAFT), RANKED), Ok(()));
        // Unranked public All Pick.
        assert_eq!(classify(Some(game_mode::ALL_DRAFT), PUBLIC), Ok(()));
        // The older All Pick id, still present in historical matches.
        assert_eq!(classify(Some(game_mode::ALL_PICK), RANKED), Ok(()));
        assert_eq!(classify(Some(game_mode::ALL_PICK), PUBLIC), Ok(()));
    }

    #[test]
    fn turbo_is_excluded_and_says_so() {
        // In its usual public lobby...
        assert_eq!(
            classify(Some(game_mode::TURBO), PUBLIC),
            Err(ExclusionReason::Turbo)
        );
        // ...and the lobby never rescues it.
        assert_eq!(
            classify(Some(game_mode::TURBO), RANKED),
            Err(ExclusionReason::Turbo)
        );
    }

    #[test]
    fn every_other_draft_format_is_excluded() {
        for (mode, name) in [
            (2, "captains mode"),
            (3, "random draft"),
            (4, "single draft"),
            (5, "all random"),
            (12, "least played"),
            (15, "custom"),
            (16, "captains draft"),
            (17, "balanced draft"),
            (18, "ability draft"),
            (19, "event"),
            (20, "all random deathmatch"),
            (21, "1v1 mid"),
            (24, "mutation"),
        ] {
            assert_eq!(
                classify(Some(mode), RANKED),
                Err(ExclusionReason::OtherGameMode),
                "{name} must not reach the coaching dataset",
            );
        }
    }

    #[test]
    fn all_pick_outside_public_matchmaking_is_excluded() {
        for (lobby, name) in [
            (1, "practice"),
            (2, "tournament"),
            (4, "co-op bots"),
            (8, "1v1 mid"),
            (9, "battle cup"),
            (12, "event"),
        ] {
            assert_eq!(
                classify(Some(game_mode::ALL_DRAFT), Some(lobby)),
                Err(ExclusionReason::NonPublicLobby),
                "an All Pick game in a {name} lobby is not ladder play",
            );
        }
    }

    #[test]
    fn an_unreported_mode_is_excluded_rather_than_assumed() {
        assert_eq!(classify(None, RANKED), Err(ExclusionReason::ModeUnknown));
        assert_eq!(
            classify(Some(game_mode::ALL_DRAFT), None),
            Err(ExclusionReason::ModeUnknown)
        );
        assert_eq!(classify(None, None), Err(ExclusionReason::ModeUnknown));
    }

    /// The drift guard. If somebody adds a mode to the constants and not to the
    /// SQL — or edits the SQL by hand — this fails rather than letting two
    /// different match populations exist inside one application.
    #[test]
    fn the_sql_predicate_lists_exactly_the_same_ids_as_the_rust_rule() {
        let sql = sql_predicate("m");

        let arrays: Vec<Vec<i32>> = sql
            .split("ARRAY[")
            .skip(1)
            .map(|chunk| {
                chunk
                    .split(']')
                    .next()
                    .unwrap()
                    .split(',')
                    .map(|v| v.trim().parse().unwrap())
                    .collect()
            })
            .collect();

        assert_eq!(arrays.len(), 2, "one array per dimension");
        assert_eq!(arrays[0], ELIGIBLE_GAME_MODES);
        assert_eq!(arrays[1], ELIGIBLE_LOBBY_TYPES);
        assert!(!arrays[0].contains(&game_mode::TURBO));
    }

    #[test]
    fn the_sql_predicate_reads_the_columns_off_the_alias_it_was_given() {
        let sql = sql_predicate("m");
        assert!(sql.contains("m.game_mode"));
        assert!(sql.contains("m.lobby_type"));
    }

    fn count(game_mode: Option<i32>, lobby_type: Option<i32>, matches: i64) -> ModeCount {
        ModeCount {
            game_mode,
            lobby_type,
            matches,
        }
    }

    #[test]
    fn every_match_gets_a_label_that_does_not_overclaim() {
        assert_eq!(
            mode_label(Some(game_mode::ALL_DRAFT), RANKED),
            "Ranked All Pick"
        );
        assert_eq!(mode_label(Some(game_mode::ALL_DRAFT), PUBLIC), "All Pick");
        assert_eq!(mode_label(Some(game_mode::TURBO), PUBLIC), "Turbo");
        // An id this module does not track is described, not named.
        assert_eq!(mode_label(Some(18), RANKED), "Other mode");
        assert_eq!(
            mode_label(Some(game_mode::ALL_DRAFT), Some(2)),
            "Private lobby"
        );
        assert_eq!(mode_label(None, None), "Unknown mode");
    }

    #[test]
    fn a_summary_accounts_for_every_stored_match() {
        let summary = summarize(&[
            count(Some(game_mode::ALL_DRAFT), RANKED, 30),
            count(Some(game_mode::ALL_DRAFT), PUBLIC, 20),
            count(Some(game_mode::TURBO), PUBLIC, 50),
            count(Some(18), PUBLIC, 7),
            count(None, None, 3),
        ]);

        assert_eq!(summary.total_matches, 110);
        assert_eq!(summary.eligible_matches, 50, "30 ranked + 20 public");

        let turbo = &summary.excluded[0];
        assert_eq!(turbo.reason, ExclusionReason::Turbo);
        assert_eq!(turbo.matches, 50, "largest group leads");

        let total_excluded: i64 = summary.excluded.iter().map(|g| g.matches).sum();
        assert_eq!(
            total_excluded, 60,
            "nothing may vanish between the two sides"
        );
    }

    #[test]
    fn groups_with_the_same_reason_are_merged() {
        // Two different ineligible drafts are one reason with one count.
        let summary = summarize(&[count(Some(2), RANKED, 4), count(Some(3), PUBLIC, 6)]);

        assert_eq!(summary.excluded.len(), 1);
        assert_eq!(summary.excluded[0].reason, ExclusionReason::OtherGameMode);
        assert_eq!(summary.excluded[0].matches, 10);
        assert_eq!(summary.eligible_matches, 0);
    }
}
