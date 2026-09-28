//! Turning one match's timeline into measured, timestamped evidence.
//!
//! This is the deterministic half of single-match coaching, and the reason the
//! model is able to say anything a player can check. Every sentence composed
//! here carries a second the provider reported, and every second in it came from
//! an event the provider recorded.
//!
//! Three rules, all of them enforced by the code rather than asked of the model:
//!
//!   1. **Nothing is derived that the provider did not state.** A death's
//!      circumstances are the provider's own flags; where a flag is absent, no
//!      statement is made about it. `Option<bool>` is honoured as three values,
//!      not two.
//!   2. **A pattern needs more than one instance.** One burst death is noise, and
//!      a sentence about it invites the model to call it a habit. The thresholds
//!      are named constants below, not magic numbers in a branch.
//!   3. **An absence is stated, never implied.** A match with no parsed replay
//!      produces an explicit "there is no timeline" item, because a model handed
//!      aggregates and silence will reach for a timeline anyway.
//!
//! Pure: no database, no provider, no clock. The exact text a model will be
//! shown is a function of its arguments, which is what makes it testable and
//! what lets the analysis be cached by a hash of its evidence.

use crate::domain::coaching::{Evidence, EvidenceKind};
use crate::domain::match_facts::{clock, DeathEvent, MatchFacts};
use crate::services::benchmarks::percentile;

/// A death this soon after respawning is treated as walking back into the same
/// fight. Chosen to be longer than a mid-game respawn but shorter than the time
/// it takes to cross the map and make a new decision.
const REPEAT_DEATH_SECONDS: i32 = 45;

/// A death this soon before the player's own tower fell is reported next to it.
/// Not a claim of causation — the statement says "shortly before", and the model
/// is shown both timestamps so the reader can judge.
const DEATH_BEFORE_TOWER_SECONDS: i32 = 45;

/// How many instances of a circumstance make it worth naming.
///
/// Two, not one. A single burst death is something that happened; two is
/// something the player did. Rule 2 of the module, as a number.
const MIN_INSTANCES: usize = 2;

/// Timestamps listed in one statement before it is summarized instead.
const MAX_LISTED_DEATHS: usize = 8;
const MAX_LISTED_TIMINGS: usize = 12;
const MAX_LISTED_OBJECTIVES: usize = 6;

/// Why no timeline is available for a match.
///
/// Carried as a type rather than a string because the three cases mean different
/// things to a reader and to an operator: one is Valve's, one is ours, and one is
/// nobody's fault yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    /// No STRATZ credentials on this deployment. Waiting will not help.
    NotConfigured,
    /// The provider was asked and could not answer.
    ProviderFailed,
    /// The provider answered, and does not have this match — or does not show
    /// this player in it, which is what an anonymous Dota profile looks like from
    /// the outside.
    ///
    /// Distinct from a 404 for the request: the match is the player's own, and
    /// this application has it. Only the second provider does not.
    NotFound,
    /// The provider answered, and Valve has not parsed the replay.
    NotParsed,
}

impl Unavailable {
    /// The sentence a player reads, and the one the model is shown.
    ///
    /// Written in the same voice as every other evidence statement on purpose:
    /// the absence of data is a measured fact about this match, and demoting it
    /// to a UI footnote is how it stops reaching the model at all.
    fn statement(self, provider: &str) -> String {
        match self {
            Unavailable::NotConfigured => format!(
                "No second-by-second timeline is available for this match: this deployment has no \
                 {provider} access configured. Everything below is the match totals, so nothing \
                 here can say when anything happened."
            ),
            Unavailable::ProviderFailed => format!(
                "No second-by-second timeline is available for this match: {provider} could not be \
                 reached. Everything below is the match totals, so nothing here can say when \
                 anything happened. Trying again later may work."
            ),
            Unavailable::NotFound => format!(
                "No second-by-second timeline is available for this match: {provider} does not have \
                 it, or does not show you as one of its players — which is what a Dota profile set \
                 to private looks like from outside. Everything below is the match totals, so \
                 nothing here can say when anything happened."
            ),
            Unavailable::NotParsed => format!(
                "No second-by-second timeline exists for this match: {provider} has it, but the \
                 replay was never parsed, which is the case for most public games. Deaths, item \
                 timings and objectives cannot be placed in time. Everything below is the match \
                 totals."
            ),
        }
    }
}

