//! Session creation and the history API, end to end.
//!
//! The rules under test are the ones that would otherwise fail silently: a
//! session built from the wrong matches still renders as a page of numbers, and
//! a progress chart drawn over a polluted history looks exactly like one drawn
//! over a clean one.

mod support;

use support::{
    app_with, app_with_providers, batch, seed_app, skip, test_config, unique_steam_id, Lane,
    MockDota, StubBenchmarks, StubHeroMeta, StubLlm, StubVerifier, PUBLIC_ALL_PICK,
    RANKED_ALL_PICK, TURBO,
};

use dota_coach_backend::domain::r#match::NormalizedMatch;
use dota_coach_backend::domain::role::CoachableRole;
use dota_coach_backend::repositories::coaching_session;

/// Carry games, support games, and Turbo — one hero throughout, so only the
/// scope rules can separate them.
fn mixed_history(carry: i64, support: i64, turbo: i64) -> Vec<NormalizedMatch> {
    let mut all = support::batch(1_000, carry, RANKED_ALL_PICK, Lane::Carry, carry / 2);
    if support > 0 {
        all.extend(batch(
            2_000,
            support,
            RANKED_ALL_PICK,
            Lane::Support,
            support / 2,
        ));
    }
    if turbo > 0 {
        all.extend(batch(3_000, turbo, TURBO, Lane::Carry, turbo / 2));
    }

    for m in all.iter_mut() {
        m.hero_id = 35;
    }
    all
}

