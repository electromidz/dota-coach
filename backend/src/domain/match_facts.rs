//! The timestamped record of one match.
//!
//! [`crate::domain::r#match::Match`] is the *aggregate* of a game — totals, per
//! minute rates, and the handful of time-sliced figures OpenDota exposes. This
//! module is the other half: the individual events that happened, each with the
//! second it happened at.
//!
//! Single-match coaching needs that distinction. "You had seven deaths" is an
//! aggregate and is not coachable; "you died at 18:42 and again at 19:31,
//! twenty-nine seconds after respawning" names a decision. Nothing here is
//! derived or estimated — every field is something the provider stated, and
//! anything the provider did not state is `None` rather than zero.
//!
//! Provider-agnostic by construction: STRATZ shapes stop at
//! [`crate::services::match_facts`], the same way OpenDota shapes stop at
//! [`crate::services::dota`].

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One match as the provider timed it, for the player who asked.
///
/// `Serialize`/`Deserialize` because the normalized form is what gets cached —
/// re-fetching a finished match from a rate-limited provider to answer the same
/// question twice is waste, and a finished match does not change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchFacts {
    pub match_id: i64,
    pub duration_seconds: i32,
    pub started_at: DateTime<Utc>,
    pub won: bool,
    /// True when the provider has parsed the replay. Every event list below is
    /// gated on it: without a parse there are no events, and an empty list on
    /// an unparsed match means "not measured", not "none happened".
    ///
    /// This is the distinction the analysis layer must never blur. A player who
    /// died six times in an unparsed match has six deaths and zero death
    /// events, and "you died at no particular time" is not a finding.
    pub parsed: bool,
    pub player: MatchFactsPlayer,
    /// The player's own deaths, in time order.
    pub deaths: Vec<DeathEvent>,
    /// The player's own purchases, in time order.
    pub purchases: Vec<PurchaseEvent>,
    /// Every tower that fell, either side, in time order.
    pub towers: Vec<TowerEvent>,
    /// Every Roshan kill, in time order. The provider does not attribute these
    /// to a side, so neither does this.
    pub roshan_kills: Vec<i32>,
}

/// The requested player's record in the match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchFactsPlayer {
    pub account_id: i64,
    pub hero_id: i32,
    /// Resolved from the provider's own hero catalogue. `None` when the
    /// catalogue could not be read — a cosmetic lookup must not fail a match.
    pub hero_name: Option<String>,
    pub is_radiant: bool,
    /// The provider's lane assignment, verbatim. Unlike our own
    /// [`crate::domain::r#match::derive_role`] estimate, this is something the
    /// provider claims rather than something we inferred, so it is carried as
    /// the provider's own words.
    pub lane: Option<String>,
    /// The provider's position call (`POSITION_1` … `POSITION_5`), verbatim.
    pub position: Option<String>,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub gpm: i32,
    pub xpm: i32,
    pub last_hits: i32,
    pub denies: Option<i32>,
    pub net_worth: Option<i32>,
    pub level: Option<i32>,
    pub hero_damage: Option<i32>,
    pub tower_damage: Option<i32>,
    pub hero_healing: Option<i32>,
    /// Net worth at each completed minute, index 0 = minute 0. Parsed replays
    /// only.
    pub net_worth_per_minute: Vec<i32>,
    /// Last hits in each minute — a per-minute rate, not a running total.
    /// Parsed replays only.
    pub last_hits_per_minute: Vec<i32>,
}

/// One death, with everything the provider knows about the circumstances.
///
/// The flags are the reason this type exists. A death's timestamp says when;
/// these say whether it was a decision. They are carried as `Option<bool>`
/// because "the provider did not say" and "no" are different facts, and only
/// one of them may be coached on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeathEvent {
    /// Seconds from the horn.
    pub time_seconds: i32,
    pub killer_hero_id: Option<i32>,
    pub killer_hero_name: Option<String>,
    /// Gold the player lost on this death.
    pub gold_lost: Option<i32>,
    /// Gold handed to the killer.
    pub gold_fed: Option<i32>,
    /// Seconds spent waiting to respawn.
    pub time_dead_seconds: Option<i32>,
    /// The provider's judgement that the player was killed faster than they
    /// could react.
    pub was_burst: Option<bool>,
    /// The provider's judgement that the player still had a heal or a salve
    /// available when they died.
    pub had_heal_available: Option<bool>,
    /// The provider's judgement that the player was already in a fight.
    pub was_in_a_fight: Option<bool>,
    /// The provider's judgement that the player started a teleport out and did
    /// not finish it.
    pub attempted_to_escape: Option<bool>,
}

/// One item bought, at the second it was bought.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PurchaseEvent {
    /// Seconds from the horn. Negative before it, which is legal: the shop is
    /// open during the loading screen.
    pub time_seconds: i32,
    pub item_id: i32,
    /// Resolved from the provider's item catalogue. `None` when the catalogue
    /// could not be read, or when the id is not in it.
    pub item_name: Option<String>,
}