/// The one evidence item that says a timeline is missing.
///
/// A stable id, so an insight that acknowledges the gap can cite it — and so the
/// model has something to point at instead of inventing a timestamp.
pub fn unavailable(reason: Unavailable, provider: &str) -> Evidence {
    Evidence {
        id: "match.timeline.unavailable".to_string(),
        kind: EvidenceKind::Match,
        label: "Match timeline".to_string(),
        statement: reason.statement(provider),
        sample: 1,
        confidence: percentile::confidence_for(1),
    }
}

/// Build the timestamped evidence for one match.
///
/// Returns an empty vector when there is nothing timed to say — the caller pairs
/// that with [`unavailable`] rather than this function inventing a reason it
/// does not know.
pub fn build(facts: &MatchFacts) -> Vec<Evidence> {
    if !facts.has_timeline() {
        return Vec::new();
    }

    let mut out = Vec::new();

    deaths(facts, &mut out);
    death_cost(facts, &mut out);
    repeat_deaths(facts, &mut out);
    circumstances(facts, &mut out);
    item_timings(facts, &mut out);
    objectives(facts, &mut out);
    deaths_before_lost_towers(facts, &mut out);

    out
}

/// Every death, at the second it happened, with who did it.
///
/// The foundation the rest of the timeline rests on: a claim about fight
/// selection is only checkable if the fights have times on them.
fn deaths(facts: &MatchFacts, out: &mut Vec<Evidence>) {
    if facts.deaths.is_empty() {
        // Not a gap. A death-free game is a fact worth stating, and stating it
        // stops the model reading the absence of death evidence as an absence of
        // measurement.
        push(
            out,
            "match.timeline.deaths",
            "Deaths",
            "You did not die in this match.".to_string(),
        );
        return;
    }

    let listed: Vec<String> = facts
        .deaths
        .iter()
        .take(MAX_LISTED_DEATHS)
        .map(|death| match death.killer_hero_name.as_deref() {
            Some(killer) => format!("{} to {killer}", clock(death.time_seconds)),
            None => clock(death.time_seconds),
        })
        .collect();

    let remaining = facts.deaths.len().saturating_sub(listed.len());
    let tail = if remaining > 0 {
        format!(", and {remaining} more")
    } else {
        String::new()
    };

    push(
        out,
        "match.timeline.deaths",
        "Deaths, with times",
        format!(
            "You died {} {}, at {}{tail}.",
            facts.deaths.len(),
            plural(facts.deaths.len(), "time", "times"),
            listed.join(", "),
        ),
    );
}

/// What the deaths cost, in the two currencies a death is actually paid in.
///
/// Gold and time, both summed from per-event figures the provider reported. The
/// share of match duration is the part that makes it land: "four minutes dead"
/// is abstract, "four minutes of a forty-two minute game" is a decision.
fn death_cost(facts: &MatchFacts, out: &mut Vec<Evidence>) {
    let gold: i32 = facts.deaths.iter().filter_map(|d| d.gold_lost).sum();
    let dead: i32 = facts
        .deaths
        .iter()
        .filter_map(|d| d.time_dead_seconds)
        .sum();

    // Only claim a total when the provider reported the parts. A sum over a
    // partially-populated list is a number nobody measured.
    let has_gold = facts.deaths.iter().all(|d| d.gold_lost.is_some());
    let has_dead = facts.deaths.iter().all(|d| d.time_dead_seconds.is_some());

    if has_gold && gold > 0 {
        push(
            out,
            "match.timeline.gold_lost",
            "Gold lost to deaths",
            format!("Your deaths cost you {gold} gold in this match."),
        );
    }

    if has_dead && dead > 0 && facts.duration_seconds > 0 {
        let share = (dead as f32 / facts.duration_seconds as f32) * 100.0;
        push(
            out,
            "match.timeline.time_dead",
            "Time spent dead",
            format!(
                "You spent {} waiting to respawn, which is {share:.0}% of a {} match.",
                duration(dead),
                duration(facts.duration_seconds),
            ),
        );
    }
}

