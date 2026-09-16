//! What the coach is allowed to see.
//!
//! Phase 4's claim is architectural rather than behavioural: a Support match
//! cannot reach a Carry analysis *because it was never fetched*, not because a
//! prompt asked a model to ignore it. These tests assert that from the outside,
//! against the evidence set the API actually returns — which is the same list
//! the model is shown.
//!
//! The evidence is deliberately a good target for this. Every statement in it
//! is a sentence the backend composed from its own numbers, so a leak is not a
//! subtle statistical drift: it is a number that could only have come from
//! matches the scope excludes.

mod support;

use serde_json::Value;
use support::{
    batch, seed_app, skip, test_config, Lane, PUBLIC_ALL_PICK, RANKED_ALL_PICK, TOURNAMENT, TURBO,
};

/// A history whose every role and mode is a different, identifiable size.
///
/// Distinct counts are what make a leak legible: if Carry coaching reports 30
/// matches it read exactly the Carry games, and any other number names the
/// population that leaked in.
fn mixed_history() -> Vec<dota_coach_backend::domain::r#match::NormalizedMatch> {
    let mut history = batch(1_000, 30, RANKED_ALL_PICK, Lane::Carry, 15);
    history.extend(batch(2_000, 25, RANKED_ALL_PICK, Lane::Support, 20));
    history.extend(batch(3_000, 20, PUBLIC_ALL_PICK, Lane::Offlane, 10));
    history.extend(batch(4_000, 15, RANKED_ALL_PICK, Lane::Mid, 5));
    // The two populations that must never appear in any coaching dataset.
    history.extend(batch(5_000, 10, TURBO, Lane::Carry, 10));
    history.extend(batch(6_000, 6, TOURNAMENT, Lane::Carry, 6));
    history
}

/// Every evidence statement, joined — what the model would read.
fn statements(body: &Value) -> String {
    body["evidence"]
        .as_array()
        .expect("evidence array")
        .iter()
        .map(|e| e["statement"].as_str().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

fn evidence_by_id<'a>(body: &'a Value, id: &str) -> Option<&'a Value> {
    body["evidence"].as_array()?.iter().find(|e| e["id"] == id)
}

