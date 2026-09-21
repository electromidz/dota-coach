//! The cross-cutting guarantees, and specifically the ones nothing else
//! covers.
//!
//! Most of the rules in this suite's remit are already held down where they
//! were built: role and mode isolation in `coaching_scope` and
//! `competitive_scope`, session immutability in `coaching_sessions`, cache
//! safety in `coaching_cache`, progress comparison in the progress engine's
//! own unit tests. Duplicating those here would make the suite slower without
//! making it stricter.
//!
//! What was genuinely untested, and is tested here:
//!
//!   * `stale` — the flag that tells a player their stored analysis predates
//!     their recent games. The mechanism existed and nothing asserted it.
//!   * `GET /api/coach` across two players. Stats, matches, the player model,
//!     sessions and the cache each had an isolation test; the coaching
//!     evidence endpoint itself did not.
//!   * The conversation's daily ceiling.
//!   * Everything degradable being degraded at once.

mod support;

use support::{
    app_with_providers, batch, skip, test_config, unique_steam_id, Lane, MockDota, StubBenchmarks,
    StubHeroMeta, StubLlm, StubVerifier, RANKED_ALL_PICK,
};

use dota_coach_backend::domain::r#match::NormalizedMatch;

fn carry_history(start: i64, count: i64) -> Vec<NormalizedMatch> {
    let mut all = batch(start, count, RANKED_ALL_PICK, Lane::Carry, count / 2);
    for m in all.iter_mut() {
        m.hero_id = 35;
    }
    all
}

