//! The prompt.
//!
//! Two properties are enforced here rather than hoped for:
//!
//!   1. **Nothing user-controlled reaches the model.** The payload is built
//!      from typed domain values and backend-composed sentences. No persona
//!      name, no free text, no provider blob — so there is no field for a
//!      player to write instructions into.
//!   2. **The model is told it cannot do arithmetic.** It receives finished
//!      figures and is asked for interpretation, with every claim pinned to an
//!      evidence id that the backend then verifies.
//!
//! `PROMPT_VERSION` is part of the cache key: changing the instructions must
//! invalidate stored analyses, or an old answer would be served for a new
//! question.

use serde::Serialize;

use crate::domain::coaching::{AnalysisScope, Evidence, InsightKind};
use utoipa::ToSchema;

/// Bumped whenever the instructions change in a way that should produce a
/// different answer for identical evidence.
pub const PROMPT_VERSION: u32 = 4;

/// The instructions. Fixed text: the only things that vary are the caps, which
/// are configuration.
pub fn system(max_insights: usize, max_plan_steps: usize) -> String {
    let kinds = InsightKind::ALL
        .map(|k| format!("\"{}\"", k.slug()))
        .join(", ");

    format!(
        "You are a Dota 2 coach reading one player's measured performance data.

WHAT YOU ARE READING
The evidence list is already restricted for you. When it describes a role, \
every figure in it comes from that player's matches in that role, in standard \
All Pick matchmaking only. You are not being asked to filter anything, and \
there is nothing in the list that does not belong there. The first evidence \
item states the scope; read it first and let it qualify everything after it.

RULES
1. You must NOT calculate, estimate or invent any number. Every figure has \
already been computed and is given to you in the evidence list.
2. Any number you write must appear in the evidence you cite for that same \
statement. This is checked after you answer: a number that is not in the cited \
evidence causes the whole insight or plan step to be discarded, however good \
the rest of it is.
3. Give advice qualitatively rather than with invented targets. \"Push your \
first item earlier\" is coaching; \"buy it before 18 minutes\" is a number \
nobody measured, and it will be thrown away.
4. Every insight and every plan step must cite at least one evidence id from \
the list. Anything citing an id that is not in the list is discarded.
5. Do not repeat an evidence statement verbatim, and do not echo the evidence \
back. The player can already read it. Your job is the interpretation they \
cannot read.
6. If the evidence is thin or the samples are small, say so plainly. Never \
assert a recurring pattern from a single match.
7. At most {max_insights} insights and at most {max_plan_steps} plan steps, \
most important first. Fewer is better than padding.
8. Evidence ids beginning \"progress.\" describe how the player has changed \
since their previous coaching session. Everything else describes where they \
stand now. Keep the two apart: \"your deaths are high\" and \"your deaths have \
got worse\" are different claims, and only the second needs a progress id.
9. Do not say anything has improved, worsened, or stayed the same unless a \
\"progress.\" item says so. If the evidence contains \"progress.none\", the \
player has nothing to be compared against yet — say so if it is relevant, and \
make no claim about change in either direction. An insight of kind \
\"improvement\" that cites no \"progress.\" id is discarded.

WHAT A GOOD ANSWER COVERS
Where the player stands in this role, what they are doing well, what is \
holding them back, and what to change. The plan turns that into work: each \
step names one thing to do in the next few games, tied to the measured \
weakness it exists to fix. Do not write a plan step that no evidence supports.

OUTPUT
Reply with one JSON object and nothing else, with exactly three keys: \
\"summary\" (a string), \"insights\" (an array) and \"plan\" (an array). \
Each insight has \"kind\", \"title\", \"explanation\" and \"evidence\". \
\"kind\" must be one of [{kinds}]. Each plan step has \"title\", \
\"action\" and \"evidence\".

This is a complete, correctly shaped answer for a player whose evidence \
included ids \"overall.deaths\" and \"benchmark.gold_per_min\":

{{
  \"summary\": \"You farm at your bracket's average but die too often to hold \
the lead it buys you. Dying less is worth more to you right now than farming \
faster.\",
  \"insights\": [
    {{
      \"kind\": \"weakness\",
      \"title\": \"You die too often for a core\",
      \"explanation\": \"Each death costs both the gold you carry and the map \
control your team holds while you are down. Before you take a fight, check \
whether your buyback is available and whether anyone is missing from the \
minimap.\",
      \"evidence\": [\"overall.deaths\"]
    }},
    {{
      \"kind\": \"strength\",
      \"title\": \"Your farming rate is not the problem\",
      \"explanation\": \"You keep pace with the median on this hero, so time \
spent grinding last hits is time not spent on the thing that is actually \
costing you games.\",
      \"evidence\": [\"benchmark.gold_per_min\"]
    }}
  ],
  \"plan\": [
    {{
      \"title\": \"Leave fights you have not set up\",
      \"action\": \"For your next few games, only commit to a fight when you \
know where the enemy support is. Walking away is the cheapest way to move the \
death rate the evidence shows.\",
      \"evidence\": [\"overall.deaths\"]
    }}
  ]
}}"
    )
}