#[tokio::test]
async fn coaching_evidence_is_restricted_to_the_chosen_role() {
    let Some(db) = support::pool().await else {
        return skip("coaching_evidence_is_restricted_to_the_chosen_role");
    };

    let (app, session) = seed_app(db, mixed_history(), 100).await;
    app.choose_role(&session, "carry").await;

    let body = app.get("/api/coach", Some(&session.token)).await.json();

    assert_eq!(body["role"], "carry");

    // The record is the load-bearing number: 30 Carry games, 15 of them won.
    // Any other total names what leaked — 55 would be Carry + Support, 40 would
    // be Carry + Turbo, 90 would be every role.
    let record = evidence_by_id(&body, "overall.record").expect("a record statement");
    assert_eq!(record["sample"], 30);
    let text = record["statement"].as_str().unwrap();
    assert!(
        text.contains("won 15 and lost 15"),
        "the Carry record is 15-15, not {text}",
    );

    // The scope is stated to the model in the same voice as the figures.
    let scope = evidence_by_id(&body, "scope.population").expect("a scope statement");
    let scope_text = scope["statement"].as_str().unwrap();
    assert!(scope_text.contains("Carry"), "{scope_text}");
    assert!(scope_text.contains("Turbo"), "{scope_text}");

    // Every sample in the set is bounded by the role's own match count. A
    // figure averaged over more matches than the role contains could only have
    // come from outside it.
    for item in body["evidence"].as_array().unwrap() {
        let sample = item["sample"].as_i64().unwrap_or(0);
        assert!(
            sample <= 30,
            "{} rests on {sample} matches, more than the 30 in scope",
            item["id"],
        );
    }

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn switching_role_switches_the_evidence_rather_than_adding_to_it() {
    let Some(db) = support::pool().await else {
        return skip("switching_role_switches_the_evidence_rather_than_adding_to_it");
    };

    let (app, session) = seed_app(db, mixed_history(), 100).await;

    app.choose_role(&session, "carry").await;
    let carry = app.get("/api/coach", Some(&session.token)).await.json();

    app.choose_role(&session, "soft_support").await;
    let support = app.get("/api/coach", Some(&session.token)).await.json();

    assert_eq!(carry["role"], "carry");
    assert_eq!(support["role"], "soft_support");

    assert_eq!(
        evidence_by_id(&carry, "overall.record").unwrap()["sample"],
        30
    );
    assert_eq!(
        evidence_by_id(&support, "overall.record").unwrap()["sample"],
        25,
    );

    // 20 of 25 support games were wins, and none of the carry ones can be in
    // there: a combined set would read 35 of 55.
    let support_record = evidence_by_id(&support, "overall.record").unwrap()["statement"]
        .as_str()
        .unwrap();
    assert!(
        support_record.contains("won 20 and lost 5"),
        "{support_record}",
    );

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn no_ineligible_mode_reaches_a_coaching_evidence_set() {
    let Some(db) = support::pool().await else {
        return skip("no_ineligible_mode_reaches_a_coaching_evidence_set");
    };

    // Turbo and tournament games are all wins and all on Carry, so including
    // them would inflate the Carry record in a way the numbers would show.
    let (app, session) = seed_app(db, mixed_history(), 100).await;
    app.choose_role(&session, "carry").await;

    let body = app.get("/api/coach", Some(&session.token)).await.json();
    let record = evidence_by_id(&body, "overall.record").unwrap();

    assert_eq!(
        record["sample"], 30,
        "10 Turbo and 6 tournament Carry wins must be invisible here",
    );
    assert!(!statements(&body).contains("46 stored"));

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn hero_evidence_is_restricted_to_heroes_played_in_the_chosen_role() {
    let Some(db) = support::pool().await else {
        return skip("hero_evidence_is_restricted_to_heroes_played_in_the_chosen_role");
    };

    // Two heroes, cleanly split by role: Luna is only ever a carry here, and
    // Crystal Maiden only ever a support.
    let mut history = batch(1_000, 20, RANKED_ALL_PICK, Lane::Carry, 12);
    for m in history.iter_mut() {
        m.hero_id = 35; // Luna
    }
    let mut supports = batch(2_000, 18, RANKED_ALL_PICK, Lane::Support, 9);
    for m in supports.iter_mut() {
        m.hero_id = 5; // Crystal Maiden
    }
    history.extend(supports);

    let (app, session) = seed_app(db, history, 100).await;
    app.choose_role(&session, "carry").await;

    let body = app.get("/api/coach", Some(&session.token)).await.json();
    let text = statements(&body);

    assert!(
        text.contains("Luna"),
        "the role's own hero is missing:\n{text}"
    );
    assert!(
        !text.contains("Crystal Maiden"),
        "a support hero reached carry coaching:\n{text}",
    );
    // Hero evidence carries the hero id, so the leak is checkable structurally
    // as well as in the prose.
    assert!(evidence_by_id(&body, "hero.35").is_some());
    assert!(evidence_by_id(&body, "hero.5").is_none());

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn patterns_are_detected_inside_the_role_not_across_the_career() {
    let Some(db) = support::pool().await else {
        return skip("patterns_are_detected_inside_the_role_not_across_the_career");
    };

    // Dying constantly as a support, cleanly as a carry. A career-wide detector
    // would dilute the support habit across 40 matches and might report it
    // against the carry games too.
    let mut history = batch(1_000, 20, RANKED_ALL_PICK, Lane::Carry, 10);
    for m in history.iter_mut() {
        m.deaths = 1;
    }
    let mut supports = batch(2_000, 20, RANKED_ALL_PICK, Lane::Support, 10);
    for m in supports.iter_mut() {
        m.deaths = 14;
    }
    history.extend(supports);

    let (app, session) = seed_app(db, history, 100).await;

    app.choose_role(&session, "carry").await;
    let carry = app.get("/api/coach", Some(&session.token)).await.json();

    app.choose_role(&session, "soft_support").await;
    let support = app.get("/api/coach", Some(&session.token)).await.json();

    let carry_patterns = carry["patterns"].as_array().unwrap();
    let support_patterns = support["patterns"].as_array().unwrap();

    assert!(
        carry_patterns.iter().all(|p| p["id"] != "high_death_rate"),
        "a support habit was reported against carry games: {carry_patterns:?}",
    );
    assert!(
        support_patterns
            .iter()
            .any(|p| p["id"] == "high_death_rate"),
        "the support habit went undetected in its own role: {support_patterns:?}",
    );

    // And the pattern's denominator is the role's matches, not the career's.
    let death_rate = support_patterns
        .iter()
        .find(|p| p["id"] == "high_death_rate")
        .unwrap();
    assert_eq!(death_rate["measured"], 20);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn the_training_focus_belongs_to_the_role_it_was_chosen_in() {
    let Some(db) = support::pool().await else {
        return skip("the_training_focus_belongs_to_the_role_it_was_chosen_in");
    };

    let mut history = batch(1_000, 20, RANKED_ALL_PICK, Lane::Carry, 10);
    for m in history.iter_mut() {
        m.deaths = 2;
    }
    let mut supports = batch(2_000, 20, RANKED_ALL_PICK, Lane::Support, 10);
    for m in supports.iter_mut() {
        m.deaths = 14;
    }
    history.extend(supports);

    let (app, session) = seed_app(db, history, 100).await;

    app.choose_role(&session, "soft_support").await;
    let support = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();
    let support_key = support["focus"]["key"].as_str().map(str::to_string);

    app.choose_role(&session, "carry").await;
    let carry = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();

    // Each role keeps its own goal. Switching roles must not retire the focus
    // the player will come back to.
    app.choose_role(&session, "soft_support").await;
    let support_again = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();

    assert_eq!(
        support_again["focus"]["key"].as_str().map(str::to_string),
        support_key,
        "the support goal was lost while working on carry",
    );
    assert_ne!(
        carry["focus"]["key"], support["focus"]["key"],
        "both roles were handed the same goal: {} vs {}",
        carry["focus"]["key"], support["focus"]["key"],
    );

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn coaching_refuses_to_pick_a_role_on_the_players_behalf() {
    let Some(db) = support::pool().await else {
        return skip("coaching_refuses_to_pick_a_role_on_the_players_behalf");
    };

    let (app, session) = seed_app(db, mixed_history(), 100).await;

    // A synced player who has not chosen. Every coaching read refuses, because
    // any default would either mix roles or override the player's decision.
    for path in ["/api/coach", "/api/coach/training-focus"] {
        let response = app.get(path, Some(&session.token)).await;
        assert_eq!(response.status, 409, "{path}: {}", response.body);
        assert!(response.body.contains("role"), "{path}: {}", response.body);
    }

    // And the role-selection screen itself keeps answering, because that is
    // where the refusal sends them.
    assert_eq!(
        app.get("/api/coach/roles", Some(&session.token))
            .await
            .status,
        200,
    );

    app.cleanup(&[]).await;
}

// ---------------------------------------------------------------------------
// What survives the model
// ---------------------------------------------------------------------------

/// An answer with one honest insight, one honest plan step, and two pieces of
/// fabrication that must not reach a player.
const MIXED_ANSWER: &str = r#"{
    "summary": "Your deaths are the thing holding this role back.",
    "insights": [
        {
            "kind": "weakness",
            "title": "You die too often",
            "explanation": "Each death hands over the map while you are down.",
            "evidence": ["overall.deaths"]
        },
        {
            "kind": "weakness",
            "title": "Your farm is far behind",
            "explanation": "You average 137 gold per minute, which is dire.",
            "evidence": ["overall.economy"]
        }
    ],
    "plan": [
        {
            "title": "Walk away from fights you have not set up",
            "action": "Only commit when you know where the enemy support is.",
            "evidence": ["overall.deaths"]
        },
        {
            "title": "Hit a timing",
            "action": "Finish your first big item before 18 minutes.",
            "evidence": ["overall.economy"]
        }
    ]
}"#;

#[tokio::test]
async fn the_model_gets_a_training_plan_through_and_its_fabrications_dropped() {
    let Some(db) = support::pool().await else {
        return skip("the_model_gets_a_training_plan_through_and_its_fabrications_dropped");
    };

    let mut config = test_config();
    config.dota.sync_match_limit = 500;

    let app = support::app_with_providers(
        db,
        support::MockDota::with_matches(mixed_history()),
        support::StubVerifier::rejecting(),
        support::StubBenchmarks::serving(),
        support::StubHeroMeta::serving(),
        support::StubLlm::with_answer(MIXED_ANSWER),
        config,
    );
    let session = app.login_as(support::unique_steam_id()).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;
    app.choose_role(&session, "carry").await;

    let body = app
        .post("/api/coach/analyze", Some(&session.token))
        .await
        .json();
    let analysis = &body["analysis"];

    assert_eq!(analysis["scope"], "role");
    assert_eq!(analysis["role"], "carry");

    // The insight whose figure came from nowhere is gone; the honest one stays.
    let insights = analysis["insights"].as_array().unwrap();
    assert_eq!(insights.len(), 1, "{insights:?}");
    assert_eq!(insights[0]["title"], "You die too often");

    // Same rule on the plan: the step that invented an 18-minute target is
    // dropped, and the surviving step is numbered one rather than two.
    let plan = analysis["plan"].as_array().unwrap();
    assert_eq!(plan.len(), 1, "{plan:?}");
    assert_eq!(plan[0]["position"], 1);
    assert_eq!(plan[0]["evidence"][0], "overall.deaths");

    // And the stored analysis reads back the same way, plan included.
    let reread = app.get("/api/coach", Some(&session.token)).await.json();
    assert_eq!(reread["analysis"]["plan"].as_array().unwrap().len(), 1);
    assert_eq!(reread["analysis"]["plan"][0]["title"], plan[0]["title"]);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn an_analysis_generated_for_one_role_is_not_served_for_another() {
    let Some(db) = support::pool().await else {
        return skip("an_analysis_generated_for_one_role_is_not_served_for_another");
    };

    let (app, session) = seed_app(db, mixed_history(), 100).await;

    app.choose_role(&session, "carry").await;
    let generated = app
        .post("/api/coach/analyze", Some(&session.token))
        .await
        .json();
    assert_eq!(generated["analysis"]["role"], "carry");

    // Switching role must not inherit the previous role's answer, however
    // similar the evidence behind it looks.
    app.choose_role(&session, "soft_support").await;
    let support = app.get("/api/coach", Some(&session.token)).await.json();

    assert!(
        support["analysis"].is_null(),
        "a carry analysis was served under a support heading: {}",
        support["analysis"],
    );

    app.cleanup(&[]).await;
}