#[tokio::test]
async fn a_stored_analysis_is_flagged_stale_once_new_matches_land() {
    let Some(db) = support::pool().await else {
        return skip("a_stored_analysis_is_flagged_stale_once_new_matches_land");
    };

    let dota = MockDota::with_matches(carry_history(1_000, 20));
    let mut config = test_config();
    config.dota.sync_match_limit = 500;
    config.roles.analysis_match_limit = 100;
    config.dota.sync_cooldown_seconds = 0;

    let app = app_with_providers(
        db,
        dota.clone(),
        StubVerifier::rejecting(),
        StubBenchmarks::serving(),
        StubHeroMeta::serving(),
        StubLlm::answering(),
        config,
    );
    let steam_id = unique_steam_id();
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;
    app.choose_role(&session, "carry").await;

    let generated = app.post("/api/coach/analyze", Some(&session.token)).await;
    assert_eq!(
        generated.status,
        axum::http::StatusCode::OK,
        "{}",
        generated.body
    );

    // Fresh analysis, fresh evidence: nothing to warn about.
    let body = app.get("/api/coach", Some(&session.token)).await.json();
    assert!(!body["analysis"].is_null());
    assert_eq!(body["stale"], false);

    // Ten more games. The stored analysis was written from figures that no
    // longer describe the player, and saying nothing would leave them reading
    // last week's advice as though it were current.
    let mut grown = carry_history(1_000, 20);
    grown.extend(carry_history(9_000, 10));
    dota.set_matches(grown);
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app.get("/api/coach", Some(&session.token)).await.json();
    assert!(
        !body["analysis"].is_null(),
        "the analysis is still shown — stale is a caveat, not a deletion",
    );
    assert_eq!(body["stale"], true);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn one_player_never_receives_another_players_coaching_evidence() {
    let Some(db) = support::pool().await else {
        return skip("one_player_never_receives_another_players_coaching_evidence");
    };

    let mut config = test_config();
    config.dota.sync_match_limit = 500;
    config.roles.analysis_match_limit = 100;

    let app = app_with_providers(
        db,
        MockDota::with_matches(carry_history(1_000, 20)),
        StubVerifier::rejecting(),
        StubBenchmarks::serving(),
        StubHeroMeta::serving(),
        StubLlm::answering(),
        config,
    );

    let alice_steam = unique_steam_id();
    let bob_steam = unique_steam_id();

    let alice = app.login_as(alice_steam).await;
    app.post("/api/players/me/sync", Some(&alice.token)).await;
    app.choose_role(&alice, "carry").await;
    app.post("/api/coach/analyze", Some(&alice.token)).await;

    let alice_view = app.get("/api/coach", Some(&alice.token)).await.json();
    assert!(!alice_view["evidence"].as_array().unwrap().is_empty());
    assert!(!alice_view["analysis"].is_null());

    // Bob has synced nothing and chosen nothing. He must reach the
    // precondition, not Alice's evidence — and certainly not her analysis,
    // which is prose about a player who is not him.
    let bob = app.login_as(bob_steam).await;
    let response = app.get("/api/coach", Some(&bob.token)).await;
    assert_eq!(response.status, axum::http::StatusCode::CONFLICT);

    // Even once Bob picks the same role, his coach is empty.
    let chosen = app
        .post_json("/api/coach/role", r#"{"role":"carry"}"#, Some(&bob.token))
        .await;
    assert_eq!(chosen.status, axum::http::StatusCode::OK);

    let bob_view = app.get("/api/coach", Some(&bob.token)).await.json();
    assert!(
        bob_view["analysis"].is_null(),
        "Alice's analysis must not surface for Bob",
    );
    assert_eq!(
        bob_view["evidence"].as_array().unwrap().len(),
        0,
        "Bob has no matches, so he has no evidence",
    );

    app.cleanup(&[alice_steam, bob_steam]).await;
}

#[tokio::test]
async fn the_conversation_has_a_daily_ceiling() {
    let Some(db) = support::pool().await else {
        return skip("the_conversation_has_a_daily_ceiling");
    };

    let mut config = test_config();
    config.dota.sync_match_limit = 500;
    config.roles.analysis_match_limit = 100;
    config.coach.chat_daily_limit = 2;

    let app = app_with_providers(
        db,
        MockDota::with_matches(carry_history(1_000, 20)),
        StubVerifier::rejecting(),
        StubBenchmarks::serving(),
        StubHeroMeta::serving(),
        // Qualitative, so nothing is refused for quoting a figure.
        StubLlm::with_answer("Take fewer fights you did not start."),
        config,
    );
    let steam_id = unique_steam_id();
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;
    app.choose_role(&session, "carry").await;

    for i in 0..2 {
        let response = app
            .post_json(
                "/api/coach/conversation",
                &format!(r#"{{"question":"Question {i}?"}}"#),
                Some(&session.token),
            )
            .await;
        assert_eq!(
            response.status,
            axum::http::StatusCode::OK,
            "{}",
            response.body
        );
    }

    // The ceiling is on *replies*, and two have now been given.
    let refused = app
        .post_json(
            "/api/coach/conversation",
            r#"{"question":"One more?"}"#,
            Some(&session.token),
        )
        .await;
    assert_eq!(refused.status, axum::http::StatusCode::TOO_MANY_REQUESTS);

    // Reading is free and still works — a spent budget must not hide the
    // conversation the player has already paid for.
    let transcript = app
        .get("/api/coach/conversation", Some(&session.token))
        .await
        .json();
    assert_eq!(transcript["messages"].as_array().unwrap().len(), 4);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn everything_degradable_down_at_once_still_answers() {
    let Some(db) = support::pool().await else {
        return skip("everything_degradable_down_at_once_still_answers");
    };

    let mut config = test_config();
    config.dota.sync_match_limit = 500;
    config.roles.analysis_match_limit = 100;

    // No peer distributions, no hero meta, no model, no coaching history, no
    // cache. The player's own matches are the only thing left.
    config.coach.cache_enabled = false;

    let app = app_with_providers(
        db,
        MockDota::with_matches(carry_history(1_000, 20)),
        StubVerifier::rejecting(),
        StubBenchmarks::unavailable(),
        StubHeroMeta::unavailable(),
        StubLlm::unconfigured(),
        config,
    );
    let steam_id = unique_steam_id();
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;
    app.choose_role(&session, "carry").await;

    // The coach still reads. Fewer pieces of evidence, and it says why.
    let coach = app.get("/api/coach", Some(&session.token)).await;
    assert_eq!(coach.status, axum::http::StatusCode::OK, "{}", coach.body);

    let body = coach.json();
    assert!(
        !body["evidence"].as_array().unwrap().is_empty(),
        "the player's own figures are local and survive every outage",
    );
    assert_eq!(body["llm_available"], false);
    assert!(!body["note"].as_str().unwrap().is_empty());

    // Progress answers with no history rather than failing.
    let progress = app.get("/api/coach/progress", Some(&session.token)).await;
    assert_eq!(progress.status, axum::http::StatusCode::OK);
    assert!(progress.json()["comparison"].is_null());

    // So does the history list.
    let history = app.get("/api/coach/sessions", Some(&session.token)).await;
    assert_eq!(history.status, axum::http::StatusCode::OK);
    assert_eq!(history.json()["total"], 0);

    // And the transcript.
    let conversation = app
        .get("/api/coach/conversation", Some(&session.token))
        .await;
    assert_eq!(conversation.status, axum::http::StatusCode::OK);
    assert_eq!(conversation.json()["llm_available"], false);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_player_with_no_matches_at_all_is_told_what_to_do() {
    let Some(db) = support::pool().await else {
        return skip("a_player_with_no_matches_at_all_is_told_what_to_do");
    };

    let mut config = test_config();
    config.roles.analysis_match_limit = 100;

    let app = app_with_providers(
        db,
        MockDota::with_matches(Vec::new()),
        StubVerifier::rejecting(),
        StubBenchmarks::serving(),
        StubHeroMeta::serving(),
        StubLlm::answering(),
        config,
    );
    let steam_id = unique_steam_id();
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    // Two different reasons to have no role, and they get different answers:
    // a player with nothing synced cannot meaningfully choose one, and
    // telling them to pick would be a dead end.
    let coach = app.get("/api/coach", Some(&session.token)).await;
    assert_eq!(coach.status, axum::http::StatusCode::CONFLICT);
    assert!(
        coach.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Sync"),
        "{}",
        coach.body
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn two_players_with_identical_histories_do_not_share_an_analysis() {
    let Some(db) = support::pool().await else {
        return skip("two_players_with_identical_histories_do_not_share_an_analysis");
    };

    let mut config = test_config();
    config.dota.sync_match_limit = 500;
    config.roles.analysis_match_limit = 100;

    let app = app_with_providers(
        db,
        MockDota::with_matches(carry_history(1_000, 20)),
        StubVerifier::rejecting(),
        StubBenchmarks::serving(),
        StubHeroMeta::serving(),
        StubLlm::answering(),
        config,
    );

    let alice_steam = unique_steam_id();
    let bob_steam = unique_steam_id();

    // The same mock history for both, so their evidence statements — and
    // therefore their `context_hash` — are identical. The stored analysis is
    // content-addressed, and the *only* thing keeping Alice's answer out of
    // Bob's response is that the lookup is scoped by player. A refactor that
    // dropped that scope would be invisible in every other test.
    let alice = app.login_as(alice_steam).await;
    app.post("/api/players/me/sync", Some(&alice.token)).await;
    app.choose_role(&alice, "carry").await;
    app.post("/api/coach/analyze", Some(&alice.token)).await;

    let bob = app.login_as(bob_steam).await;
    app.post("/api/players/me/sync", Some(&bob.token)).await;
    app.choose_role(&bob, "carry").await;

    let bob_view = app.get("/api/coach", Some(&bob.token)).await.json();
    assert!(
        bob_view["analysis"].is_null(),
        "a content-addressed cache must still be scoped by player",
    );

    // Bob's own evidence is present — he has the same games, so this is not
    // an empty response hiding the leak.
    assert!(!bob_view["evidence"].as_array().unwrap().is_empty());

    // And the rows are genuinely separate.
    let rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT dota_player_id) FROM coaching_analyses
          WHERE dota_player_id IN ($1, $2)",
    )
    .bind(alice.dota_player_id)
    .bind(bob.dota_player_id)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(rows, 1, "only Alice generated one");

    app.cleanup(&[alice_steam, bob_steam]).await;
}