#[tokio::test]
async fn a_sync_records_a_session_for_the_coached_role() {
    let Some(db) = support::pool().await else {
        return skip("a_sync_records_a_session_for_the_coached_role");
    };

    let (app, session) = seed_app(db, mixed_history(20, 0, 0), 100).await;
    app.choose_role(&session, "carry").await;

    // The seeding sync ran before a role existed, so this is the first sync
    // that can checkpoint anything.
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let stored = coaching_session::latest(&app.db, session.dota_player_id, CoachableRole::Carry)
        .await
        .unwrap()
        .expect("a sync with a chosen role records a session");

    assert_eq!(stored.sequence, 1);
    assert_eq!(stored.analyzed_match_count, 20);
    // Measured, not written: no model was ever called.
    assert_eq!(stored.analysis_id, None);
    assert!(stored.performance.is_some());
    assert!(
        !stored.metrics.is_empty(),
        "a session exists to carry numbers"
    );

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_session_never_contains_turbo_or_another_role() {
    let Some(db) = support::pool().await else {
        return skip("a_session_never_contains_turbo_or_another_role");
    };

    // Twenty Carry, fifteen Support, twelve Turbo — all on one hero.
    let (app, session) = seed_app(db, mixed_history(20, 15, 12), 100).await;
    app.choose_role(&session, "carry").await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let stored = coaching_session::latest(&app.db, session.dota_player_id, CoachableRole::Carry)
        .await
        .unwrap()
        .expect("a session was recorded");

    // Only the Carry games. Not because anything filtered them afterwards —
    // the other matches were never fetched.
    assert_eq!(stored.analyzed_match_count, 20);

    let ids = stored.analyzed_match_ids.clone();
    let rows: Vec<(String, Option<i32>)> =
        sqlx::query_as("SELECT role, game_mode FROM matches WHERE id = ANY($1)")
            .bind(&ids)
            .fetch_all(&app.db)
            .await
            .unwrap();

    assert_eq!(rows.len(), 20);
    for (role, game_mode) in rows {
        assert_eq!(role, "Carry", "a Carry session read a {role} match");
        assert_ne!(game_mode, Some(23), "a Carry session read a Turbo match");
    }

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_second_sync_without_enough_new_games_records_nothing() {
    let Some(db) = support::pool().await else {
        return skip("a_second_sync_without_enough_new_games_records_nothing");
    };

    let (app, session) = seed_app(db, mixed_history(20, 0, 0), 100).await;
    app.choose_role(&session, "carry").await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    assert_eq!(
        coaching_session::count(&app.db, session.dota_player_id, CoachableRole::Carry)
            .await
            .unwrap(),
        1
    );

    // Same games again. A session here would be a duplicate snapshot, and
    // every comparison against it would report noise as change.
    app.post("/api/players/me/sync", Some(&session.token)).await;

    assert_eq!(
        coaching_session::count(&app.db, session.dota_player_id, CoachableRole::Carry)
            .await
            .unwrap(),
        1,
        "re-reading the same window is not a new session",
    );

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_sync_without_a_chosen_role_still_succeeds() {
    let Some(db) = support::pool().await else {
        return skip("a_sync_without_a_chosen_role_still_succeeds");
    };

    // No `choose_role`. Sessions are per role, so there is nothing to snapshot
    // — and that must not cost the player their sync.
    let (app, session) = seed_app(db, mixed_history(20, 0, 0), 100).await;

    let response = app.post("/api/players/me/sync", Some(&session.token)).await;
    assert_eq!(response.status, axum::http::StatusCode::OK);

    for role in CoachableRole::ALL {
        assert_eq!(
            coaching_session::count(&app.db, session.dota_player_id, role)
                .await
                .unwrap(),
            0
        );
    }

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_benchmark_outage_skips_the_session_without_failing_the_sync() {
    let Some(db) = support::pool().await else {
        return skip("a_benchmark_outage_skips_the_session_without_failing_the_sync");
    };

    let mut config = test_config();
    config.dota.sync_match_limit = 500;
    config.roles.analysis_match_limit = 100;

    let app = app_with(
        db,
        MockDota::with_matches(mixed_history(20, 0, 0)),
        StubVerifier::rejecting(),
        StubBenchmarks::unavailable(),
        config,
    );
    let steam_id = unique_steam_id();
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;
    app.choose_role(&session, "carry").await;

    let response = app.post("/api/players/me/sync", Some(&session.token)).await;

    // The matches are stored either way; a snapshot is what waits.
    assert_eq!(response.status, axum::http::StatusCode::OK);

    // A session permanently missing its peer half would poison every
    // comparison made against it, so none is written at all.
    assert_eq!(
        coaching_session::count(&app.db, session.dota_player_id, CoachableRole::Carry)
            .await
            .unwrap(),
        0,
        "a partial snapshot is worse than no snapshot",
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn history_reads_newest_first_and_paginates_like_the_match_list() {
    let Some(db) = support::pool().await else {
        return skip("history_reads_newest_first_and_paginates_like_the_match_list");
    };

    let (app, session) = seed_app(db, mixed_history(20, 0, 0), 100).await;
    app.choose_role(&session, "carry").await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app
        .get("/api/coach/sessions", Some(&session.token))
        .await
        .json();

    assert_eq!(body["role"], "carry");
    assert_eq!(body["total"], 1);
    assert_eq!(body["total_pages"], 1);
    assert_eq!(body["page"], 1);

    let first = &body["sessions"][0];
    assert_eq!(first["sequence"], 1);
    assert_eq!(first["analyzed_match_count"], 20);
    assert_eq!(first["has_analysis"], false);
    // A summary carries no snapshot documents.
    assert!(first["metrics"].is_null());

    // Rejected, never clamped — the rule `/api/matches` holds.
    for query in ["?page=0", "?limit=0", "?limit=101", "?role=jungle"] {
        let response = app
            .get(&format!("/api/coach/sessions{query}"), Some(&session.token))
            .await;
        assert_eq!(
            response.status,
            axum::http::StatusCode::BAD_REQUEST,
            "{query} should be rejected",
        );
    }

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn history_can_be_read_for_a_role_no_longer_being_coached() {
    let Some(db) = support::pool().await else {
        return skip("history_can_be_read_for_a_role_no_longer_being_coached");
    };

    let (app, session) = seed_app(db, mixed_history(20, 15, 0), 100).await;
    app.choose_role(&session, "carry").await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    // The player moves on to another role. Their Carry history is still theirs
    // to read, and reading it must not mean re-selecting Carry.
    app.choose_role(&session, "soft_support").await;

    let body = app
        .get("/api/coach/sessions?role=carry", Some(&session.token))
        .await
        .json();

    assert_eq!(body["role"], "carry");
    assert_eq!(body["total"], 1);

    // And the newly chosen role has a history of its own, which is empty.
    let body = app
        .get("/api/coach/sessions", Some(&session.token))
        .await
        .json();
    assert_eq!(body["role"], "soft_support");
    assert_eq!(body["total"], 0);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn one_session_is_served_verbatim_and_only_to_its_owner() {
    let Some(db) = support::pool().await else {
        return skip("one_session_is_served_verbatim_and_only_to_its_owner");
    };

    let bob_steam = unique_steam_id();
    let (app, alice) = seed_app(db, mixed_history(20, 0, 0), 100).await;
    app.choose_role(&alice, "carry").await;
    app.post("/api/players/me/sync", Some(&alice.token)).await;

    let list = app
        .get("/api/coach/sessions", Some(&alice.token))
        .await
        .json();
    let id = list["sessions"][0]["id"].as_str().unwrap().to_string();

    let body = app
        .get(&format!("/api/coach/sessions/{id}"), Some(&alice.token))
        .await
        .json();

    let stored = &body["session"];
    assert_eq!(stored["sequence"], 1);
    // The full snapshot, which the list deliberately omits.
    assert!(!stored["metrics"].as_array().unwrap().is_empty());
    assert_eq!(stored["analyzed_match_ids"].as_array().unwrap().len(), 20);

    // Bob knows the id and asks for it directly.
    let bob = app.login_as(bob_steam).await;
    let response = app
        .get(&format!("/api/coach/sessions/{id}"), Some(&bob.token))
        .await;

    // 404, not 403: whether the id exists is not Bob's business.
    assert_eq!(response.status, axum::http::StatusCode::NOT_FOUND);
    assert_eq!(response.error_code(), "NOT_FOUND");

    app.cleanup(&[bob_steam]).await;
}

#[tokio::test]
async fn generating_an_analysis_binds_it_to_the_session_it_read() {
    let Some(db) = support::pool().await else {
        return skip("generating_an_analysis_binds_it_to_the_session_it_read");
    };

    let mut config = test_config();
    config.dota.sync_match_limit = 500;
    config.roles.analysis_match_limit = 100;

    let app = app_with_providers(
        db,
        MockDota::with_matches(mixed_history(20, 0, 0)),
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

    let response = app.post("/api/coach/analyze", Some(&session.token)).await;
    assert_eq!(
        response.status,
        axum::http::StatusCode::OK,
        "{}",
        response.body
    );

    let stored = coaching_session::latest(&app.db, session.dota_player_id, CoachableRole::Carry)
        .await
        .unwrap()
        .expect("generating creates the session it binds to");

    // The model's reading is attached to the snapshot describing exactly the
    // games it was shown.
    let analysis_id = stored
        .analysis_id
        .expect("the analysis is bound to its session");

    // Re-running leaves the original binding alone: a session records the
    // first interpretation made of it.
    app.post("/api/coach/analyze", Some(&session.token)).await;

    let reread = coaching_session::latest(&app.db, session.dota_player_id, CoachableRole::Carry)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reread.id, stored.id);
    assert_eq!(reread.analysis_id, Some(analysis_id));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn history_without_a_chosen_role_says_so_rather_than_guessing() {
    let Some(db) = support::pool().await else {
        return skip("history_without_a_chosen_role_says_so_rather_than_guessing");
    };

    let (app, session) = seed_app(db, mixed_history(20, 0, 0), 100).await;

    let response = app.get("/api/coach/sessions", Some(&session.token)).await;

    // Picking a role for the player would mix evidence the product exists to
    // keep apart, so this is a precondition rather than a fallback.
    assert_eq!(response.status, axum::http::StatusCode::CONFLICT);

    // But an explicit role is readable without choosing one.
    let body = app
        .get("/api/coach/sessions?role=mid", Some(&session.token))
        .await
        .json();
    assert_eq!(body["role"], "mid");
    assert_eq!(body["total"], 0);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn public_all_pick_counts_towards_a_session() {
    let Some(db) = support::pool().await else {
        return skip("public_all_pick_counts_towards_a_session");
    };

    // Unranked public All Pick is part of the competitive population; only
    // Turbo and the unsupported modes are not.
    let mut history = batch(1_000, 10, RANKED_ALL_PICK, Lane::Carry, 5);
    history.extend(batch(4_000, 10, PUBLIC_ALL_PICK, Lane::Carry, 5));
    for m in history.iter_mut() {
        m.hero_id = 35;
    }

    let (app, session) = seed_app(db, history, 100).await;
    app.choose_role(&session, "carry").await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let stored = coaching_session::latest(&app.db, session.dota_player_id, CoachableRole::Carry)
        .await
        .unwrap()
        .expect("a session was recorded");
    assert_eq!(stored.analyzed_match_count, 20);

    app.cleanup(&[]).await;
}