/// Deaths that happened straight after the previous respawn.
///
/// The gap is measured from when the player came back, not from when they last
/// died, which is why `time_dead_seconds` is required for it: without the
/// respawn timer, "died twice within a minute" cannot tell a chain-death from a
/// long fight.
fn repeat_deaths(facts: &MatchFacts, out: &mut Vec<Evidence>) {
    let mut instances: Vec<(i32, i32)> = Vec::new();

    for pair in facts.deaths.windows(2) {
        let (previous, next) = (&pair[0], &pair[1]);
        let Some(dead_for) = previous.time_dead_seconds else {
            continue;
        };

        let respawned_at = previous.time_seconds + dead_for;
        let alive_for = next.time_seconds - respawned_at;

        // Negative would mean the events overlap, which they cannot; treating it
        // as "not measurable" is safer than reporting a negative gap.
        if (0..=REPEAT_DEATH_SECONDS).contains(&alive_for) {
            instances.push((next.time_seconds, alive_for));
        }
    }

    if instances.is_empty() {
        return;
    }

    let listed: Vec<String> = instances
        .iter()
        .map(|(at, alive)| format!("{} ({alive}s after respawning)", clock(*at)))
        .collect();

    push(
        out,
        "match.timeline.repeat_deaths",
        "Deaths soon after respawning",
        format!(
            "{} of your deaths came within {REPEAT_DEATH_SECONDS} seconds of respawning: {}.",
            instances.len(),
            listed.join(", "),
        ),
    );
}

/// The provider's own judgements about how the player died.
///
/// Each is reported only at [`MIN_INSTANCES`] or more, and only from events where
/// the provider actually set the flag — a `None` is not a `false`.
fn circumstances(facts: &MatchFacts, out: &mut Vec<Evidence>) {
    for (id, label, sentence, flag) in [
        (
            "match.timeline.burst_deaths",
            "Deaths with no time to react",
            "killed faster than you could respond",
            (|d: &DeathEvent| d.was_burst) as fn(&DeathEvent) -> Option<bool>,
        ),
        (
            "match.timeline.unused_heal",
            "Deaths with a heal still available",
            "killed while you still had a heal or salve available",
            |d: &DeathEvent| d.had_heal_available,
        ),
        (
            "match.timeline.late_escape",
            "Deaths escaping too late",
            "killed after starting a teleport out that did not finish",
            |d: &DeathEvent| d.attempted_to_escape,
        ),
        (
            "match.timeline.deaths_outside_fights",
            "Deaths caught alone",
            "killed while not already in a fight",
            // Inverted deliberately: `false` here is the finding. Being caught
            // out of a fight is a positioning decision, and being killed inside
            // one frequently is not a mistake at all.
            |d: &DeathEvent| d.was_in_a_fight.map(|engaged| !engaged),
        ),
    ] {
        let times: Vec<i32> = facts
            .deaths
            .iter()
            .filter(|d| flag(d) == Some(true))
            .map(|d| d.time_seconds)
            .collect();

        if times.len() < MIN_INSTANCES {
            continue;
        }

        let listed: Vec<String> = times.iter().copied().map(clock).collect();
        push(
            out,
            id,
            label,
            format!(
                "{} of your {} deaths were {sentence}, at {}. This is the match data's own \
                 reading of those deaths, not an inference drawn from them.",
                times.len(),
                facts.deaths.len(),
                listed.join(", "),
            ),
        );
    }
}

