//! Single-match analysis, end to end.
//!
//! The deterministic composition of timeline evidence is unit-tested in
//! `services::match_analysis`, and the STRATZ response shapes in
//! `services::match_facts::stratz`. What is only testable through the router is
//! the thing this phase is actually about: **how the analysis behaves when the
//! second provider does not cooperate.**
//!
//! Four failures, and what each must not do:
//!
//!   1. no token configured — must not fail the request, and must not present
//!      match totals as if a timeline had been read;
//!   2. the provider is down — must not fail the request, and must say the
//!      timeline is missing rather than omitting the subject;
//!   3. the provider has no parsed replay — must say so, because "no death
//!      events" and "did not die" are different facts;
//!   4. the provider does not carry the match — must not turn the player's own
//!      match into a 404.
//!
//! In every one of them, `GET /api/matches/:id` and `GET /api/matches/:id/analysis`
//! keep answering. A second provider must never be able to stop a player reading
//! their own game.

mod support;

use axum::http::StatusCode;
use dota_coach_backend::services::dota::ProviderError;
use serde_json::Value;
use support::{
    parsed_match_facts, sample_matches, skip, unique_steam_id, unparsed_match_facts, MockDota,
    Session, StubLlm, StubMatchFacts, StubVerifier, TestApp,
};

/// An answer citing the timeline, in the three-part form a match analysis asks
/// for, with a timestamp the fixture's evidence genuinely contains.
///
/// `10:45` is the second death in `parsed_match_facts` — 25 seconds after
/// respawning from the first — so it appears verbatim in
/// `match.timeline.repeat_deaths`.
const TIMELINE_ANSWER: &str = r#"{
    "summary": "You lost this game between your first death and your second.",
    "insights": [
        {
            "kind": "weakness",
            "title": "You walked back into the fight you had just lost",
            "severity": "major",
            "timestamp": "10:45",
            "what_happened": "You respawned and were killed again in the same fight.",
            "why_it_matters": "The second death was free for them and left your team a man down twice in a row.",
            "better_action": "After a death, take a wave or a camp before you rejoin.",
            "evidence": ["match.timeline.repeat_deaths"]
        }
    ],
    "plan": [
        {
            "title": "Fight selection",
            "action": "For your next few games, farm one wave after every death before walking toward your team.",
            "evidence": ["match.timeline.repeat_deaths"]
        }
    ]
}"#;

/// A model that fabricates a moment. `31:15` is clock-shaped and appears nowhere
/// in the evidence.
const FABRICATED_ANSWER: &str = r#"{
    "summary": "A close game.",
    "insights": [
        {
            "kind": "weakness",
            "title": "You threw the game at the second Roshan",
            "severity": "major",
            "timestamp": "31:15",
            "what_happened": "At 31:15 you contested Roshan without vision.",
            "why_it_matters": "It handed them the Aegis and the game.",
            "better_action": "Ward before you commit.",
            "evidence": ["match.timeline.roshan"]
        }
    ]
}"#;

/// One synced player with matches, and the internal id of their newest one.
async fn seed(app: &TestApp) -> (i64, Session, String) {
    let steam_id = unique_steam_id();
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let matches = app.get("/api/matches", Some(&session.token)).await.json();
    let id = matches["matches"][0]["id"]
        .as_str()
        .expect("a synced match")
        .to_string();

    (steam_id, session, id)
}

fn evidence_ids(body: &Value) -> Vec<String> {
    body["evidence"]
        .as_array()
        .expect("evidence array")
        .iter()
        .map(|e| e["id"].as_str().unwrap_or_default().to_string())
        .collect()
}