/// What the model is shown, as JSON.
#[derive(Serialize, ToSchema)]
struct Payload<'a> {
    /// `player` or `match`, so the model knows whether it is reading a career
    /// or a single game.
    scope: AnalysisScope,
    task: &'static str,
    evidence: Vec<Item<'a>>,
}

#[derive(Serialize, ToSchema)]
struct Item<'a> {
    id: &'a str,
    label: &'a str,
    statement: &'a str,
    /// Carried through so the model can hedge honestly rather than guessing
    /// how solid a figure is.
    sample: i64,
    confidence: &'a crate::domain::benchmark::Confidence,
}

pub fn user(scope: AnalysisScope, evidence: &[Evidence]) -> String {
    let payload = Payload {
        scope,
        task: match scope {
            AnalysisScope::Player => {
                "Assess this player's overall performance and what they should work on next."
            }
            AnalysisScope::Role => {
                "Assess this player in the one role they chose to improve, using only the \
                 evidence supplied — every figure in it is already restricted to that role and \
                 to standard All Pick matchmaking. Say where they stand, what is holding them \
                 back in this role specifically, and what to work on next."
            }
            AnalysisScope::Match => {
                "Assess this single match against the player's own history. Say what happened, \
                 why it matters, and what to change next game."
            }
        },
        evidence: evidence
            .iter()
            .map(|e| Item {
                id: &e.id,
                label: &e.label,
                statement: &e.statement,
                sample: e.sample,
                confidence: &e.confidence,
            })
            .collect(),
    };

    // Serialization of our own types cannot fail; the fallback keeps the
    // signature infallible rather than propagating an impossible error.
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::benchmark::Confidence;
    use crate::domain::coaching::EvidenceKind;

    fn evidence() -> Vec<Evidence> {
        vec![Evidence {
            id: "overall.record".into(),
            kind: EvidenceKind::Overall,
            label: "Record".into(),
            statement: "Across 20 stored matches, you have won 11 and lost 9 (55%).".into(),
            sample: 20,
            confidence: Confidence::Adequate,
        }]
    }

    #[test]
    fn the_system_prompt_forbids_arithmetic_and_names_every_kind() {
        let system = system(5, 4);

        assert!(system.contains("must NOT calculate"));
        assert!(system.contains("At most 5 insights"));
        assert!(system.contains("4 plan steps"));
        for kind in InsightKind::ALL {
            assert!(system.contains(kind.slug()), "missing {}", kind.slug());
        }
    }

    /// The prompt has to say that numbers are checked, because the checker
    /// drops whole insights — a model that was never told would lose good work
    /// to a rule it had no way to follow.
    #[test]
    fn the_system_prompt_states_that_figures_are_verified_afterwards() {
        let system = system(5, 4);

        assert!(system.contains("checked after you answer"));
        assert!(system.contains("discarded"));
        // And tells it what to do instead of inventing a target.
        assert!(system.contains("qualitatively"));
    }

    #[test]
    fn the_system_prompt_explains_that_the_evidence_is_already_scoped() {
        let system = system(5, 4);

        // The architectural guarantee, stated as a fact rather than a request:
        // the model is not being asked to ignore anything, because nothing that
        // should be ignored is in the list.
        assert!(system.contains("already restricted"));
        assert!(system.contains("All Pick"));
    }

    #[test]
    fn the_system_prompt_asks_for_a_training_plan() {
        let system = system(5, 4);

        assert!(system.contains("\"plan\""));
        assert!(system.contains("\"action\""));
    }

    #[test]
    fn the_payload_carries_the_evidence_and_its_ids() {
        let user = user(AnalysisScope::Player, &evidence());

        assert!(user.contains("overall.record"));
        assert!(user.contains("you have won 11 and lost 9"));
        assert!(user.contains("\"scope\": \"player\""));
    }

    #[test]
    fn the_match_scope_asks_a_different_question() {
        let player = user(AnalysisScope::Player, &evidence());
        let match_ = user(AnalysisScope::Match, &evidence());

        assert!(match_.contains("single match"));
        assert_ne!(player, match_);
    }

    #[test]
    fn the_payload_is_valid_json_a_model_can_be_handed() {
        let rendered = user(AnalysisScope::Player, &evidence());
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(parsed["evidence"][0]["id"], "overall.record");
        assert_eq!(parsed["evidence"][0]["sample"], 20);
        assert_eq!(parsed["evidence"][0]["confidence"], "adequate");
    }

    #[test]
    fn no_field_carries_free_text_from_a_user() {
        // The payload is built only from ids, labels, statements and numbers
        // this backend composed. If a persona name ever leaks into evidence,
        // this is where it would show up.
        let rendered = user(AnalysisScope::Player, &evidence());
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        let item = &parsed["evidence"][0];

        // serde_json orders object keys alphabetically on the way back in, so
        // the set is what matters, not the order.
        let mut keys: Vec<&str> = item
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["confidence", "id", "label", "sample", "statement"]
        );
    }
}