/// When the player's items came online.
///
/// "Significance" is decided by purchase count rather than by a hardcoded item
/// list: consumables — tangoes, salves, wards, teleport scrolls — are bought
/// repeatedly, and a keep is bought once. That rule needs no knowledge of the
/// item roster, so it does not rot with a patch, and it cannot silently drop a
/// newly-added item the way a hardcoded list would.
fn item_timings(facts: &MatchFacts, out: &mut Vec<Evidence>) {
    if facts.purchases.is_empty() {
        return;
    }

    let keeps: Vec<&crate::domain::match_facts::PurchaseEvent> = facts
        .purchases
        .iter()
        .filter(|purchase| {
            // An unnamed id is not information a player can use, and a name is
            // what makes a timing coachable.
            purchase.item_name.is_some()
                && facts
                    .purchases
                    .iter()
                    .filter(|other| other.item_id == purchase.item_id)
                    .count()
                    == 1
        })
        .collect();

    if keeps.is_empty() {
        return;
    }

    let listed: Vec<String> = keeps
        .iter()
        .take(MAX_LISTED_TIMINGS)
        .map(|purchase| {
            format!(
                "{} at {}",
                purchase.item_name.as_deref().unwrap_or_default(),
                clock(purchase.time_seconds),
            )
        })
        .collect();

    let remaining = keeps.len().saturating_sub(listed.len());
    let tail = if remaining > 0 {
        format!(", and {remaining} more")
    } else {
        String::new()
    };

    push(
        out,
        "match.timeline.items",
        "Item timings",
        format!("You completed {}{tail}.", listed.join(", ")),
    );
}

/// Towers and Roshan, on both sides of the map.
///
/// Objectives are the scoreboard a Dota match is actually decided on, and their
/// timestamps are what let a death be read as expensive or free.
fn objectives(facts: &MatchFacts, out: &mut Vec<Evidence>) {
    let taken: Vec<String> = facts
        .towers_taken()
        .take(MAX_LISTED_OBJECTIVES)
        .map(|t| clock(t.time_seconds))
        .collect();
    let lost: Vec<String> = facts
        .towers_lost()
        .take(MAX_LISTED_OBJECTIVES)
        .map(|t| clock(t.time_seconds))
        .collect();

    if !taken.is_empty() || !lost.is_empty() {
        let mut statement = String::new();
        if taken.is_empty() {
            statement.push_str("Your team took no towers in this match. ");
        } else {
            statement.push_str(&format!(
                "Your team took {} at {}. ",
                count_of(facts.towers_taken().count(), "tower", "towers"),
                taken.join(", "),
            ));
        }
        if lost.is_empty() {
            statement.push_str("Your team lost none.");
        } else {
            statement.push_str(&format!(
                "Your team lost {} at {}.",
                count_of(facts.towers_lost().count(), "tower", "towers"),
                lost.join(", "),
            ));
        }

        push(out, "match.timeline.towers", "Towers", statement);
    }

    if !facts.roshan_kills.is_empty() {
        let listed: Vec<String> = facts
            .roshan_kills
            .iter()
            .take(MAX_LISTED_OBJECTIVES)
            .copied()
            .map(clock)
            .collect();

        push(
            out,
            "match.timeline.roshan",
            "Roshan",
            format!(
                "Roshan died {} in this match, at {}. The match data does not say which team \
                 killed him.",
                count_of(facts.roshan_kills.len(), "time", "times"),
                listed.join(", "),
            ),
        );
    }
}

/// Deaths that landed shortly before the player's own team lost a tower.
///
/// Stated as adjacency, never as cause. The sentence gives both timestamps and
/// the gap, and says outright what it is not claiming — because "your death cost
/// you the tower" is exactly the sort of confident, unfalsifiable sentence this
/// pipeline exists to prevent.
fn deaths_before_lost_towers(facts: &MatchFacts, out: &mut Vec<Evidence>) {
    let lost: Vec<i32> = facts.towers_lost().map(|t| t.time_seconds).collect();
    if lost.is_empty() || facts.deaths.is_empty() {
        return;
    }

    let mut pairs: Vec<(i32, i32)> = Vec::new();
    for tower in &lost {
        if let Some(death) = facts
            .deaths
            .iter()
            .filter(|d| {
                let gap = tower - d.time_seconds;
                (0..=DEATH_BEFORE_TOWER_SECONDS).contains(&gap)
            })
            // The closest one, so a single tower is reported once rather than
            // once per death near it.
            .min_by_key(|d| tower - d.time_seconds)
        {
            pairs.push((death.time_seconds, *tower));
        }
    }

    if pairs.is_empty() {
        return;
    }

    let listed: Vec<String> = pairs
        .iter()
        .map(|(death, tower)| {
            format!(
                "died at {} and the tower fell at {} ({}s later)",
                clock(*death),
                clock(*tower),
                tower - death,
            )
        })
        .collect();

    push(
        out,
        "match.timeline.deaths_before_objectives",
        "Deaths before lost towers",
        format!(
            "On {} {}, your team lost a tower within {DEATH_BEFORE_TOWER_SECONDS} seconds of one \
             of your deaths: you {}. These are two timestamps close together; the match data does \
             not establish that one caused the other.",
            pairs.len(),
            plural(pairs.len(), "occasion", "occasions"),
            listed.join("; "),
        ),
    );
}