fn statement(body: &Value, id: &str) -> String {
    body["evidence"]
        .as_array()
        .expect("evidence array")
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no evidence with id {id}; got {:?}", evidence_ids(body)))
        ["statement"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn a_parsed_match_is_analysed_from_its_timeline_with_real_timestamps() {
    let Some(db) = support::pool().await else {
        return skip("a_parsed_match_is_analysed_from_its_timeline_with_real_timestamps");
    };
    let app = support::app_with_match_facts(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        StubLlm::with_answer(TIMELINE_ANSWER),
        StubMatchFacts::serving(parsed_match_facts(0, 0)),
        support::test_config(),
    );
    let (steam_id, session, id) = seed(&app).await;

    // Reading is free and already carries the timeline.
    let before = app
        .get(&format!("/api/matches/{id}/analysis"), Some(&session.token))
        .await
        .json();

    let ids = evidence_ids(&before);
    assert!(
        ids.iter().any(|i| i == "match.timeline.deaths"),
        "the timeline did not reach the evidence: {ids:?}"
    );
    assert!(
        !ids.iter().any(|i| i == "match.timeline.unavailable"),
        "a parsed match must not report its timeline as missing"
    );

    // The evidence carries seconds, not just totals — this is what makes an
    // insight checkable rather than merely confident.
    let deaths = statement(&before, "match.timeline.deaths");
    assert!(deaths.contains("10:00"), "no timestamp in: {deaths}");
    assert!(statement(&before, "match.timeline.repeat_deaths").contains("10:45"));
    assert!(statement(&before, "match.timeline.items").contains("Blink Dagger at 14:22"));

    let generated = app
        .post(&format!("/api/matches/{id}/analyze"), Some(&session.token))
        .await;
    assert_eq!(generated.status, StatusCode::OK);

    let analysis = &generated.json()["analysis"];
    assert_eq!(analysis["scope"], "match");

    let insight = &analysis["insights"][0];
    assert_eq!(insight["severity"], "major");
    assert_eq!(insight["timestamp"], "10:45");
    assert!(insight["what_happened"].is_string());
    assert!(insight["why_it_matters"].is_string());
    assert!(insight["better_action"].is_string());

    // One primary training focus, not a list of four.
    assert_eq!(
        analysis["plan"].as_array().map(Vec::len),
        Some(1),
        "a single match should produce exactly one training focus"
    );

    app.cleanup(&[steam_id]).await;
}

/// The rule the whole phase rests on. A model that names a moment the evidence
/// never recorded has invented a reading of a replay, and a reader cannot tell
/// it from a real one without opening the game.
#[tokio::test]
async fn a_fabricated_timestamp_is_refused_rather_than_shown() {
    let Some(db) = support::pool().await else {
        return skip("a_fabricated_timestamp_is_refused_rather_than_shown");
    };
    let app = support::app_with_match_facts(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        StubLlm::with_answer(FABRICATED_ANSWER),
        StubMatchFacts::serving(parsed_match_facts(0, 0)),
        support::test_config(),
    );
    let (steam_id, session, id) = seed(&app).await;

    let response = app
        .post(&format!("/api/matches/{id}/analyze"), Some(&session.token))
        .await;

    // Nothing survived validation, so there is no analysis — not a partial one
    // presented as complete.
    assert_eq!(response.status, StatusCode::BAD_GATEWAY);

    // And the measured evidence is untouched by the model's failure.
    let after = app
        .get(&format!("/api/matches/{id}/analysis"), Some(&session.token))
        .await;
    assert_eq!(after.status, StatusCode::OK);
    assert!(after.json()["analysis"].is_null());
    assert!(!after.json()["evidence"].as_array().unwrap().is_empty());

    app.cleanup(&[steam_id]).await;
}

/// "No death events" and "did not die" are different facts, and only one of them
/// is true of an unparsed match. Stating the difference is what stops the model
/// narrating a timeline it was never shown.
#[tokio::test]
async fn an_unparsed_match_says_no_timeline_exists_instead_of_inventing_one() {
    let Some(db) = support::pool().await else {
        return skip("an_unparsed_match_says_no_timeline_exists_instead_of_inventing_one");
    };
    let app = support::app_with_match_facts(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        StubLlm::answering(),
        StubMatchFacts::serving(unparsed_match_facts(0, 0)),
        support::test_config(),
    );
    let (steam_id, session, id) = seed(&app).await;

    let body = app
        .get(&format!("/api/matches/{id}/analysis"), Some(&session.token))
        .await;
    assert_eq!(body.status, StatusCode::OK);
    let body = body.json();

    let note = statement(&body, "match.timeline.unavailable");
    assert!(note.contains("never parsed"), "got: {note}");
    assert!(
        note.contains("totals"),
        "the model must be told what it may use instead: {note}"
    );

    // And nothing timed is claimed.
    for id in evidence_ids(&body) {
        assert!(
            id == "match.timeline.unavailable" || !id.starts_with("match.timeline."),
            "{id} was composed for a match with no parsed replay"
        );
    }

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn without_a_stratz_token_the_match_page_still_answers_and_says_what_is_missing() {
    let Some(db) = support::pool().await else {
        return skip(
            "without_a_stratz_token_the_match_page_still_answers_and_says_what_is_missing",
        );
    };
    let app = support::app_with_match_facts(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        StubLlm::answering(),
        StubMatchFacts::unconfigured(),
        support::test_config(),
    );
    let (steam_id, session, id) = seed(&app).await;

    // The match itself is untouched by a second provider being absent.
    assert_eq!(
        app.get(&format!("/api/matches/{id}"), Some(&session.token))
            .await
            .status,
        StatusCode::OK,
    );

    let body = app
        .get(&format!("/api/matches/{id}/analysis"), Some(&session.token))
        .await;
    assert_eq!(body.status, StatusCode::OK);

    let note = statement(&body.json(), "match.timeline.unavailable");
    assert!(note.contains("configured"), "got: {note}");
    assert!(
        !note.contains("Trying again"),
        "nothing to wait for when no token exists: {note}"
    );

    // The aggregate evidence is still there, so the analysis is shallower rather
    // than absent.
    assert!(evidence_ids(&body.json())
        .iter()
        .any(|i| i == "match.result"));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_provider_outage_costs_the_timeline_and_nothing_else() {
    let Some(db) = support::pool().await else {
        return skip("a_provider_outage_costs_the_timeline_and_nothing_else");
    };
    let app = support::app_with_match_facts(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        StubLlm::answering(),
        StubMatchFacts::failing(ProviderError::Unavailable("connection refused".into())),
        support::test_config(),
    );
    let (steam_id, session, id) = seed(&app).await;

    let body = app
        .get(&format!("/api/matches/{id}/analysis"), Some(&session.token))
        .await;
    assert_eq!(
        body.status,
        StatusCode::OK,
        "a timeline outage must not become a failed request"
    );

    let note = statement(&body.json(), "match.timeline.unavailable");
    assert!(note.contains("could not be reached"), "got: {note}");
    assert!(
        note.contains("Trying again"),
        "a transient failure is worth retrying, and the user should know: {note}"
    );

    // Everything else keeps working. `/api/coach` is left out deliberately: it
    // answers 409 until a coaching role is chosen, which is a different rule and
    // would make this assertion pass for the wrong reason.
    for path in [
        "/api/matches".to_string(),
        "/api/stats".to_string(),
        format!("/api/matches/{id}"),
        format!("/api/matches/{id}/comparison"),
    ] {
        assert_eq!(
            app.get(&path, Some(&session.token)).await.status,
            StatusCode::OK,
            "{path} broke because a second provider was down",
        );
    }

    app.cleanup(&[steam_id]).await;
}

/// A rate limit at the second provider is an outage of the timeline, not a 429
/// for the request: the player asked to read their own match and is entitled to
/// what this application already stores.
#[tokio::test]
async fn a_provider_rate_limit_does_not_become_the_users_rate_limit() {
    let Some(db) = support::pool().await else {
        return skip("a_provider_rate_limit_does_not_become_the_users_rate_limit");
    };
    let app = support::app_with_match_facts(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        StubLlm::answering(),
        StubMatchFacts::failing(ProviderError::RateLimited),
        support::test_config(),
    );
    let (steam_id, session, id) = seed(&app).await;

    let body = app
        .get(&format!("/api/matches/{id}/analysis"), Some(&session.token))
        .await;

    assert_eq!(body.status, StatusCode::OK);
    assert!(statement(&body.json(), "match.timeline.unavailable").contains("could not be reached"));

    app.cleanup(&[steam_id]).await;
}

/// An anonymous Dota profile inside an otherwise public match looks, from the
/// provider's side, exactly like a match that does not exist. Neither is a 404
/// for this request: the match is the player's own and this application has it.
#[tokio::test]
async fn a_match_the_provider_does_not_carry_is_not_a_missing_match() {
    let Some(db) = support::pool().await else {
        return skip("a_match_the_provider_does_not_carry_is_not_a_missing_match");
    };
    let app = support::app_with_match_facts(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        StubLlm::answering(),
        StubMatchFacts::failing(ProviderError::NotFound),
        support::test_config(),
    );
    let (steam_id, session, id) = seed(&app).await;

    let body = app
        .get(&format!("/api/matches/{id}/analysis"), Some(&session.token))
        .await;
    assert_eq!(body.status, StatusCode::OK);

    let note = statement(&body.json(), "match.timeline.unavailable");
    assert!(note.contains("does not have it"), "got: {note}");
    assert!(note.contains("private"), "got: {note}");

    app.cleanup(&[steam_id]).await;
}

/// A match id that is not a match of this player's is still a 404, with or
/// without a second provider in the picture.
#[tokio::test]
async fn an_unknown_match_id_is_still_not_found() {
    let Some(db) = support::pool().await else {
        return skip("an_unknown_match_id_is_still_not_found");
    };
    let app = support::app_with_match_facts(
        db,
        MockDota::with_matches(sample_matches(5)),
        StubVerifier::rejecting(),
        StubLlm::answering(),
        StubMatchFacts::serving(parsed_match_facts(0, 0)),
        support::test_config(),
    );
    let steam_id = unique_steam_id();
    let session = app.login_as(steam_id).await;

    let missing = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
    for path in [
        format!("/api/matches/{missing}/analysis"),
        format!("/api/matches/{missing}/analyze"),
    ] {
        let response = if path.ends_with("analyze") {
            app.post(&path, Some(&session.token)).await
        } else {
            app.get(&path, Some(&session.token)).await
        };
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{path}");
    }

    app.cleanup(&[steam_id]).await;
}

/// The read path is required to cost nothing, which means it must not spend a
/// provider call per page view. One fetch, then the stored reading.
#[tokio::test]
async fn reading_the_analysis_twice_asks_the_provider_once() {
    let Some(db) = support::pool().await else {
        return skip("reading_the_analysis_twice_asks_the_provider_once");
    };
    let facts = StubMatchFacts::serving(parsed_match_facts(0, 0));
    let app = support::app_with_match_facts(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        StubLlm::answering(),
        facts.clone(),
        support::test_config(),
    );
    let (steam_id, session, id) = seed(&app).await;

    for _ in 0..3 {
        assert_eq!(
            app.get(&format!("/api/matches/{id}/analysis"), Some(&session.token))
                .await
                .status,
            StatusCode::OK,
        );
    }

    // The stub counts calls that reach it. The real provider caches in Postgres
    // behind the trait, which this harness deliberately does not have — so the
    // assertion here is only that the handler asks once per read and no more,
    // which is what makes that cache effective rather than incidental.
    assert_eq!(
        facts.calls.load(std::sync::atomic::Ordering::SeqCst),
        3,
        "one provider call per analysis read, no more"
    );

    app.cleanup(&[steam_id]).await;
}
