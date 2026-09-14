//! Phase 3 behaviour: authentication, identity scoping, synchronization.
//!
//! The Dota provider and Valve are stubbed; Postgres is real. Set
//! `DATABASE_URL` (or `TEST_DATABASE_URL`) to run these — without one they
//! print a skip notice rather than passing vacuously.

mod support;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use support::{
    app, app_with, app_with_config, app_with_llm, matches_with, sample_matches, skip, test_config,
    unique_steam_id, Failure, MockDota, StubBenchmarks, StubHeroMeta, StubLlm, StubVerifier,
};

// ---------------------------------------------------------------------------
// Authentication
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_anonymous_request_is_rejected_everywhere() {
    let Some(db) = support::pool().await else {
        return skip("an_anonymous_request_is_rejected_everywhere");
    };
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    for (method, path) in [
        ("GET", "/api/players/me"),
        ("POST", "/api/players/me/sync"),
        ("GET", "/api/matches"),
        ("GET", "/api/matches/00000000-0000-0000-0000-000000000000"),
        ("GET", "/api/auth/me"),
        ("GET", "/api/stats"),
        ("GET", "/api/benchmark"),
        ("GET", "/api/benchmark/gold_per_min"),
        ("GET", "/api/heroes"),
        ("GET", "/api/heroes/recommendations"),
        ("GET", "/api/hero-intelligence"),
        ("GET", "/api/coach"),
        ("GET", "/api/coach/player-model"),
        ("GET", "/api/coach/training-focus"),
        ("POST", "/api/coach/analyze"),
        (
            "GET",
            "/api/matches/00000000-0000-0000-0000-000000000000/analysis",
        ),
        (
            "POST",
            "/api/matches/00000000-0000-0000-0000-000000000000/analyze",
        ),
    ] {
        let response = if method == "GET" {
            app.get(path, None).await
        } else {
            app.post(path, None).await
        };

        assert_eq!(
            response.status,
            StatusCode::UNAUTHORIZED,
            "{method} {path} should require a session"
        );
        assert_eq!(response.error_code(), "UNAUTHENTICATED");
    }
}

#[tokio::test]
async fn a_forged_session_cookie_is_rejected() {
    let Some(db) = support::pool().await else {
        return skip("a_forged_session_cookie_is_rejected");
    };
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let response = app.get("/api/players/me", Some("not-a-real-token")).await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    assert_eq!(response.error_code(), "UNAUTHENTICATED");
}