fn push(out: &mut Vec<Evidence>, id: &str, label: &str, statement: String) {
    out.push(Evidence {
        id: id.to_string(),
        kind: EvidenceKind::Match,
        label: label.to_string(),
        statement,
        // One match. Exact, and generalizing to nothing — which is what the
        // confidence below is for, and why the model is told not to call a
        // single game a pattern.
        sample: 1,
        confidence: percentile::confidence_for(1),
    })
}

/// `4:12` reads as a clock; `4m 12s` reads as an elapsed span. Time spent dead
/// is a span, and rendering it as a clock invites it to be read as a timestamp.
fn duration(seconds: i32) -> String {
    let (minutes, rest) = (seconds / 60, seconds % 60);
    match (minutes, rest) {
        (0, s) => format!("{s}s"),
        (m, 0) => format!("{m}m"),
        (m, s) => format!("{m}m {s}s"),
    }
}

fn count_of(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn plural(count: usize, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 {
        one
    } else {
        many
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::match_facts::{MatchFactsPlayer, PurchaseEvent, TowerEvent};
    use chrono::Utc;

    fn death(time: i32, dead_for: Option<i32>) -> DeathEvent {
        DeathEvent {
            time_seconds: time,
            killer_hero_id: Some(26),
            killer_hero_name: Some("Lion".into()),
            gold_lost: Some(200),
            gold_fed: Some(150),
            time_dead_seconds: dead_for,
            was_burst: None,
            had_heal_available: None,
            was_in_a_fight: None,
            attempted_to_escape: None,
        }
    }

    fn facts() -> MatchFacts {
        MatchFacts {
            match_id: 7_500_000_001,
            duration_seconds: 2400,
            started_at: Utc::now(),
            won: false,
            parsed: true,
            player: MatchFactsPlayer {
                account_id: 1,
                hero_id: 35,
                hero_name: Some("Sniper".into()),
                is_radiant: true,
                lane: Some("Safe lane".into()),
                position: Some("POSITION_1".into()),
                kills: 4,
                deaths: 3,
                assists: 6,
                gpm: 480,
                xpm: 520,
                last_hits: 240,
                denies: Some(8),
                net_worth: Some(18_000),
                level: Some(22),
                hero_damage: Some(20_000),
                tower_damage: Some(2_000),
                hero_healing: Some(0),
                net_worth_per_minute: vec![0, 300, 700],
                last_hits_per_minute: vec![0, 4, 6],
            },
            deaths: vec![death(600, Some(20)), death(1200, Some(30))],
            purchases: vec![PurchaseEvent {
                time_seconds: 862,
                item_id: 1,
                item_name: Some("Blink Dagger".into()),
            }],
            towers: Vec::new(),
            roshan_kills: Vec::new(),
        }
    }

    fn find<'a>(items: &'a [Evidence], id: &str) -> Option<&'a Evidence> {
        items.iter().find(|e| e.id == id)
    }

    fn statement<'a>(items: &'a [Evidence], id: &str) -> &'a str {
        find(items, id)
            .map(|e| e.statement.as_str())
            .unwrap_or_else(|| panic!("no evidence with id {id}"))
    }

    #[test]
    fn every_death_reaches_the_evidence_with_its_timestamp_and_killer() {
        let items = build(&facts());
        let deaths = statement(&items, "match.timeline.deaths");

        assert!(deaths.contains("10:00 to Lion"), "got: {deaths}");
        assert!(deaths.contains("20:00 to Lion"), "got: {deaths}");
        assert!(deaths.contains("died 2 times"));
    }

    /// The whole point of the module: no timestamp is ever composed from
    /// anything but an event the provider timed.
    #[test]
    fn an_unparsed_match_produces_no_timeline_evidence_at_all() {
        let mut unparsed = facts();
        unparsed.parsed = false;

        assert!(build(&unparsed).is_empty());
    }

    #[test]
    fn a_missing_timeline_is_stated_in_the_same_voice_as_a_measurement() {
        for reason in [
            Unavailable::NotConfigured,
            Unavailable::ProviderFailed,
            Unavailable::NotFound,
            Unavailable::NotParsed,
        ] {
            let item = unavailable(reason, "STRATZ");

            assert_eq!(item.id, "match.timeline.unavailable");
            assert_eq!(item.kind, EvidenceKind::Match);
            // A model shown "no timeline" must know what it may not conclude.
            assert!(
                item.statement.contains("totals"),
                "{reason:?} does not say what is available instead"
            );
        }

        // The three are distinguishable: a user must not be told to try again
        // when nothing is configured.
        assert!(unavailable(Unavailable::NotConfigured, "STRATZ")
            .statement
            .contains("configured"));
        assert!(unavailable(Unavailable::ProviderFailed, "STRATZ")
            .statement
            .contains("Trying again"));
        assert!(unavailable(Unavailable::NotParsed, "STRATZ")
            .statement
            .contains("never parsed"));
    }

    /// A death-free game is a fact. Reporting nothing would leave the model with
    /// aggregates and silence, and silence is what it fills in.
    #[test]
    fn a_match_with_no_deaths_says_so_rather_than_omitting_the_item() {
        let mut clean = facts();
        clean.deaths = Vec::new();
        // Keep a purchase so the match still has a timeline at all.
        let items = build(&clean);

        assert_eq!(
            statement(&items, "match.timeline.deaths"),
            "You did not die in this match."
        );
    }

    #[test]
    fn the_cost_of_dying_is_summed_in_gold_and_in_time() {
        let items = build(&facts());

        assert!(statement(&items, "match.timeline.gold_lost").contains("400 gold"));

        let dead = statement(&items, "match.timeline.time_dead");
        assert!(dead.contains("50s"), "got: {dead}");
        // 50 seconds of a 2400-second match.
        assert!(dead.contains("2%"), "got: {dead}");
    }

    /// A sum over a partially-reported list is a number nobody measured, and it
    /// would be indistinguishable from a real one.
    #[test]
    fn a_total_is_not_claimed_when_the_provider_reported_only_some_of_the_parts() {
        let mut partial = facts();
        partial.deaths[1].gold_lost = None;
        partial.deaths[1].time_dead_seconds = None;

        let items = build(&partial);

        assert!(find(&items, "match.timeline.gold_lost").is_none());
        assert!(find(&items, "match.timeline.time_dead").is_none());
        // The deaths themselves are still reported.
        assert!(find(&items, "match.timeline.deaths").is_some());
    }

    #[test]
    fn a_death_straight_after_respawning_is_measured_from_the_respawn() {
        let mut chained = facts();
        // Died at 10:00, dead for 20s, so alive again at 10:20 — and dead again
        // 25 seconds later.
        chained.deaths = vec![death(600, Some(20)), death(645, Some(20))];

        let items = build(&chained);
        let statement = statement(&items, "match.timeline.repeat_deaths");
        assert!(
            statement.contains("10:45 (25s after respawning)"),
            "got: {statement}"
        );
    }

    #[test]
    fn deaths_far_apart_are_not_reported_as_repeats() {
        assert!(find(&build(&facts()), "match.timeline.repeat_deaths").is_none());
    }

    /// Without the respawn timer the gap cannot be measured, and guessing it
    /// from the previous death would turn a long fight into a chain-death.
    #[test]
    fn a_death_with_no_respawn_timer_cannot_produce_a_repeat_finding() {
        let mut unknown = facts();
        unknown.deaths = vec![death(600, None), death(620, None)];

        assert!(find(&build(&unknown), "match.timeline.repeat_deaths").is_none());
    }

    #[test]
    fn a_provider_judgement_is_reported_once_it_happened_twice() {
        let mut burst = facts();
        burst.deaths[0].was_burst = Some(true);

        // One instance is something that happened, not something the player does.
        assert!(find(&build(&burst), "match.timeline.burst_deaths").is_none());

        burst.deaths[1].was_burst = Some(true);
        let items = build(&burst);
        let statement = statement(&items, "match.timeline.burst_deaths");
        assert!(statement.contains("10:00, 20:00"), "got: {statement}");
        assert!(statement.contains("2 of your 2 deaths"));
    }

    /// `None` is not `false`. A provider that did not judge a death must not have
    /// a judgement attributed to it.
    #[test]
    fn an_unset_flag_is_never_counted_as_the_finding() {
        let mut unset = facts();
        for death in &mut unset.deaths {
            death.was_burst = None;
            death.had_heal_available = None;
            death.attempted_to_escape = None;
            death.was_in_a_fight = None;
        }
        let items = build(&unset);

        for id in [
            "match.timeline.burst_deaths",
            "match.timeline.unused_heal",
            "match.timeline.late_escape",
            "match.timeline.deaths_outside_fights",
        ] {
            assert!(
                find(&items, id).is_none(),
                "{id} was claimed from a null flag"
            );
        }
    }

    /// Being killed in a fight is frequently not a mistake; being caught alone
    /// is a positioning decision. The inversion is the finding.
    #[test]
    fn being_caught_outside_a_fight_is_the_finding_rather_than_dying_in_one() {
        let mut caught = facts();
        for death in &mut caught.deaths {
            death.was_in_a_fight = Some(false);
        }
        assert!(find(&build(&caught), "match.timeline.deaths_outside_fights").is_some());

        let mut in_fights = facts();
        for death in &mut in_fights.deaths {
            death.was_in_a_fight = Some(true);
        }
        assert!(find(&build(&in_fights), "match.timeline.deaths_outside_fights").is_none());
    }

    /// The rule that replaces a hardcoded item list: bought once is a keep,
    /// bought repeatedly is a consumable.
    #[test]
    fn repeatedly_bought_items_are_left_out_of_the_timings() {
        let mut shopping = facts();
        shopping.purchases = vec![
            PurchaseEvent {
                time_seconds: -30,
                item_id: 44,
                item_name: Some("Tango".into()),
            },
            PurchaseEvent {
                time_seconds: 240,
                item_id: 44,
                item_name: Some("Tango".into()),
            },
            PurchaseEvent {
                time_seconds: 480,
                item_id: 44,
                item_name: Some("Tango".into()),
            },
            PurchaseEvent {
                time_seconds: 862,
                item_id: 1,
                item_name: Some("Blink Dagger".into()),
            },
            PurchaseEvent {
                time_seconds: 1304,
                item_id: 116,
                item_name: Some("Black King Bar".into()),
            },
        ];

        let items = build(&shopping);
        let statement = statement(&items, "match.timeline.items");
        assert!(
            statement.contains("Blink Dagger at 14:22"),
            "got: {statement}"
        );
        assert!(statement.contains("Black King Bar at 21:44"));
        assert!(
            !statement.contains("Tango"),
            "a consumable reached the timings: {statement}"
        );
    }

    #[test]
    fn an_item_the_catalogue_could_not_name_is_left_out_rather_than_shown_as_an_id() {
        let mut unnamed = facts();
        unnamed.purchases = vec![PurchaseEvent {
            time_seconds: 600,
            item_id: 9999,
            item_name: None,
        }];

        assert!(find(&build(&unnamed), "match.timeline.items").is_none());
    }

    #[test]
    fn towers_are_reported_from_the_players_own_side() {
        let mut objectives = facts();
        objectives.towers = vec![
            TowerEvent {
                time_seconds: 900,
                was_radiant_tower: false,
            },
            TowerEvent {
                time_seconds: 1500,
                was_radiant_tower: true,
            },
            TowerEvent {
                time_seconds: 1800,
                was_radiant_tower: true,
            },
        ];

        // The fixture player is Radiant.
        let items = build(&objectives);
        let statement = statement(&items, "match.timeline.towers");
        assert!(
            statement.contains("took 1 tower at 15:00"),
            "got: {statement}"
        );
        assert!(
            statement.contains("lost 2 towers at 25:00, 30:00"),
            "got: {statement}"
        );
    }

    #[test]
    fn roshan_timings_do_not_claim_a_side_the_data_does_not_carry() {
        let mut rosh = facts();
        rosh.roshan_kills = vec![1_500, 2_100];

        let items = build(&rosh);
        let statement = statement(&items, "match.timeline.roshan");
        assert!(statement.contains("25:00, 35:00"));
        assert!(statement.contains("does not say which team"));
    }

    /// Adjacency, stated as adjacency. The disclaimer is part of the evidence,
    /// not a UI footnote, because the model reads the evidence and not the UI.
    #[test]
    fn a_death_near_a_lost_tower_is_reported_without_claiming_causation() {
        let mut near = facts();
        near.deaths = vec![death(600, Some(20))];
        near.towers = vec![TowerEvent {
            time_seconds: 630,
            was_radiant_tower: true,
        }];

        let items = build(&near);
        let statement = statement(&items, "match.timeline.deaths_before_objectives");
        assert!(statement.contains("died at 10:00"), "got: {statement}");
        assert!(statement.contains("10:30 (30s later)"), "got: {statement}");
        assert!(statement.contains("does not establish that one caused the other"));
    }

    #[test]
    fn a_death_long_before_a_lost_tower_is_not_linked_to_it() {
        let mut apart = facts();
        apart.deaths = vec![death(600, Some(20))];
        apart.towers = vec![TowerEvent {
            time_seconds: 1_800,
            was_radiant_tower: true,
        }];

        assert!(find(&build(&apart), "match.timeline.deaths_before_objectives").is_none());
    }

    /// A death *after* a tower fell says nothing about losing it.
    #[test]
    fn a_death_after_the_tower_fell_is_not_linked_to_it() {
        let mut after = facts();
        after.deaths = vec![death(700, Some(20))];
        after.towers = vec![TowerEvent {
            time_seconds: 690,
            was_radiant_tower: true,
        }];

        assert!(find(&build(&after), "match.timeline.deaths_before_objectives").is_none());
    }

    #[test]
    fn every_item_is_kinded_as_match_evidence_with_a_unique_id() {
        let mut full = facts();
        full.deaths = vec![death(600, Some(20)), death(645, Some(20))];
        for death in &mut full.deaths {
            death.was_burst = Some(true);
            death.had_heal_available = Some(true);
            death.attempted_to_escape = Some(true);
            death.was_in_a_fight = Some(false);
        }
        full.towers = vec![TowerEvent {
            time_seconds: 670,
            was_radiant_tower: true,
        }];
        full.roshan_kills = vec![1_500];

        let items = build(&full);
        let mut ids: Vec<&str> = items.iter().map(|e| e.id.as_str()).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();

        assert_eq!(ids.len(), total, "a citation would be ambiguous");
        assert!(items.iter().all(|e| e.kind == EvidenceKind::Match));
        assert!(items.iter().all(|e| e.sample == 1));
    }

    /// The stored analysis is keyed by a hash of its evidence, so identical
    /// inputs have to produce identical text.
    #[test]
    fn the_same_match_always_produces_the_same_evidence() {
        assert_eq!(build(&facts()), build(&facts()));
    }

    #[test]
    fn a_span_reads_as_a_span_and_not_as_a_clock() {
        assert_eq!(duration(45), "45s");
        assert_eq!(duration(120), "2m");
        assert_eq!(duration(135), "2m 15s");
    }
}