/// One tower falling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TowerEvent {
    pub time_seconds: i32,
    /// True when the tower that fell was Radiant's. Combine with the player's
    /// own side to decide whether it was a gain or a loss.
    pub was_radiant_tower: bool,
}

impl MatchFacts {
    /// Whether there is any event-level data at all.
    ///
    /// A parsed match with no purchases is possible but vanishingly rare; this
    /// exists so the analysis layer can state "no timeline was available" once,
    /// honestly, rather than silently producing an aggregate-only reading that
    /// looks like a complete one.
    pub fn has_timeline(&self) -> bool {
        self.parsed
            && (!self.deaths.is_empty() || !self.purchases.is_empty() || !self.towers.is_empty())
    }

    /// Towers the player's own team lost.
    pub fn towers_lost(&self) -> impl Iterator<Item = &TowerEvent> {
        let side = self.player.is_radiant;
        self.towers
            .iter()
            .filter(move |t| t.was_radiant_tower == side)
    }

    /// Towers the player's own team took.
    pub fn towers_taken(&self) -> impl Iterator<Item = &TowerEvent> {
        let side = self.player.is_radiant;
        self.towers
            .iter()
            .filter(move |t| t.was_radiant_tower != side)
    }
}

/// `m:ss` from a second offset.
///
/// One place, because a timestamp the player reads has to match the timestamp
/// in the evidence the model read, and two formatters drift.
pub fn clock(seconds: i32) -> String {
    // Pre-horn purchases are reported as negative seconds. "0:00" is the
    // honest rendering: the shopping happened before the clock started.
    if seconds <= 0 {
        return "0:00".to_string();
    }
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(is_radiant: bool, towers: Vec<TowerEvent>) -> MatchFacts {
        MatchFacts {
            match_id: 1,
            duration_seconds: 2400,
            started_at: Utc::now(),
            won: true,
            parsed: true,
            player: MatchFactsPlayer {
                account_id: 1,
                hero_id: 1,
                hero_name: None,
                is_radiant,
                lane: None,
                position: None,
                kills: 0,
                deaths: 0,
                assists: 0,
                gpm: 0,
                xpm: 0,
                last_hits: 0,
                denies: None,
                net_worth: None,
                level: None,
                hero_damage: None,
                tower_damage: None,
                hero_healing: None,
                net_worth_per_minute: Vec::new(),
                last_hits_per_minute: Vec::new(),
            },
            deaths: Vec::new(),
            purchases: Vec::new(),
            towers,
            roshan_kills: Vec::new(),
        }
    }

    #[test]
    fn the_clock_reads_as_a_player_would_say_it() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(9), "0:09");
        assert_eq!(clock(70), "1:10");
        assert_eq!(clock(1122), "18:42");
        assert_eq!(clock(3600), "60:00");
    }

    /// Pre-horn shopping is reported as a negative offset. Rendering it as
    /// "-1:30" would put a purchase before the game in a list of in-game
    /// timestamps.
    #[test]
    fn pre_horn_purchases_do_not_produce_a_negative_timestamp() {
        assert_eq!(clock(-90), "0:00");
    }

    #[test]
    fn towers_are_attributed_from_the_players_own_side() {
        let towers = vec![
            TowerEvent {
                time_seconds: 600,
                was_radiant_tower: true,
            },
            TowerEvent {
                time_seconds: 900,
                was_radiant_tower: false,
            },
        ];

        let radiant = facts(true, towers.clone());
        assert_eq!(radiant.towers_lost().count(), 1);
        assert_eq!(radiant.towers_lost().next().unwrap().time_seconds, 600);
        assert_eq!(radiant.towers_taken().next().unwrap().time_seconds, 900);

        // The same events read the other way round for the other team.
        let dire = facts(false, towers);
        assert_eq!(dire.towers_lost().next().unwrap().time_seconds, 900);
        assert_eq!(dire.towers_taken().next().unwrap().time_seconds, 600);
    }

    /// The distinction the whole module exists to preserve: an unparsed match
    /// has no timeline, and an empty event list on one must not read as "this
    /// player died at no particular time".
    #[test]
    fn an_unparsed_match_never_claims_a_timeline() {
        let mut unparsed = facts(true, Vec::new());
        unparsed.parsed = false;
        unparsed.deaths = vec![DeathEvent {
            time_seconds: 300,
            killer_hero_id: None,
            killer_hero_name: None,
            gold_lost: None,
            gold_fed: None,
            time_dead_seconds: None,
            was_burst: None,
            had_heal_available: None,
            was_in_a_fight: None,
            attempted_to_escape: None,
        }];

        assert!(!unparsed.has_timeline());
    }

    #[test]
    fn a_parsed_match_with_no_events_has_no_timeline_either() {
        assert!(!facts(true, Vec::new()).has_timeline());
    }
}