#[tokio::test]
async fn an_expired_session_is_rejected() {
    let Some(db) = support::pool().await else {
        return skip("an_expired_session_is_rejected");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let token = app.expired_session_for(steam_id).await;
    let response = app.get("/api/players/me", Some(&token)).await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn the_steam_login_flow_creates_an_account_a_dota_link_and_a_session() {
    let Some(db) = support::pool().await else {
        return skip("the_steam_login_flow_creates_an_account_a_dota_link_and_a_session");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::default().into(),
        StubVerifier::accepting(steam_id),
    );

    // 1. Starting the flow issues the login nonce and redirects to Steam.
    let start = app.get("/api/auth/steam", None).await;
    assert_eq!(start.status, StatusCode::SEE_OTHER);
    assert!(start
        .location
        .as_deref()
        .unwrap()
        .starts_with("https://steamcommunity.example/openid/login?"));

    let nonce = start
        .cookie_value("dota_coach_login_state")
        .expect("login should set a state cookie");

    // 2. Steam sends the browser back with the nonce it was given.
    let callback = app
        .request(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/auth/steam/callback?state={nonce}&openid.mode=id_res"
                ))
                .header(header::COOKIE, format!("dota_coach_login_state={nonce}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    assert_eq!(callback.status, StatusCode::SEE_OTHER);
    assert_eq!(callback.location.as_deref(), Some("http://localhost:3000"));

    let session = callback
        .cookie_value("dota_coach_session")
        .expect("a completed login should set a session cookie");

    // 3. The session works, and the Dota account was linked automatically.
    let me = app.get("/api/players/me", Some(&session)).await;
    assert_eq!(me.status, StatusCode::OK);

    let body = me.json();
    assert_eq!(body["user"]["steam_id"], steam_id.to_string());
    assert_eq!(
        body["dota_player"]["dota_account_id"],
        steam_id - 76_561_197_960_265_728
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn the_session_cookie_is_http_only() {
    let Some(db) = support::pool().await else {
        return skip("the_session_cookie_is_http_only");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::default().into(),
        StubVerifier::accepting(steam_id),
    );

    let start = app.get("/api/auth/steam", None).await;
    let nonce = start.cookie_value("dota_coach_login_state").unwrap();

    let callback = app
        .request(
            Request::builder()
                .method("GET")
                .uri(format!("/api/auth/steam/callback?state={nonce}"))
                .header(header::COOKIE, format!("dota_coach_login_state={nonce}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    let raw = callback
        .cookies
        .iter()
        .find(|c| c.starts_with("dota_coach_session="))
        .expect("session cookie");

    assert!(raw.contains("HttpOnly"), "cookie must be HttpOnly: {raw}");
    assert!(
        raw.contains("SameSite=Lax"),
        "cookie must be SameSite: {raw}"
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_callback_without_the_matching_nonce_establishes_no_session() {
    let Some(db) = support::pool().await else {
        return skip("a_callback_without_the_matching_nonce_establishes_no_session");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::default().into(),
        StubVerifier::accepting(steam_id),
    );

    // A login the user never started: attacker-chosen state, no cookie.
    let response = app
        .get("/api/auth/steam/callback?state=attacker", None)
        .await;

    assert_eq!(response.status, StatusCode::SEE_OTHER);
    assert_eq!(
        response.location.as_deref(),
        Some("http://localhost:3000/?error=login_expired")
    );
    assert!(response.cookie_value("dota_coach_session").is_none());
}

#[tokio::test]
async fn a_mismatched_nonce_establishes_no_session() {
    let Some(db) = support::pool().await else {
        return skip("a_mismatched_nonce_establishes_no_session");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::default().into(),
        StubVerifier::accepting(steam_id),
    );

    let response = app
        .request(
            Request::builder()
                .method("GET")
                .uri("/api/auth/steam/callback?state=one")
                .header(header::COOKIE, "dota_coach_login_state=two")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    assert_eq!(
        response.location.as_deref(),
        Some("http://localhost:3000/?error=login_expired")
    );
    assert!(response.cookie_value("dota_coach_session").is_none());
}

#[tokio::test]
async fn an_assertion_steam_rejects_establishes_no_session() {
    let Some(db) = support::pool().await else {
        return skip("an_assertion_steam_rejects_establishes_no_session");
    };
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let start = app.get("/api/auth/steam", None).await;
    let nonce = start.cookie_value("dota_coach_login_state").unwrap();

    let response = app
        .request(
            Request::builder()
                .method("GET")
                .uri(format!("/api/auth/steam/callback?state={nonce}"))
                .header(header::COOKIE, format!("dota_coach_login_state={nonce}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    assert_eq!(
        response.location.as_deref(),
        Some("http://localhost:3000/?error=steam_rejected")
    );
    assert!(response.cookie_value("dota_coach_session").is_none());
}

#[tokio::test]
async fn steam_being_down_is_reported_without_leaking_detail() {
    let Some(db) = support::pool().await else {
        return skip("steam_being_down_is_reported_without_leaking_detail");
    };
    let app = app(db, MockDota::default().into(), StubVerifier::unavailable());

    let start = app.get("/api/auth/steam", None).await;
    let nonce = start.cookie_value("dota_coach_login_state").unwrap();

    let response = app
        .request(
            Request::builder()
                .method("GET")
                .uri(format!("/api/auth/steam/callback?state={nonce}"))
                .header(header::COOKIE, format!("dota_coach_login_state={nonce}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    let location = response.location.unwrap();
    assert_eq!(location, "http://localhost:3000/?error=steam_unavailable");
    assert!(!location.contains("stub offline"));
}

#[tokio::test]
async fn logging_out_destroys_the_session() {
    let Some(db) = support::pool().await else {
        return skip("logging_out_destroys_the_session");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    assert_eq!(
        app.get("/api/players/me", Some(&session.token))
            .await
            .status,
        StatusCode::OK
    );

    let logout = app.post("/api/auth/logout", Some(&session.token)).await;
    assert_eq!(logout.status, StatusCode::OK);

    // The same cookie must no longer work.
    assert_eq!(
        app.get("/api/players/me", Some(&session.token))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );

    app.cleanup(&[steam_id]).await;
}

// ---------------------------------------------------------------------------
// Profile
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_authenticated_user_sees_their_own_dota_profile() {
    let Some(db) = support::pool().await else {
        return skip("an_authenticated_user_sees_their_own_dota_profile");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let response = app.get("/api/players/me", Some(&session.token)).await;
    assert_eq!(response.status, StatusCode::OK);

    let body = response.json();
    // SteamID64 must survive as a string; it does not fit a JS number.
    assert_eq!(body["user"]["steam_id"], steam_id.to_string());
    assert_eq!(
        body["dota_player"]["dota_account_id"],
        steam_id - 76_561_197_960_265_728
    );
    assert_eq!(body["matches_stored"], 0);

    app.cleanup(&[steam_id]).await;
}

// ---------------------------------------------------------------------------
// Synchronization
// ---------------------------------------------------------------------------

#[tokio::test]
async fn syncing_twice_stores_each_match_once() {
    let Some(db) = support::pool().await else {
        return skip("syncing_twice_stores_each_match_once");
    };
    let steam_id = unique_steam_id();
    let dota = MockDota::with_matches(sample_matches(5));
    let app = app(db, dota.clone(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let first = app.post("/api/players/me/sync", Some(&session.token)).await;
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(first.json()["sync"]["new_matches"], 5);

    let second = app.post("/api/players/me/sync", Some(&session.token)).await;
    assert_eq!(second.status, StatusCode::OK);
    assert_eq!(second.json()["sync"]["new_matches"], 0);
    assert_eq!(second.json()["sync"]["duplicates_skipped"], 5);

    let stored = app.stored_match_ids(session.dota_player_id).await;
    assert_eq!(stored.len(), 5, "no duplicates: {stored:?}");

    // Already-stored matches are not re-fetched from the provider.
    assert_eq!(
        dota.detail_calls.load(std::sync::atomic::Ordering::SeqCst),
        5
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_provider_repeating_a_match_id_still_stores_it_once() {
    let Some(db) = support::pool().await else {
        return skip("a_provider_repeating_a_match_id_still_stores_it_once");
    };
    let steam_id = unique_steam_id();

    let mut matches = sample_matches(2);
    let duplicate = matches[0].clone();
    matches.push(duplicate);

    let dota = MockDota::with_matches(matches);
    let app = app(db, dota, StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let response = app.post("/api/players/me/sync", Some(&session.token)).await;
    assert_eq!(response.json()["sync"]["new_matches"], 2);

    let stored = app.stored_match_ids(session.dota_player_id).await;
    assert_eq!(stored.len(), 2);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_new_match_appearing_later_is_added_without_touching_the_rest() {
    let Some(db) = support::pool().await else {
        return skip("a_new_match_appearing_later_is_added_without_touching_the_rest");
    };
    let steam_id = unique_steam_id();
    let dota = MockDota::with_matches(sample_matches(3));
    let app = app(db, dota.clone(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    app.post("/api/players/me/sync", Some(&session.token)).await;
    dota.set_matches(sample_matches(4));

    let response = app.post("/api/players/me/sync", Some(&session.token)).await;
    assert_eq!(response.json()["sync"]["new_matches"], 1);
    assert_eq!(response.json()["sync"]["duplicates_skipped"], 3);
    assert_eq!(app.stored_match_ids(session.dota_player_id).await.len(), 4);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn the_provider_being_down_fails_the_sync_without_leaking_detail() {
    let Some(db) = support::pool().await else {
        return skip("the_provider_being_down_fails_the_sync_without_leaking_detail");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::failing(Failure::Unavailable),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;

    let response = app.post("/api/players/me/sync", Some(&session.token)).await;

    assert_eq!(response.status, StatusCode::BAD_GATEWAY);
    assert_eq!(response.error_code(), "UPSTREAM_UNAVAILABLE");
    assert!(!response.body.contains("connection refused"));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn provider_rate_limiting_surfaces_as_429() {
    let Some(db) = support::pool().await else {
        return skip("provider_rate_limiting_surfaces_as_429");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::failing(Failure::RateLimited),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;

    let response = app.post("/api/players/me/sync", Some(&session.token)).await;

    assert_eq!(response.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.error_code(), "RATE_LIMITED");

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn an_unreadable_provider_response_is_an_upstream_error_not_a_crash() {
    let Some(db) = support::pool().await else {
        return skip("an_unreadable_provider_response_is_an_upstream_error_not_a_crash");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::failing(Failure::InvalidResponse),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;

    let response = app.post("/api/players/me/sync", Some(&session.token)).await;

    assert_eq!(response.status, StatusCode::BAD_GATEWAY);
    assert!(!response.body.contains("unexpected shape"));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_private_or_missing_match_detail_still_stores_the_summary() {
    let Some(db) = support::pool().await else {
        return skip("a_private_or_missing_match_detail_still_stores_the_summary");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_failing_details(sample_matches(3), Failure::NotFound),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;

    let response = app.post("/api/players/me/sync", Some(&session.token)).await;
    assert_eq!(response.status, StatusCode::OK);

    let sync = &response.json()["sync"];
    assert_eq!(sync["new_matches"], 3);
    assert_eq!(sync["details_enriched"], 0);
    assert_eq!(sync["details_failed"], 3);

    assert_eq!(app.stored_match_ids(session.dota_player_id).await.len(), 3);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_sync_never_pulls_more_than_the_configured_limit() {
    let Some(db) = support::pool().await else {
        return skip("a_sync_never_pulls_more_than_the_configured_limit");
    };
    let steam_id = unique_steam_id();

    let mut config = test_config();
    config.dota.sync_match_limit = 5;

    let app = app_with_config(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        config,
    );
    let session = app.login_as(steam_id).await;

    let response = app.post("/api/players/me/sync", Some(&session.token)).await;

    assert_eq!(response.json()["sync"]["matches_seen"], 5);
    assert_eq!(app.stored_match_ids(session.dota_player_id).await.len(), 5);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_second_sync_inside_the_cooldown_is_throttled() {
    let Some(db) = support::pool().await else {
        return skip("a_second_sync_inside_the_cooldown_is_throttled");
    };
    let steam_id = unique_steam_id();

    let mut config = test_config();
    config.dota.sync_cooldown_seconds = 300;

    let app = app_with_config(
        db,
        MockDota::with_matches(sample_matches(2)),
        StubVerifier::rejecting(),
        config,
    );
    let session = app.login_as(steam_id).await;

    assert_eq!(
        app.post("/api/players/me/sync", Some(&session.token))
            .await
            .status,
        StatusCode::OK
    );

    let second = app.post("/api/players/me/sync", Some(&session.token)).await;
    assert_eq!(second.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(second.error_code(), "RATE_LIMITED");

    app.cleanup(&[steam_id]).await;
}

// ---------------------------------------------------------------------------
// Deterministic analytics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn syncing_computes_metrics_for_every_stored_match() {
    let Some(db) = support::pool().await else {
        return skip("syncing_computes_metrics_for_every_stored_match");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(sample_matches(6)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;

    let sync = app.post("/api/players/me/sync", Some(&session.token)).await;
    assert_eq!(sync.json()["sync"]["metrics_computed"], 6);

    let stats = app.get("/api/stats", Some(&session.token)).await;
    assert_eq!(stats.status, StatusCode::OK);
    assert_eq!(stats.json()["overall"]["matches"], 6);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn metrics_are_not_recomputed_once_they_are_current() {
    let Some(db) = support::pool().await else {
        return skip("metrics_are_not_recomputed_once_they_are_current");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(sample_matches(4)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;

    app.post("/api/players/me/sync", Some(&session.token)).await;

    // Second pass: nothing new, and nothing stale to recompute.
    let again = app.post("/api/players/me/sync", Some(&session.token)).await;
    assert_eq!(again.json()["sync"]["new_matches"], 0);
    assert_eq!(again.json()["sync"]["metrics_computed"], 0);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn changing_a_matchs_facts_invalidates_its_metrics() {
    let Some(db) = support::pool().await else {
        return skip("changing_a_matchs_facts_invalidates_its_metrics");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(sample_matches(3)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    // Simulate a fact backfill touching the underlying row. A version check
    // alone would call these metrics current; they are not.
    sqlx::query("UPDATE matches SET team_kills = 40 WHERE dota_player_id = $1")
        .bind(session.dota_player_id)
        .execute(&app.db)
        .await
        .unwrap();

    let resync = app.post("/api/players/me/sync", Some(&session.token)).await;
    assert_eq!(
        resync.json()["sync"]["metrics_computed"],
        3,
        "metrics must be recomputed when their inputs change"
    );

    // And the newly available input now produces a value.
    let stats = app.get("/api/stats", Some(&session.token)).await.json();
    assert_eq!(stats["overall"]["kill_participation_sample"], 3);
    assert!(stats["overall"]["avg_kill_participation"].as_f64().unwrap() > 0.0);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn stats_aggregate_wins_heroes_and_roles() {
    let Some(db) = support::pool().await else {
        return skip("stats_aggregate_wins_heroes_and_roles");
    };
    let steam_id = unique_steam_id();
    // `sample_matches` alternates the result on the match id's parity.
    let app = app(
        db,
        MockDota::with_matches(sample_matches(10)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app.get("/api/stats", Some(&session.token)).await.json();
    let overall = &body["overall"];

    assert_eq!(overall["matches"], 10);
    assert_eq!(
        overall["wins"].as_i64().unwrap() + overall["losses"].as_i64().unwrap(),
        10
    );
    // KDA 8/4/12 -> (8+12)/4 = 5 on every sample match.
    assert!((overall["avg_kda"].as_f64().unwrap() - 5.0).abs() < 0.01);

    // Every sample match is the same hero and role, so both roll up to one row.
    assert_eq!(body["heroes"].as_array().unwrap().len(), 1);
    assert_eq!(body["heroes"][0]["matches"], 10);
    assert_eq!(body["roles"].as_array().unwrap().len(), 1);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn stats_report_an_empty_history_without_dividing_by_zero() {
    let Some(db) = support::pool().await else {
        return skip("stats_report_an_empty_history_without_dividing_by_zero");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let body = app.get("/api/stats", Some(&session.token)).await.json();

    assert_eq!(body["overall"]["matches"], 0);
    // No matches means no win rate — not a 0% one.
    assert!(body["overall"]["win_rate"].is_null());
    assert!(body["overall"]["avg_kda"].is_null());
    assert!(body["heroes"].as_array().unwrap().is_empty());

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn unparsed_matches_report_no_time_sliced_metrics() {
    let Some(db) = support::pool().await else {
        return skip("unparsed_matches_report_no_time_sliced_metrics");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(sample_matches(3)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app.get("/api/stats", Some(&session.token)).await.json();

    // The fixtures carry no parsed replay, so nothing pretends to have @10 data.
    assert_eq!(body["overall"]["parsed_matches"], 0);
    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn one_user_cannot_read_another_users_stats() {
    let Some(db) = support::pool().await else {
        return skip("one_user_cannot_read_another_users_stats");
    };
    let alice_steam = unique_steam_id();
    let bob_steam = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(sample_matches(5)),
        StubVerifier::rejecting(),
    );

    let alice = app.login_as(alice_steam).await;
    let bob = app.login_as(bob_steam).await;
    app.post("/api/players/me/sync", Some(&alice.token)).await;

    // Stats are scoped by session, so Bob sees his own empty history.
    let bob_stats = app.get("/api/stats", Some(&bob.token)).await;
    assert_eq!(bob_stats.json()["overall"]["matches"], 0);

    let alice_stats = app.get("/api/stats", Some(&alice.token)).await;
    assert_eq!(alice_stats.json()["overall"]["matches"], 5);

    app.cleanup(&[alice_steam, bob_steam]).await;
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn benchmarks_place_the_player_in_the_peer_distribution() {
    let Some(db) = support::pool().await else {
        return skip("benchmarks_place_the_player_in_the_peer_distribution");
    };
    let steam_id = unique_steam_id();
    // 20 matches on one hero clears the sample floor comfortably.
    let app = app(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app.get("/api/benchmark", Some(&session.token)).await.json();

    assert_eq!(body["sample"], 20);
    // Hero only: the provider cannot segment by rank, and must not imply it.
    assert_eq!(body["segmented_by"][0], "hero");

    let gpm = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["metric"] == "gold_per_min")
        .expect("gold_per_min result");

    assert_eq!(gpm["peer_median"], 500.0);
    assert_eq!(gpm["top_20_value"], 800.0);
    assert_eq!(gpm["confidence"], "adequate");
    assert!(gpm["percentile"].as_f64().unwrap() > 0.0);
    // Sample matches sit at 550 gpm, below the 800 top-20 line.
    assert!(gpm["gap_to_top_20"].as_f64().unwrap() > 0.0);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_thin_sample_is_shown_without_a_percentile() {
    let Some(db) = support::pool().await else {
        return skip("a_thin_sample_is_shown_without_a_percentile");
    };
    let steam_id = unique_steam_id();
    // Two matches: below the floor for any percentile claim.
    let app = app(
        db,
        MockDota::with_matches(sample_matches(2)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app.get("/api/benchmark", Some(&session.token)).await.json();
    let gpm = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["metric"] == "gold_per_min")
        .unwrap();

    assert_eq!(gpm["confidence"], "insufficient");
    assert!(
        gpm["percentile"].is_null(),
        "must not rank a two-game average"
    );
    // The player's own value and the reference are still shown.
    assert!(gpm["player_value"].as_f64().unwrap() > 0.0);
    assert_eq!(gpm["peer_median"], 500.0);
    assert!(gpm["note"].as_str().unwrap().contains("Not enough matches"));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_lower_is_better_metric_ranks_in_the_right_direction() {
    let Some(db) = support::pool().await else {
        return skip("a_lower_is_better_metric_ranks_in_the_right_direction");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app.get("/api/benchmark", Some(&session.token)).await.json();
    let deaths = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["metric"] == "deaths_per_min")
        .expect("deaths_per_min result");

    assert_eq!(deaths["higher_is_better"], false);
    // Sample matches die 4 times in 40 minutes = 0.1/min, better than the
    // stub's 0.15 median, so this must rank *above* the 50th percentile.
    assert!(
        deaths["percentile"].as_f64().unwrap() > 50.0,
        "fewer deaths than the median must rank better, got {}",
        deaths["percentile"]
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_provider_outage_still_shows_the_players_own_numbers() {
    let Some(db) = support::pool().await else {
        return skip("a_provider_outage_still_shows_the_players_own_numbers");
    };
    let steam_id = unique_steam_id();
    let app = app_with(
        db,
        MockDota::with_matches(sample_matches(10)),
        StubVerifier::rejecting(),
        StubBenchmarks::unavailable(),
        test_config(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let response = app.get("/api/benchmark", Some(&session.token)).await;

    // Degrades rather than failing: the local figures are still worth showing.
    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    assert!(body["note"].as_str().unwrap().contains("unavailable"));
    assert!(!body["results"].as_array().unwrap().is_empty());
    assert!(body["results"][0]["percentile"].is_null());
    // And no leak of the provider's own error text.
    assert!(!response.body.contains("offline"));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_single_metric_can_be_requested() {
    let Some(db) = support::pool().await else {
        return skip("a_single_metric_can_be_requested");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app
        .get("/api/benchmark/gold_per_min", Some(&session.token))
        .await
        .json();

    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["metric"], "gold_per_min");

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn an_unknown_metric_is_rejected() {
    let Some(db) = support::pool().await else {
        return skip("an_unknown_metric_is_rejected");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let response = app
        .get("/api/benchmark/not_a_metric", Some(&session.token))
        .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.error_code(), "BAD_REQUEST");

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn benchmarks_with_no_matches_explain_rather_than_error() {
    let Some(db) = support::pool().await else {
        return skip("benchmarks_with_no_matches_explain_rather_than_error");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let response = app.get("/api/benchmark", Some(&session.token)).await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["sample"], 0);
    assert!(response.json()["note"].as_str().unwrap().contains("Sync"));

    app.cleanup(&[steam_id]).await;
}

// ---------------------------------------------------------------------------
// Hero Intelligence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_hero_pool_is_built_from_the_players_own_history() {
    let Some(db) = support::pool().await else {
        return skip("the_hero_pool_is_built_from_the_players_own_history");
    };
    let steam_id = unique_steam_id();
    // 20 matches on Luna, alternating wins.
    let app = app(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app.get("/api/heroes", Some(&session.token)).await.json();

    let luna = &body["pool"][0];
    assert_eq!(luna["hero_id"], 35);
    assert_eq!(luna["matches"], 20);
    assert_eq!(luna["wins"], 10);
    assert_eq!(luna["losses"], 10);
    // Recent form is the last ten on *this* hero, not the last ten overall.
    assert_eq!(luna["recent_matches"], 10);
    // Even results against an even baseline: comfort, not signature.
    assert_eq!(luna["tier"], "comfort");
    assert_eq!(luna["confidence"], "adequate");

    assert_eq!(body["summary"]["heroes"], 1);
    assert_eq!(body["summary"]["comfort"], 1);
    assert_eq!(body["summary"]["established"], 1);
    assert_eq!(body["recent_window"], 10);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn the_hero_pool_answers_with_every_provider_down() {
    let Some(db) = support::pool().await else {
        return skip("the_hero_pool_answers_with_every_provider_down");
    };
    let steam_id = unique_steam_id();
    let app = support::app_with_providers(
        db,
        MockDota::with_matches(sample_matches(10)),
        StubVerifier::rejecting(),
        StubBenchmarks::unavailable(),
        StubHeroMeta::unavailable(),
        StubLlm::answering(),
        test_config(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let response = app.get("/api/heroes", Some(&session.token)).await;

    // The player's own repertoire is local data; no outage can withhold it.
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["pool"][0]["matches"], 10);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_played_hero_outranks_a_stronger_meta_hero_the_player_has_never_touched() {
    let Some(db) = support::pool().await else {
        return skip("a_played_hero_outranks_a_stronger_meta_hero_the_player_has_never_touched");
    };
    let steam_id = unique_steam_id();
    // Luna: 20 games, meta strength 80. Puck: never played, meta strength 95.
    let app = app(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app
        .get("/api/heroes/recommendations", Some(&session.token))
        .await
        .json();

    let recommendations = body["recommendations"].as_array().unwrap();
    let position = |hero_id: i64| {
        recommendations
            .iter()
            .position(|r| r["hero_id"] == hero_id)
            .unwrap_or_else(|| panic!("hero {hero_id} missing from recommendations"))
    };

    assert!(
        position(35) < position(13),
        "the hero with real history must outrank the stronger meta stranger"
    );

    let puck = &recommendations[position(13)];
    assert_ne!(
        puck["level"], "recommended",
        "a hero with no games on it is never a full recommendation"
    );
    assert_eq!(puck["matches"], 0);

    // And the score explains itself rather than arriving as a bare number.
    let luna = &recommendations[position(35)];
    assert!(!luna["parts"].as_array().unwrap().is_empty());
    assert!(luna["parts"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["detail"].as_str().is_some_and(|d| !d.is_empty())));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn hero_intelligence_reports_the_segmentation_it_actually_used() {
    let Some(db) = support::pool().await else {
        return skip("hero_intelligence_reports_the_segmentation_it_actually_used");
    };
    let steam_id = unique_steam_id();
    // The stubbed provider reports rank_tier 55 — Legend.
    let app = app(
        db,
        MockDota::with_matches(sample_matches(12)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app
        .get("/api/hero-intelligence", Some(&session.token))
        .await
        .json();

    assert_eq!(body["meta"]["available"], true);
    assert_eq!(body["meta"]["bracket"], "legend");
    assert_eq!(body["meta"]["bracket_label"], "Legend");
    assert_eq!(body["meta"]["segmented_by"][0], "rank_bracket");
    assert_eq!(body["meta"]["source"], "StubMeta");

    // The meta section is its own thing, ordered by strength.
    let leaders = body["meta_leaders"].as_array().unwrap();
    assert_eq!(leaders[0]["hero_id"], 13, "Puck is the strongest stub hero");

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_meta_outage_degrades_the_recommendations_rather_than_failing() {
    let Some(db) = support::pool().await else {
        return skip("a_meta_outage_degrades_the_recommendations_rather_than_failing");
    };
    let steam_id = unique_steam_id();
    let app = support::app_with_providers(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        StubBenchmarks::serving(),
        StubHeroMeta::unavailable(),
        StubLlm::answering(),
        test_config(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let response = app
        .get("/api/heroes/recommendations", Some(&session.token))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();

    assert_eq!(body["meta"]["available"], false);
    assert!(body["meta"]["note"]
        .as_str()
        .unwrap()
        .contains("unavailable"));
    // Scored on the player's own history alone, and honest about it.
    let luna = &body["recommendations"][0];
    assert_eq!(luna["hero_id"], 35);
    assert!(luna["meta_strength"].is_null());
    assert!(luna["caveats"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c.as_str().unwrap().contains("Meta data")));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn recommendations_honour_a_limit() {
    let Some(db) = support::pool().await else {
        return skip("recommendations_honour_a_limit");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(sample_matches(6)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app
        .get("/api/heroes/recommendations?limit=2", Some(&session.token))
        .await
        .json();

    assert_eq!(body["recommendations"].as_array().unwrap().len(), 2);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn hero_intelligence_with_no_matches_explains_rather_than_erroring() {
    let Some(db) = support::pool().await else {
        return skip("hero_intelligence_with_no_matches_explains_rather_than_erroring");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let response = app
        .get("/api/hero-intelligence", Some(&session.token))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    assert!(body["pool"].as_array().unwrap().is_empty());
    assert!(body["note"].as_str().unwrap().contains("Sync"));
    // Meta heroes are still scored, and still held back for lack of history.
    assert!(!body["recommendations"].as_array().unwrap().is_empty());
    assert!(body["recommendations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["level"] != "recommended"));

    app.cleanup(&[steam_id]).await;
}

// ---------------------------------------------------------------------------
// AI coaching
// ---------------------------------------------------------------------------

#[tokio::test]
async fn reading_the_coach_shows_measured_evidence_without_calling_the_model() {
    let Some(db) = support::pool().await else {
        return skip("reading_the_coach_shows_measured_evidence_without_calling_the_model");
    };
    let steam_id = unique_steam_id();
    let llm = StubLlm::answering();
    let app = app_with_llm(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        llm.clone(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app.get("/api/coach", Some(&session.token)).await.json();

    assert!(!body["evidence"].as_array().unwrap().is_empty());
    assert!(body["analysis"].is_null(), "reading never generates");
    assert_eq!(body["llm_available"], true);
    assert_eq!(
        llm.calls.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "opening the coach must not spend a model call"
    );

    // Every statement is a sentence the backend composed from its own numbers.
    let record = body["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "overall.record")
        .expect("career record evidence");
    assert!(record["statement"]
        .as_str()
        .unwrap()
        .contains("20 stored matches"));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn an_analysis_may_only_cite_evidence_that_exists() {
    let Some(db) = support::pool().await else {
        return skip("an_analysis_may_only_cite_evidence_that_exists");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app
        .post("/api/coach/analyze", Some(&session.token))
        .await
        .json();

    let ids: Vec<&str> = body["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();

    let insights = body["analysis"]["insights"].as_array().unwrap();
    assert!(!insights.is_empty());

    for insight in insights {
        let refs = insight["evidence"].as_array().unwrap();
        assert!(
            !refs.is_empty(),
            "an insight with no evidence is not stored"
        );
        for reference in refs {
            assert!(
                ids.contains(&reference.as_str().unwrap()),
                "insight cited {reference}, which is not in the evidence"
            );
        }
        assert_eq!(insight["kind"], "weakness");
        assert_eq!(insight["kind_label"], "Weakness");
    }

    assert_eq!(body["analysis"]["model"], "stub-model");
    assert_eq!(body["cached"], false);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn an_answer_citing_invented_evidence_is_rejected_rather_than_shown() {
    let Some(db) = support::pool().await else {
        return skip("an_answer_citing_invented_evidence_is_rejected_rather_than_shown");
    };
    let steam_id = unique_steam_id();
    // A plausible-sounding insight about a statistic this backend never
    // measured — exactly the failure mode the validation exists for.
    let inventing = StubLlm::with_answer(
        r#"{"summary": "Ward more.", "insights": [{
            "kind": "weakness",
            "title": "You place too few wards",
            "explanation": "You average 2.1 observer wards per game, well below your bracket.",
            "evidence": ["benchmark.wards_placed"]
        }]}"#,
    );
    let app = app_with_llm(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        inventing,
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let response = app.post("/api/coach/analyze", Some(&session.token)).await;

    assert_eq!(response.status, StatusCode::BAD_GATEWAY);
    assert_eq!(response.error_code(), "UPSTREAM_UNAVAILABLE");
    // And nothing was stored, so the next read is still clean.
    let body = app.get("/api/coach", Some(&session.token)).await.json();
    assert!(body["analysis"].is_null());

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn asking_the_same_question_twice_costs_one_model_call() {
    let Some(db) = support::pool().await else {
        return skip("asking_the_same_question_twice_costs_one_model_call");
    };
    let steam_id = unique_steam_id();
    let llm = StubLlm::answering();
    let app = app_with_llm(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        llm.clone(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let first = app
        .post("/api/coach/analyze", Some(&session.token))
        .await
        .json();
    let second = app
        .post("/api/coach/analyze", Some(&session.token))
        .await
        .json();

    assert_eq!(first["cached"], false);
    assert_eq!(second["cached"], true);
    assert_eq!(
        llm.calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "identical evidence must be answered from storage"
    );
    assert_eq!(first["analysis"]["id"], second["analysis"]["id"]);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_deployment_without_a_model_still_serves_the_evidence() {
    let Some(db) = support::pool().await else {
        return skip("a_deployment_without_a_model_still_serves_the_evidence");
    };
    let steam_id = unique_steam_id();
    let app = app_with_llm(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        StubLlm::unconfigured(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let read = app.get("/api/coach", Some(&session.token)).await;
    assert_eq!(read.status, StatusCode::OK);
    let body = read.json();
    assert_eq!(body["llm_available"], false);
    assert!(!body["evidence"].as_array().unwrap().is_empty());
    assert!(body["note"].as_str().unwrap().contains("not configured"));

    // Asking for a generation says so plainly rather than pretending.
    let generate = app.post("/api/coach/analyze", Some(&session.token)).await;
    assert_eq!(generate.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(generate.error_code(), "FEATURE_UNAVAILABLE");

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_model_outage_fails_the_generation_and_nothing_else() {
    let Some(db) = support::pool().await else {
        return skip("a_model_outage_fails_the_generation_and_nothing_else");
    };
    let steam_id = unique_steam_id();
    let app = app_with_llm(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        StubLlm::unavailable(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let generate = app.post("/api/coach/analyze", Some(&session.token)).await;
    assert_eq!(generate.status, StatusCode::BAD_GATEWAY);
    // The provider's own message never reaches the client.
    assert!(!generate.json()["error"]["message"]
        .as_str()
        .unwrap()
        .contains("offline"));

    assert_eq!(
        app.get("/api/coach", Some(&session.token)).await.status,
        StatusCode::OK,
    );
    assert_eq!(
        app.get("/api/stats", Some(&session.token)).await.status,
        StatusCode::OK,
        "an LLM outage must not touch the deterministic API"
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_match_analysis_reads_the_game_against_the_players_own_averages() {
    let Some(db) = support::pool().await else {
        return skip("a_match_analysis_reads_the_game_against_the_players_own_averages");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let matches = app.get("/api/matches", Some(&session.token)).await.json();
    let id = matches["matches"][0]["id"].as_str().unwrap().to_string();

    // Reading first: no analysis yet, but the evidence is already there.
    let before = app
        .get(&format!("/api/matches/{id}/analysis"), Some(&session.token))
        .await
        .json();
    assert!(before["analysis"].is_null());

    let kda = before["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "match.kda")
        .expect("match KDA evidence");
    assert!(
        kda["statement"].as_str().unwrap().contains("your average"),
        "a single match is only coachable against the player's own baseline: {}",
        kda["statement"]
    );

    let generated = app
        .post(&format!("/api/matches/{id}/analyze"), Some(&session.token))
        .await;
    assert_eq!(generated.status, StatusCode::OK);
    assert_eq!(generated.json()["analysis"]["scope"], "match");
    assert_eq!(generated.json()["analysis"]["match_id"], id.as_str());

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn one_user_cannot_analyse_another_users_match() {
    let Some(db) = support::pool().await else {
        return skip("one_user_cannot_analyse_another_users_match");
    };
    let (owner, intruder) = (unique_steam_id(), unique_steam_id());
    let app = app(
        db,
        MockDota::with_matches(sample_matches(5)),
        StubVerifier::rejecting(),
    );

    let owner_session = app.login_as(owner).await;
    app.post("/api/players/me/sync", Some(&owner_session.token))
        .await;
    let matches = app
        .get("/api/matches", Some(&owner_session.token))
        .await
        .json();
    let id = matches["matches"][0]["id"].as_str().unwrap().to_string();

    let intruder_session = app.login_as(intruder).await;
    let response = app
        .post(
            &format!("/api/matches/{id}/analyze"),
            Some(&intruder_session.token),
        )
        .await;

    // 404, not 403: whether an id exists is not theirs to learn.
    assert_eq!(response.status, StatusCode::NOT_FOUND);

    app.cleanup(&[owner, intruder]).await;
}

#[tokio::test]
async fn analysing_an_empty_history_is_a_precondition_error() {
    let Some(db) = support::pool().await else {
        return skip("analysing_an_empty_history_is_a_precondition_error");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let response = app.post("/api/coach/analyze", Some(&session.token)).await;

    assert_eq!(response.status, StatusCode::CONFLICT);
    assert_eq!(response.error_code(), "PRECONDITION_UNMET");

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn the_cooldown_caps_how_often_a_player_can_spend_a_model_call() {
    let Some(db) = support::pool().await else {
        return skip("the_cooldown_caps_how_often_a_player_can_spend_a_model_call");
    };
    let steam_id = unique_steam_id();
    let llm = StubLlm::answering();

    let mut config = test_config();
    config.coach.cooldown_seconds = 60;

    let app = support::app_with_providers(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
        StubBenchmarks::serving(),
        StubHeroMeta::serving(),
        llm.clone(),
        config,
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    assert_eq!(
        app.post("/api/coach/analyze", Some(&session.token))
            .await
            .status,
        StatusCode::OK,
    );

    // A different question — a match rather than the career — so this is not
    // served from the cache and genuinely wants a second model call.
    let matches = app.get("/api/matches", Some(&session.token)).await.json();
    let id = matches["matches"][0]["id"].as_str().unwrap().to_string();

    let second = app
        .post(&format!("/api/matches/{id}/analyze"), Some(&session.token))
        .await;

    assert_eq!(second.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(second.error_code(), "RATE_LIMITED");
    assert_eq!(
        llm.calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the limiter must stop the call, not merely report it"
    );

    app.cleanup(&[steam_id]).await;
}

// ---------------------------------------------------------------------------
// Player model and recurring patterns
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_habit_across_a_real_history_becomes_a_recurring_pattern() {
    let Some(db) = support::pool().await else {
        return skip("a_habit_across_a_real_history_becomes_a_recurring_pattern");
    };
    let steam_id = unique_steam_id();
    // 12 matches at 20 deaths in 40 minutes: 5 per 10 minutes, every game.
    let app = app(
        db,
        MockDota::with_matches(matches_with(12, 1, 20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app
        .get("/api/coach/player-model", Some(&session.token))
        .await
        .json();

    let pattern = body["model"]["patterns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "high_death_rate")
        .expect("the death-rate pattern");

    assert_eq!(pattern["occurrences"], 12);
    assert_eq!(pattern["measured"], 12);
    assert_eq!(pattern["status"], "active");
    // Both denominators are in the sentence, not just the verdict.
    assert!(pattern["statement"]
        .as_str()
        .unwrap()
        .contains("12 of the 12 matches"));
    assert!(!pattern["examples"].as_array().unwrap().is_empty());
    assert!(pattern["first_detected_at"].is_string());

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn no_pattern_is_claimed_from_a_thin_history() {
    let Some(db) = support::pool().await else {
        return skip("no_pattern_is_claimed_from_a_thin_history");
    };
    let steam_id = unique_steam_id();
    // Four terrible matches. A real tendency, and nowhere near evidence.
    let app = app(
        db,
        MockDota::with_matches(matches_with(4, 1, 20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app
        .get("/api/coach/player-model", Some(&session.token))
        .await
        .json();

    assert!(body["model"]["patterns"].as_array().unwrap().is_empty());
    assert_eq!(body["thresholds"]["min_measured"], 8);
    // And the silence is explained rather than read as a clean bill of health.
    let unmeasurable = body["unmeasurable"].as_array().unwrap();
    assert!(unmeasurable
        .iter()
        .any(|d| d["id"] == "high_death_rate" && d["measured"] == 4));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_detector_with_no_data_stays_silent_and_says_why() {
    let Some(db) = support::pool().await else {
        return skip("a_detector_with_no_data_stays_silent_and_says_why");
    };
    let steam_id = unique_steam_id();
    // Unparsed replays throughout, so the laning detector can never speak.
    let app = app(
        db,
        MockDota::with_matches(sample_matches(20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app
        .get("/api/coach/player-model", Some(&session.token))
        .await
        .json();

    let laning = body["unmeasurable"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == "low_cs_at_10")
        .expect("the laning detector reports its own silence");

    assert_eq!(laning["measured"], 0);
    assert!(!body["model"]["patterns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == "low_cs_at_10"));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_pattern_the_player_stops_repeating_is_resolved_not_forgotten() {
    let Some(db) = support::pool().await else {
        return skip("a_pattern_the_player_stops_repeating_is_resolved_not_forgotten");
    };
    let steam_id = unique_steam_id();
    let dota = MockDota::with_matches(matches_with(12, 1, 20));
    let app = app(db, dota.clone(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let before = app
        .get("/api/coach/player-model", Some(&session.token))
        .await
        .json();
    assert!(before["model"]["patterns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == "high_death_rate"));

    // Thirty clean matches later, the rate falls under the floor.
    dota.set_matches(matches_with(30, 100, 2));
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let after = app
        .get("/api/coach/player-model", Some(&session.token))
        .await
        .json();

    assert!(
        !after["model"]["patterns"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "high_death_rate"),
        "it no longer clears the threshold"
    );

    // But it is not erased: the fact that it was fixed is invisible in the
    // data that fixed it, which is why the model is persisted.
    let resolved = after["model"]["resolved_patterns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "high_death_rate")
        .expect("the resolved pattern is remembered");

    assert_eq!(resolved["status"], "resolved");
    assert!(resolved["statement"]
        .as_str()
        .unwrap()
        .contains("no longer meets the threshold"));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn recurring_patterns_reach_the_coach_as_citable_evidence() {
    let Some(db) = support::pool().await else {
        return skip("recurring_patterns_reach_the_coach_as_citable_evidence");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(matches_with(12, 1, 20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app.get("/api/coach", Some(&session.token)).await.json();

    let evidence = body["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "pattern.high_death_rate")
        .expect("the pattern is evidence the model may cite");

    assert_eq!(evidence["kind"], "pattern");
    // The sample is what the pattern could be checked in, not the career.
    assert_eq!(evidence["sample"], 12);
    assert!(!body["patterns"].as_array().unwrap().is_empty());

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn the_model_says_how_well_it_knows_the_player() {
    let Some(db) = support::pool().await else {
        return skip("the_model_says_how_well_it_knows_the_player");
    };
    let (new_player, veteran) = (unique_steam_id(), unique_steam_id());
    let dota = MockDota::with_matches(sample_matches(5));
    let app = app(db, dota.clone(), StubVerifier::rejecting());

    let session = app.login_as(new_player).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;
    let sparse = app
        .get("/api/coach/player-model", Some(&session.token))
        .await
        .json();

    assert_eq!(sparse["model"]["confidence"], "sparse");
    assert_eq!(sparse["model"]["matches_analyzed"], 5);
    assert!(sparse["model"]["confidence_caveat"]
        .as_str()
        .unwrap()
        .contains("first impression"));

    dota.set_matches(sample_matches(40));
    let session = app.login_as(veteran).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;
    let established = app
        .get("/api/coach/player-model", Some(&session.token))
        .await
        .json();

    assert_eq!(established["model"]["confidence"], "established");
    assert_eq!(established["model"]["matches_analyzed"], 40);
    // Role affinity and recent form are part of knowing someone.
    assert_eq!(established["model"]["preferred_roles"][0]["role"], "Carry");
    assert_eq!(established["model"]["recent_form"]["matches"], 10);

    app.cleanup(&[new_player, veteran]).await;
}

#[tokio::test]
async fn one_user_never_sees_another_users_model() {
    let Some(db) = support::pool().await else {
        return skip("one_user_never_sees_another_users_model");
    };
    let (owner, other) = (unique_steam_id(), unique_steam_id());
    let dota = MockDota::with_matches(matches_with(12, 1, 20));
    let app = app(db, dota.clone(), StubVerifier::rejecting());

    let owner_session = app.login_as(owner).await;
    app.post("/api/players/me/sync", Some(&owner_session.token))
        .await;

    // The second account syncs nothing at all.
    dota.set_matches(Vec::new());
    let other_session = app.login_as(other).await;
    app.post("/api/players/me/sync", Some(&other_session.token))
        .await;

    let body = app
        .get("/api/coach/player-model", Some(&other_session.token))
        .await
        .json();

    assert_eq!(body["model"]["matches_analyzed"], 0);
    assert!(body["model"]["patterns"].as_array().unwrap().is_empty());

    app.cleanup(&[owner, other]).await;
}

// ---------------------------------------------------------------------------
// Training focus and progress
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_player_gets_exactly_one_training_focus_with_a_checkable_goal() {
    let Some(db) = support::pool().await else {
        return skip("the_player_gets_exactly_one_training_focus_with_a_checkable_goal");
    };
    let steam_id = unique_steam_id();
    // A death-heavy history: a pattern, and a benchmark gap, both present.
    let app = app(
        db,
        MockDota::with_matches(matches_with(20, 1, 20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();

    let focus = &body["focus"];
    assert!(focus.is_object(), "one focus, not a list of weaknesses");
    assert_eq!(focus["status"], "active");

    // A goal is only a goal if it can be checked.
    assert!(focus["baseline_value"].is_number());
    assert!(focus["target_value"].is_number());
    assert!(focus["measure"].is_string());
    assert!(focus["higher_is_better"].is_boolean());

    // And the choice explains itself with every input the spec names.
    let parts: Vec<&str> = focus["score_parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["key"].as_str().unwrap())
        .collect();
    assert_eq!(
        parts,
        vec![
            "gap",
            "pattern",
            "recent",
            "impact",
            "confidence",
            "recency"
        ]
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn the_focus_is_stable_across_requests() {
    let Some(db) = support::pool().await else {
        return skip("the_focus_is_stable_across_requests");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(matches_with(20, 1, 20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let first = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();
    let second = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();

    // A focus that changed on every request would be a feed, not a plan.
    assert_eq!(first["focus"]["id"], second["focus"]["id"]);
    assert_eq!(first["focus"]["started_at"], second["focus"]["started_at"]);
    // The baseline is the promise made when it was set, so it does not drift.
    assert_eq!(
        first["focus"]["baseline_value"],
        second["focus"]["baseline_value"]
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn progress_is_measured_against_where_the_player_started() {
    let Some(db) = support::pool().await else {
        return skip("progress_is_measured_against_where_the_player_started");
    };
    let steam_id = unique_steam_id();
    let dota = MockDota::with_matches(matches_with(20, 1, 20));
    let app = app(db, dota.clone(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let before = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();
    let baseline = before["focus"]["baseline_value"].as_f64().unwrap();
    let focus_id = before["focus"]["id"].clone();

    // Ten clean matches later.
    dota.set_matches(matches_with(10, 100, 1));
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let after = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();

    // The series plots the measure over time, oldest bucket first.
    let points = after["progress"]["points"].as_array().unwrap();
    assert!(points.len() >= 2, "history is bucketed into a trend");
    assert!(
        points[0]["value"].as_f64().unwrap() > points[points.len() - 1]["value"].as_f64().unwrap()
    );
    assert_eq!(after["progress"]["window"], 10);

    // Either the same focus is now showing progress, or it was met and closed.
    if after["focus"]["id"] == focus_id {
        assert_eq!(after["focus"]["baseline_value"].as_f64().unwrap(), baseline);
        assert!(after["focus"]["progress"].as_f64().unwrap() > 0.0);
    } else {
        let history = after["history"].as_array().unwrap();
        assert!(history.iter().any(|f| f["id"] == focus_id));
    }

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_finished_focus_is_replaced_and_kept_as_history() {
    let Some(db) = support::pool().await else {
        return skip("a_finished_focus_is_replaced_and_kept_as_history");
    };
    let steam_id = unique_steam_id();
    let dota = MockDota::with_matches(matches_with(20, 1, 20));
    let app = app(db, dota.clone(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let first = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();
    let first_key = first["focus"]["key"].as_str().unwrap().to_string();

    // Twenty clean matches: the death rate over the recent window collapses.
    dota.set_matches(matches_with(20, 100, 1));
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let after = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();

    let history = after["history"].as_array().unwrap();
    let previous = history
        .iter()
        .find(|f| f["key"] == first_key.as_str())
        .expect("the finished focus is kept");

    assert_ne!(previous["status"], "active");
    assert!(previous["ended_at"].is_string());
    // And whatever is active now is not the one just finished.
    if after["focus"].is_object() {
        assert_ne!(after["focus"]["key"], first_key.as_str());
    }

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_player_with_nothing_to_fix_is_told_so_rather_than_given_busywork() {
    let Some(db) = support::pool().await else {
        return skip("a_player_with_nothing_to_fix_is_told_so_rather_than_given_busywork");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let body = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();

    assert!(body["focus"].is_null());
    assert!(body["note"].as_str().unwrap().contains("Sync"));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn the_current_focus_reaches_the_coach_as_evidence() {
    let Some(db) = support::pool().await else {
        return skip("the_current_focus_reaches_the_coach_as_evidence");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(matches_with(20, 1, 20)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    // Reading the coach before a focus exists must not create one.
    let before = app.get("/api/coach", Some(&session.token)).await.json();
    assert!(
        !before["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["id"] == "focus.current"),
        "reading the coach must not commit the player to a goal"
    );

    // Selecting one is what the training-focus endpoint is for.
    app.get("/api/coach/training-focus", Some(&session.token))
        .await;

    let after = app.get("/api/coach", Some(&session.token)).await.json();
    let evidence = after["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "focus.current")
        .expect("the focus is evidence the model may cite");

    assert_eq!(evidence["kind"], "focus");
    assert!(evidence["statement"].as_str().unwrap().contains("target"));

    app.cleanup(&[steam_id]).await;
}

// ---------------------------------------------------------------------------
// Match history, pagination and ownership
// ---------------------------------------------------------------------------

#[tokio::test]
async fn match_history_is_paginated_newest_first() {
    let Some(db) = support::pool().await else {
        return skip("match_history_is_paginated_newest_first");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(sample_matches(25)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let first = app
        .get("/api/matches?page=1&limit=10", Some(&session.token))
        .await;
    assert_eq!(first.status, StatusCode::OK);

    let body = first.json();
    assert_eq!(body["matches"].as_array().unwrap().len(), 10);
    assert_eq!(body["total"], 25);
    assert_eq!(body["page"], 1);
    assert_eq!(body["limit"], 10);
    assert_eq!(body["total_pages"], 3);

    // Newest first: the last sample match has the latest start time.
    assert_eq!(body["matches"][0]["match_id"], 9_000_000_024i64);

    // The final page holds the remainder.
    let last = app
        .get("/api/matches?page=3&limit=10", Some(&session.token))
        .await;
    assert_eq!(last.json()["matches"].as_array().unwrap().len(), 5);

    // Past the end is empty, not an error.
    let beyond = app
        .get("/api/matches?page=9&limit=10", Some(&session.token))
        .await;
    assert_eq!(beyond.status, StatusCode::OK);
    assert!(beyond.json()["matches"].as_array().unwrap().is_empty());

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn invalid_pagination_is_rejected() {
    let Some(db) = support::pool().await else {
        return skip("invalid_pagination_is_rejected");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    for query in [
        "?page=0",
        "?page=-1",
        "?limit=0",
        "?limit=101",
        "?limit=-5",
        "?page=abc",
        "?limit=abc",
    ] {
        let response = app
            .get(&format!("/api/matches{query}"), Some(&session.token))
            .await;

        assert_eq!(
            response.status,
            StatusCode::BAD_REQUEST,
            "{query} should be rejected"
        );
        assert_eq!(response.error_code(), "BAD_REQUEST");
    }

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn an_empty_history_is_an_empty_page_not_an_error() {
    let Some(db) = support::pool().await else {
        return skip("an_empty_history_is_an_empty_page_not_an_error");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let response = app.get("/api/matches", Some(&session.token)).await;

    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    assert!(body["matches"].as_array().unwrap().is_empty());
    assert_eq!(body["total"], 0);
    assert_eq!(body["total_pages"], 0);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_user_can_open_their_own_match() {
    let Some(db) = support::pool().await else {
        return skip("a_user_can_open_their_own_match");
    };
    let steam_id = unique_steam_id();
    let app = app(
        db,
        MockDota::with_matches(sample_matches(2)),
        StubVerifier::rejecting(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let list = app.get("/api/matches", Some(&session.token)).await;
    let id = list.json()["matches"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let response = app
        .get(&format!("/api/matches/{id}"), Some(&session.token))
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["match"]["id"], id);
    assert_eq!(response.json()["match"]["hero_name"], "Luna");

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn one_user_cannot_read_another_users_matches() {
    let Some(db) = support::pool().await else {
        return skip("one_user_cannot_read_another_users_matches");
    };
    let alice_steam = unique_steam_id();
    let bob_steam = unique_steam_id();

    let app = app(
        db,
        MockDota::with_matches(sample_matches(3)),
        StubVerifier::rejecting(),
    );

    let alice = app.login_as(alice_steam).await;
    let bob = app.login_as(bob_steam).await;

    app.post("/api/players/me/sync", Some(&alice.token)).await;

    let alice_matches = app.get("/api/matches", Some(&alice.token)).await;
    let alice_match_id = alice_matches.json()["matches"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Bob knows the id and asks for it directly.
    let response = app
        .get(&format!("/api/matches/{alice_match_id}"), Some(&bob.token))
        .await;

    // 404, not 403: whether the id exists is not Bob's business.
    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(response.error_code(), "NOT_FOUND");

    // And Bob's own history is untouched by Alice's sync.
    let bob_list = app.get("/api/matches", Some(&bob.token)).await;
    assert_eq!(bob_list.json()["total"], 0);

    app.cleanup(&[alice_steam, bob_steam]).await;
}

#[tokio::test]
async fn each_user_syncs_into_their_own_history() {
    let Some(db) = support::pool().await else {
        return skip("each_user_syncs_into_their_own_history");
    };
    let alice_steam = unique_steam_id();
    let bob_steam = unique_steam_id();

    let app = app(
        db,
        MockDota::with_matches(sample_matches(4)),
        StubVerifier::rejecting(),
    );

    let alice = app.login_as(alice_steam).await;
    let bob = app.login_as(bob_steam).await;

    app.post("/api/players/me/sync", Some(&alice.token)).await;
    app.post("/api/players/me/sync", Some(&bob.token)).await;

    // The same provider match ids land under both players without colliding:
    // uniqueness is per player, not global.
    assert_eq!(app.stored_match_ids(alice.dota_player_id).await.len(), 4);
    assert_eq!(app.stored_match_ids(bob.dota_player_id).await.len(), 4);

    assert_eq!(
        app.get("/api/matches", Some(&alice.token)).await.json()["total"],
        4
    );

    app.cleanup(&[alice_steam, bob_steam]).await;
}

#[tokio::test]
async fn an_unknown_match_id_is_a_404() {
    let Some(db) = support::pool().await else {
        return skip("an_unknown_match_id_is_a_404");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let response = app
        .get(
            "/api/matches/00000000-0000-0000-0000-000000000000",
            Some(&session.token),
        )
        .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(response.error_code(), "NOT_FOUND");

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_malformed_match_id_is_a_400_in_the_usual_envelope() {
    let Some(db) = support::pool().await else {
        return skip("a_malformed_match_id_is_a_400_in_the_usual_envelope");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let response = app
        .get("/api/matches/not-a-uuid", Some(&session.token))
        .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.error_code(), "BAD_REQUEST");
    // No serde or UUID internals in the message.
    assert!(!response.body.contains("UUID parsing failed"));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_malformed_match_id_still_requires_a_session() {
    let Some(db) = support::pool().await else {
        return skip("a_malformed_match_id_still_requires_a_session");
    };
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    // Authentication must be decided before the path is even parsed, so a bad
    // id cannot be used to probe the API anonymously.
    let response = app.get("/api/matches/not-a-uuid", None).await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// Billing: trial, entitlement, checkout, settlement
// ---------------------------------------------------------------------------

/// The body a provider would POST, in this harness's stub vocabulary.
fn notification(order_id: &str, provider_payment_id: &str, status: &str) -> String {
    serde_json::json!({
        "payment_id": provider_payment_id,
        "order_id": order_id,
        "status": status,
        "amount_cents": 100,
        "currency": "usd",
    })
    .to_string()
}

fn signed() -> [(&'static str, &'static str); 1] {
    [("x-stub-signature", support::VALID_SIGNATURE)]
}

/// Open a charge and return `(our order id, the id the notification will use)`.
///
/// A notification is matched on the order id, which is ours and is stable from
/// the moment the charge is reserved — the provider's own id is not knowable
/// from the response, and with a hosted invoice it changes once the charge
/// exists, which is exactly why matching does not depend on it.
async fn checkout(app: &support::TestApp, token: &str) -> (String, String) {
    let response = app.post("/api/billing/checkout", Some(token)).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);

    let order_id = response.json()["payment"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let provider_payment_id = format!("provider-{order_id}");

    (order_id, provider_payment_id)
}

#[tokio::test]
async fn a_new_account_is_on_a_trial_it_never_asked_for() {
    let Some(db) = support::pool().await else {
        return skip("a_new_account_is_on_a_trial_it_never_asked_for");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let body = app.get("/api/billing", Some(&session.token)).await.json();

    assert_eq!(body["entitlement"], "trial");
    assert_eq!(body["subscription"]["status"], "trialing");
    assert_eq!(body["plan"]["trial_days"], 14);
    // The price lives in configuration and reaches the page from there.
    assert_eq!(body["plan"]["amount_cents"], 100);
    assert_eq!(body["plan"]["currency"], "usd");
    // Fourteen days, minus however much of the first day has already elapsed.
    assert_eq!(body["days_remaining"], 13);
    assert!(body["payments"].as_array().unwrap().is_empty());

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn the_trial_is_anchored_to_the_account_not_to_the_first_visit() {
    let Some(db) = support::pool().await else {
        return skip("the_trial_is_anchored_to_the_account_not_to_the_first_visit");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    // An account created before this phase existed: no subscription row, and a
    // creation date well in the past.
    sqlx::query("UPDATE users SET created_at = now() - interval '20 days' WHERE steam_id = $1")
        .bind(steam_id)
        .execute(&app.db)
        .await
        .unwrap();

    let body = app.get("/api/billing", Some(&session.token)).await.json();

    // Materialising the row late must not hand out a fresh fortnight.
    assert_eq!(body["entitlement"], "free");
    assert_eq!(body["subscription"]["status"], "expired");
    assert!(body["days_remaining"].is_null());

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn an_expired_trial_closes_generation_but_not_the_product() {
    let Some(db) = support::pool().await else {
        return skip("an_expired_trial_closes_generation_but_not_the_product");
    };
    let steam_id = unique_steam_id();
    let app = app_with_llm(
        db,
        MockDota::with_matches(sample_matches(12)),
        StubVerifier::rejecting(),
        StubLlm::answering(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    // Materialise the trial, then age it out.
    app.get("/api/billing", Some(&session.token)).await;
    app.expire_trial(steam_id).await;

    let refused = app.post("/api/coach/analyze", Some(&session.token)).await;
    assert_eq!(refused.status, StatusCode::PAYMENT_REQUIRED);
    assert_eq!(refused.error_code(), "PAYMENT_REQUIRED");

    // Everything measured keeps answering: an expired trial is not a lockout.
    for path in [
        "/api/stats",
        "/api/coach",
        "/api/coach/training-focus",
        "/api/benchmark",
        "/api/heroes",
        "/api/hero-intelligence",
        "/api/matches",
        "/api/billing",
    ] {
        let response = app.get(path, Some(&session.token)).await;
        assert_eq!(
            response.status,
            StatusCode::OK,
            "{path} should survive an expired trial: {}",
            response.body
        );
    }

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_deployment_with_no_payment_provider_cannot_sell_but_does_not_lock_out() {
    let Some(db) = support::pool().await else {
        return skip("a_deployment_with_no_payment_provider_cannot_sell_but_does_not_lock_out");
    };
    let steam_id = unique_steam_id();
    let mut config = test_config();
    // What `BillingConfig::from_env` derives when no credentials are present.
    config.billing.enforce = false;

    let app = support::app_with_payments(
        db,
        MockDota::with_matches(sample_matches(12)),
        StubVerifier::rejecting(),
        support::StubPayments::unconfigured(),
        config,
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;
    app.get("/api/billing", Some(&session.token)).await;
    app.expire_trial(steam_id).await;

    let overview = app.get("/api/billing", Some(&session.token)).await.json();
    assert_eq!(overview["checkout_available"], false);

    let checkout = app
        .post("/api/billing/checkout", Some(&session.token))
        .await;
    assert_eq!(checkout.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(checkout.error_code(), "FEATURE_UNAVAILABLE");

    // Nobody can pay here, so nobody is told to.
    let analyze = app.post("/api/coach/analyze", Some(&session.token)).await;
    assert_ne!(analyze.status, StatusCode::PAYMENT_REQUIRED);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn checkout_reuses_the_charge_that_is_still_open() {
    let Some(db) = support::pool().await else {
        return skip("checkout_reuses_the_charge_that_is_still_open");
    };
    let steam_id = unique_steam_id();
    let payments = support::StubPayments::taking_payments();
    let app = support::app_with_payments(
        db,
        MockDota::default().into(),
        StubVerifier::rejecting(),
        payments.clone(),
        test_config(),
    );
    let session = app.login_as(steam_id).await;

    let first = app
        .post("/api/billing/checkout", Some(&session.token))
        .await
        .json();
    let second = app
        .post("/api/billing/checkout", Some(&session.token))
        .await
        .json();

    assert_eq!(first["payment"]["id"], second["payment"]["id"]);
    assert_eq!(first["payment"]["amount_cents"], 100);
    assert!(first["payment"]["payment_url"].as_str().is_some());
    assert_eq!(
        payments.created.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "reloading the billing page must not open a second invoice"
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn an_unsigned_notification_buys_nothing() {
    let Some(db) = support::pool().await else {
        return skip("an_unsigned_notification_buys_nothing");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;
    let (order_id, provider_payment_id) = checkout(&app, &session.token).await;

    let body = notification(&order_id, &provider_payment_id, "paid");

    for headers in [vec![], vec![("x-stub-signature", "forged")]] {
        let response = app.post_body("/api/billing/webhook", &body, &headers).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        // The endpoint never explains what a correct signature would look like.
        assert!(!response.body.contains("valid-signature"));
    }

    assert_eq!(
        app.get("/api/billing", Some(&session.token)).await.json()["entitlement"],
        "trial",
        "an unverified notification must not activate anything"
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_verified_payment_activates_the_subscription() {
    let Some(db) = support::pool().await else {
        return skip("a_verified_payment_activates_the_subscription");
    };
    let steam_id = unique_steam_id();
    let app = app_with_llm(
        db,
        MockDota::with_matches(sample_matches(12)),
        StubVerifier::rejecting(),
        StubLlm::answering(),
    );
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let (order_id, provider_payment_id) = checkout(&app, &session.token).await;
    app.expire_trial(steam_id).await;

    // Refused before the money arrives.
    assert_eq!(
        app.post("/api/coach/analyze", Some(&session.token))
            .await
            .status,
        StatusCode::PAYMENT_REQUIRED
    );

    let response = app
        .post_body(
            "/api/billing/webhook",
            &notification(&order_id, &provider_payment_id, "paid"),
            &signed(),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["outcome"], "applied");

    let overview = app.get("/api/billing", Some(&session.token)).await.json();
    assert_eq!(overview["entitlement"], "pro");
    assert_eq!(overview["subscription"]["status"], "active");
    assert_eq!(overview["days_remaining"], 29);
    assert_eq!(overview["payments"][0]["status"], "paid");
    assert!(overview["payments"][0]["completed_at"].as_str().is_some());

    assert_eq!(
        app.post("/api/coach/analyze", Some(&session.token))
            .await
            .status,
        StatusCode::OK
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_redelivered_notification_does_not_buy_a_second_month() {
    let Some(db) = support::pool().await else {
        return skip("a_redelivered_notification_does_not_buy_a_second_month");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;
    let (order_id, provider_payment_id) = checkout(&app, &session.token).await;
    let body = notification(&order_id, &provider_payment_id, "paid");

    let first = app
        .post_body("/api/billing/webhook", &body, &signed())
        .await;
    assert_eq!(first.json()["outcome"], "applied");
    let (_, after_first) = app.stored_subscription(steam_id).await.unwrap();

    let second = app
        .post_body("/api/billing/webhook", &body, &signed())
        .await;
    assert_eq!(second.status, StatusCode::OK, "a retry is acknowledged");
    assert_eq!(second.json()["outcome"], "duplicate");

    let (status, after_second) = app.stored_subscription(steam_id).await.unwrap();
    assert_eq!(status, "active");
    assert_eq!(
        after_first, after_second,
        "the paid window must not move on a redelivery"
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_notification_for_the_wrong_amount_grants_nothing() {
    let Some(db) = support::pool().await else {
        return skip("a_notification_for_the_wrong_amount_grants_nothing");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;
    let (order_id, provider_payment_id) = checkout(&app, &session.token).await;

    let underpaid = serde_json::json!({
        "payment_id": provider_payment_id,
        "order_id": order_id,
        "status": "paid",
        "amount_cents": 1,
        "currency": "usd",
    })
    .to_string();

    let response = app
        .post_body("/api/billing/webhook", &underpaid, &signed())
        .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    let (status, period_end) = app.stored_subscription(steam_id).await.unwrap();
    assert_eq!(status, "trialing");
    assert!(period_end.is_none(), "a cent must not buy a month");

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_notification_for_a_charge_we_never_opened_is_a_404() {
    let Some(db) = support::pool().await else {
        return skip("a_notification_for_a_charge_we_never_opened_is_a_404");
    };
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let response = app
        .post_body(
            "/api/billing/webhook",
            &notification(
                "00000000-0000-0000-0000-000000000000",
                "someone-elses-payment",
                "paid",
            ),
            &signed(),
        )
        .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(response.error_code(), "NOT_FOUND");
}

#[tokio::test]
async fn a_settled_charge_cannot_be_reopened_by_a_later_notification() {
    let Some(db) = support::pool().await else {
        return skip("a_settled_charge_cannot_be_reopened_by_a_later_notification");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;
    let (order_id, provider_payment_id) = checkout(&app, &session.token).await;

    app.post_body(
        "/api/billing/webhook",
        &notification(&order_id, &provider_payment_id, "paid"),
        &signed(),
    )
    .await;

    // A late "failed" for the same charge: a different event, but the charge
    // has already finished.
    let late = app
        .post_body(
            "/api/billing/webhook",
            &notification(&order_id, &provider_payment_id, "failed"),
            &signed(),
        )
        .await;

    assert_eq!(late.status, StatusCode::OK);
    assert_eq!(late.json()["outcome"], "ignored");

    let overview = app.get("/api/billing", Some(&session.token)).await.json();
    assert_eq!(overview["entitlement"], "pro");
    assert_eq!(overview["payments"][0]["status"], "paid");

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn one_user_cannot_see_another_users_charges() {
    let Some(db) = support::pool().await else {
        return skip("one_user_cannot_see_another_users_charges");
    };
    let payer = unique_steam_id();
    let stranger = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let paying_session = app.login_as(payer).await;
    checkout(&app, &paying_session.token).await;

    let other_session = app.login_as(stranger).await;
    let body = app
        .get("/api/billing/payments", Some(&other_session.token))
        .await
        .json();

    assert!(body["payments"].as_array().unwrap().is_empty());

    app.cleanup(&[payer, stranger]).await;
}

#[tokio::test]
async fn billing_endpoints_require_a_session_but_the_webhook_does_not() {
    let Some(db) = support::pool().await else {
        return skip("billing_endpoints_require_a_session_but_the_webhook_does_not");
    };
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    for path in [
        "/api/billing",
        "/api/billing/subscription",
        "/api/billing/payments",
    ] {
        assert_eq!(app.get(path, None).await.status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(
        app.post("/api/billing/checkout", None).await.status,
        StatusCode::UNAUTHORIZED
    );

    // The webhook has no session to require — it is authenticated by its
    // signature, and answers about the charge rather than about the caller.
    let response = app
        .post_body(
            "/api/billing/webhook",
            &notification("00000000-0000-0000-0000-000000000000", "x", "paid"),
            &signed(),
        )
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Phase 11: the public offer, and request identity
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_plan_is_readable_without_a_session_so_the_landing_page_can_quote_it() {
    let Some(db) = support::pool().await else {
        return skip("the_plan_is_readable_without_a_session_so_the_landing_page_can_quote_it");
    };
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let response = app.get("/api/billing/plan", None).await;
    assert_eq!(response.status, StatusCode::OK);

    let body = response.json();
    assert_eq!(body["plan"]["amount_cents"], 100);
    assert_eq!(body["plan"]["currency"], "usd");
    assert_eq!(body["plan"]["trial_days"], 14);
    assert_eq!(body["checkout_available"], true);

    // Pricing copy and nothing else: no account, no provider identifiers.
    assert!(!response.body.contains("subscription"));
    assert!(!response.body.contains("secret"));
}

#[tokio::test]
async fn every_response_carries_a_request_id() {
    let Some(db) = support::pool().await else {
        return skip("every_response_carries_a_request_id");
    };
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    // Including the ones nobody is signed in for.
    let response = app.get("/api/stats", None).await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    assert!(
        response.header("x-request-id").is_some(),
        "a failed request is exactly the one a user will ask about"
    );
}

#[tokio::test]
async fn a_caller_supplied_request_id_is_echoed_but_only_when_it_is_safe_to_log() {
    let Some(db) = support::pool().await else {
        return skip("a_caller_supplied_request_id_is_echoed_but_only_when_it_is_safe_to_log");
    };
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let joined = app
        .post_body(
            "/api/billing/webhook",
            "{}",
            &[("x-request-id", "req-01hzy8abcdef")],
        )
        .await;
    assert_eq!(
        joined.header("x-request-id").as_deref(),
        Some("req-01hzy8abcdef"),
        "a sane correlation id should survive so traces join up"
    );

    // A header that would forge a log line is replaced, not repeated.
    let hostile = app
        .post_body(
            "/api/billing/webhook",
            "{}",
            &[("x-request-id", "abcdefgh ERROR payment settled")],
        )
        .await;
    let echoed = hostile.header("x-request-id").expect("an id is always set");
    assert_ne!(echoed, "abcdefgh ERROR payment settled");
    assert!(!echoed.contains(' '));
}
