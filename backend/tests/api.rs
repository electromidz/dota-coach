//! Phase 3 behaviour: authentication, identity scoping, synchronization.
//!
//! The Dota provider and Valve are stubbed; Postgres is real. Set
//! `DATABASE_URL` (or `TEST_DATABASE_URL`) to run these — without one they
//! print a skip notice rather than passing vacuously.

mod support;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use chrono::{DateTime, Datelike};
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
        ("GET", "/api/coach/roles"),
        ("POST", "/api/coach/role"),
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
        ("GET", "/api/admin/stats"),
        ("GET", "/api/admin/users"),
        ("GET", "/api/admin/users/00000000-0000-0000-0000-000000000000"),
        (
            "POST",
            "/api/admin/users/00000000-0000-0000-0000-000000000000/extend",
        ),
        (
            "POST",
            "/api/admin/users/00000000-0000-0000-0000-000000000000/disable",
        ),
        (
            "POST",
            "/api/admin/users/00000000-0000-0000-0000-000000000000/enable",
        ),
        ("GET", "/api/admin/vouchers"),
        ("POST", "/api/admin/vouchers"),
        ("GET", "/api/admin/vouchers/00000000-0000-0000-0000-000000000000"),
        (
            "POST",
            "/api/admin/vouchers/00000000-0000-0000-0000-000000000000/deactivate",
        ),
        ("GET", "/api/admin/audit-log"),
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

    let roles = body["role_analysis"]["roles"].as_array().unwrap();
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0]["role"], "carry");
    assert_eq!(roles[0]["matches"], 10);
    assert_eq!(body["role_analysis"]["unclassified_matches"], 0);

    // The population every number above was computed over, stated on the
    // response rather than assumed by the client.
    assert_eq!(body["scope"]["population"], "ranked_public_all_pick");
    assert_eq!(body["scope"]["analyzed_matches"], 10);
    assert_eq!(body["scope"]["confidence"], "limited");
    assert_eq!(body["eligibility"]["total_matches"], 10);
    assert_eq!(body["eligibility"]["eligible_matches"], 10);

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;
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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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

    // No matches at all: the answer is "sync", not "choose a role". A player
    // with nothing stored cannot meaningfully pick one, and sending them to a
    // selection screen with five empty options would be a dead end.
    let response = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await;

    assert_eq!(response.status, StatusCode::CONFLICT);
    assert!(response.body.contains("Sync"), "{}", response.body);

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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
async fn the_background_sweep_expires_a_trial_nobody_ever_checked_on() {
    let Some(db) = support::pool().await else {
        return skip("the_background_sweep_expires_a_trial_nobody_ever_checked_on");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    app.login_as(steam_id).await;

    // A trial that lapsed a day ago, and that nothing has touched since —
    // no `/api/billing` call, which is normally what corrects this.
    sqlx::query(
        "INSERT INTO subscriptions (user_id, status, plan, trial_started_at, trial_ends_at)
         SELECT u.id, 'trialing', 'pro', now() - interval '15 days', now() - interval '1 day'
           FROM users u WHERE u.steam_id = $1",
    )
    .bind(steam_id)
    .execute(&app.db)
    .await
    .unwrap();

    let swept = dota_coach_backend::services::billing::sweep_expired(&app.db)
        .await
        .unwrap();
    assert!(swept >= 1, "the lapsed row should have been picked up");

    let (status, _) = app.stored_subscription(steam_id).await.unwrap();
    assert_eq!(status, "expired");

    let user_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE steam_id = $1")
        .bind(steam_id)
        .fetch_one(&app.db)
        .await
        .unwrap();
    let trial_expired_events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM events WHERE user_id = $1 AND type = 'trial_expired'",
    )
    .bind(user_id)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(trial_expired_events, 1);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_lapsed_paid_period_emits_subscription_expired_not_trial_expired() {
    let Some(db) = support::pool().await else {
        return skip("a_lapsed_paid_period_emits_subscription_expired_not_trial_expired");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    app.login_as(steam_id).await;

    // A paid period that ended yesterday, source `payment` — not a trial.
    sqlx::query(
        "INSERT INTO subscriptions
             (user_id, status, plan, source, trial_started_at, trial_ends_at, current_period_start, current_period_end)
         SELECT u.id, 'active', 'pro', 'payment', now() - interval '45 days', now() - interval '31 days',
                now() - interval '31 days', now() - interval '1 day'
           FROM users u WHERE u.steam_id = $1",
    )
    .bind(steam_id)
    .execute(&app.db)
    .await
    .unwrap();

    let swept = dota_coach_backend::services::billing::sweep_expired(&app.db)
        .await
        .unwrap();
    assert!(swept >= 1);

    let (status, _) = app.stored_subscription(steam_id).await.unwrap();
    assert_eq!(status, "expired");

    let user_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE steam_id = $1")
        .bind(steam_id)
        .fetch_one(&app.db)
        .await
        .unwrap();
    let types: Vec<String> =
        sqlx::query_scalar("SELECT type FROM events WHERE user_id = $1 AND type LIKE '%expired%'")
            .bind(user_id)
            .fetch_all(&app.db)
            .await
            .unwrap();
    assert_eq!(
        types,
        vec!["subscription_expired"],
        "a lapsed paid period is not a trial expiring"
    );

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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
    // Coaching is role-scoped: these matches are safe-lane carries.
    app.choose_role(&session, "carry").await;

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

// ---------------------------------------------------------------------------
// API documentation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_documentation_is_served_when_it_is_enabled() {
    let Some(db) = support::pool().await else {
        return skip("the_documentation_is_served_when_it_is_enabled");
    };

    let mut config = test_config();
    config.docs_enabled = true;

    let app = app_with_config(
        db,
        MockDota::default().into(),
        StubVerifier::rejecting(),
        config,
    );

    let spec = app.get("/api-docs/openapi.json", None).await;
    assert_eq!(spec.status, 200, "the document is readable without a session");
    assert_eq!(spec.json()["info"]["title"], "Dota Coach API");
}

/// The gate has to remove the routes, not merely hide the link: a deployment
/// with docs off must not serve the document to anyone who guesses the path.
#[tokio::test]
async fn the_documentation_is_absent_when_it_is_disabled() {
    let Some(db) = support::pool().await else {
        return skip("the_documentation_is_absent_when_it_is_disabled");
    };

    let mut config = test_config();
    config.docs_enabled = false;

    let app = app_with_config(
        db,
        MockDota::default().into(),
        StubVerifier::rejecting(),
        config,
    );

    for path in ["/docs", "/docs/", "/api-docs/openapi.json"] {
        let response = app.get(path, None).await;
        assert_eq!(response.status, 404, "{path} is served with docs disabled");
    }

    // The rest of the API is unaffected by the gate.
    assert_eq!(app.get("/health", None).await.status, 200);
}

// ---------------------------------------------------------------------------
// Admin
// ---------------------------------------------------------------------------

async fn make_admin(app: &support::TestApp, steam_id: i64) {
    sqlx::query("UPDATE users SET is_admin = true WHERE steam_id = $1")
        .bind(steam_id)
        .execute(&app.db)
        .await
        .unwrap();
}

#[tokio::test]
async fn admin_routes_reject_a_signed_in_non_admin() {
    let Some(db) = support::pool().await else {
        return skip("admin_routes_reject_a_signed_in_non_admin");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let other_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE steam_id = $1")
        .bind(steam_id)
        .fetch_one(&app.db)
        .await
        .unwrap();

    // Every admin route, not just one — a `Router` mount is easy to add
    // without wiring the extractor onto it, and that mistake would only show
    // up here.
    for (method, path) in [
        ("GET", "/api/admin/stats".to_string()),
        ("GET", "/api/admin/users".to_string()),
        ("GET", format!("/api/admin/users/{other_id}")),
        ("POST", format!("/api/admin/users/{other_id}/extend")),
        ("POST", format!("/api/admin/users/{other_id}/disable")),
        ("POST", format!("/api/admin/users/{other_id}/enable")),
        ("GET", "/api/admin/vouchers".to_string()),
        ("POST", "/api/admin/vouchers".to_string()),
        ("GET", format!("/api/admin/vouchers/{other_id}")),
        ("POST", format!("/api/admin/vouchers/{other_id}/deactivate")),
        ("GET", "/api/admin/audit-log".to_string()),
    ] {
        let response = if method == "GET" {
            app.get(&path, Some(&session.token)).await
        } else {
            app.post_json(&path, r#"{"days": 1}"#, Some(&session.token)).await
        };

        assert_eq!(
            response.status,
            StatusCode::FORBIDDEN,
            "{method} {path} should refuse a non-admin"
        );
        assert_eq!(response.json()["error"]["code"], "FORBIDDEN");
    }

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_disabled_admin_is_refused_before_the_admin_check_even_runs() {
    let Some(db) = support::pool().await else {
        return skip("a_disabled_admin_is_refused_before_the_admin_check_even_runs");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    sqlx::query("UPDATE users SET is_admin = true, status = 'disabled' WHERE steam_id = $1")
        .bind(steam_id)
        .execute(&app.db)
        .await
        .unwrap();

    let response = app.get("/api/admin/stats", Some(&session.token)).await;

    // Not FORBIDDEN: `AdminUser` wraps `CurrentUser`, and `CurrentUser`
    // rejects a disabled account before the `is_admin` check is ever reached.
    // An admin who gets disabled loses the panel too, immediately.
    assert_eq!(response.status, StatusCode::FORBIDDEN);
    assert_eq!(response.json()["error"]["code"], "ACCOUNT_DISABLED");

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn an_admin_can_search_and_filter_the_user_list() {
    let Some(db) = support::pool().await else {
        return skip("an_admin_can_search_and_filter_the_user_list");
    };
    let admin_steam_id = unique_steam_id();
    let target_steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let admin_session = app.login_as(admin_steam_id).await;
    make_admin(&app, admin_steam_id).await;
    app.login_as(target_steam_id).await;

    let found = app
        .get(
            &format!("/api/admin/users?search={target_steam_id}"),
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(found.status, StatusCode::OK);
    let body = found.json();
    assert_eq!(body["total"], 1);
    assert_eq!(body["users"][0]["steam_id"], target_steam_id.to_string());
    // No subscription row yet: `login_as` bypasses the real login path that
    // materializes the trial (Phase 3).
    assert!(body["users"][0]["subscription_status"].is_null());

    let filtered_out = app
        .get(
            &format!("/api/admin/users?search={target_steam_id}&status=disabled"),
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(filtered_out.json()["total"], 0);

    app.cleanup(&[admin_steam_id, target_steam_id]).await;
}

#[tokio::test]
async fn an_admin_can_view_one_accounts_profile_and_timeline() {
    let Some(db) = support::pool().await else {
        return skip("an_admin_can_view_one_accounts_profile_and_timeline");
    };
    let admin_steam_id = unique_steam_id();
    let target_steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let admin_session = app.login_as(admin_steam_id).await;
    make_admin(&app, admin_steam_id).await;
    app.login_as(target_steam_id).await;

    let target_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE steam_id = $1")
        .bind(target_steam_id)
        .fetch_one(&app.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO events (user_id, type, metadata) VALUES ($1, 'feature_used', '{}')")
        .bind(target_id)
        .execute(&app.db)
        .await
        .unwrap();

    let response = app
        .get(&format!("/api/admin/users/{target_id}"), Some(&admin_session.token))
        .await;
    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    assert_eq!(body["steam_id"], target_steam_id.to_string());
    assert_eq!(body["events"][0]["type"], "feature_used");

    let missing = app
        .get(
            "/api/admin/users/00000000-0000-0000-0000-000000000000",
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    app.cleanup(&[admin_steam_id, target_steam_id]).await;
}

#[tokio::test]
async fn an_admin_can_extend_a_lapsed_trial_back_to_trialing() {
    let Some(db) = support::pool().await else {
        return skip("an_admin_can_extend_a_lapsed_trial_back_to_trialing");
    };
    let admin_steam_id = unique_steam_id();
    let target_steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let admin_session = app.login_as(admin_steam_id).await;
    make_admin(&app, admin_steam_id).await;
    let target_session = app.login_as(target_steam_id).await;

    sqlx::query(
        "INSERT INTO subscriptions (user_id, status, plan, trial_started_at, trial_ends_at)
         SELECT id, 'expired', 'pro', now() - interval '20 days', now() - interval '6 days'
           FROM users WHERE steam_id = $1",
    )
    .bind(target_steam_id)
    .execute(&app.db)
    .await
    .unwrap();

    let target_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE steam_id = $1")
        .bind(target_steam_id)
        .fetch_one(&app.db)
        .await
        .unwrap();

    let response = app
        .post_json(
            &format!("/api/admin/users/{target_id}/extend"),
            r#"{"days": 7}"#,
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["subscription_status"], "trialing");

    // The extension actually restores access, not just the label.
    let billing = app
        .get("/api/billing", Some(&target_session.token))
        .await
        .json();
    assert_eq!(billing["entitlement"], "trial");

    let bad_days = app
        .post_json(
            &format!("/api/admin/users/{target_id}/extend"),
            r#"{"days": 0}"#,
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(bad_days.status, StatusCode::BAD_REQUEST);

    app.cleanup(&[admin_steam_id, target_steam_id]).await;
}

#[tokio::test]
async fn disabling_an_account_locks_it_out_on_its_very_next_request() {
    let Some(db) = support::pool().await else {
        return skip("disabling_an_account_locks_it_out_on_its_very_next_request");
    };
    let admin_steam_id = unique_steam_id();
    let target_steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let admin_session = app.login_as(admin_steam_id).await;
    make_admin(&app, admin_steam_id).await;
    let target_session = app.login_as(target_steam_id).await;

    assert_eq!(
        app.get("/api/players/me", Some(&target_session.token))
            .await
            .status,
        StatusCode::OK
    );

    let target_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE steam_id = $1")
        .bind(target_steam_id)
        .fetch_one(&app.db)
        .await
        .unwrap();

    let response = app
        .post(
            &format!("/api/admin/users/{target_id}/disable"),
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["status"], "disabled");

    // Same session, no re-login: the extractor checks status fresh every
    // request, so no session revocation is needed.
    let after = app.get("/api/players/me", Some(&target_session.token)).await;
    assert_eq!(after.status, StatusCode::FORBIDDEN);
    assert_eq!(after.json()["error"]["code"], "ACCOUNT_DISABLED");

    let missing = app
        .post(
            "/api/admin/users/00000000-0000-0000-0000-000000000000/disable",
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    app.cleanup(&[admin_steam_id, target_steam_id]).await;
}

#[tokio::test]
async fn admin_stats_validates_the_window_and_answers_the_right_shape() {
    let Some(db) = support::pool().await else {
        return skip("admin_stats_validates_the_window_and_answers_the_right_shape");
    };
    let admin_steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let admin_session = app.login_as(admin_steam_id).await;
    make_admin(&app, admin_steam_id).await;

    let backwards = app
        .get(
            "/api/admin/stats?from=2026-02-01T00:00:00Z&to=2026-01-01T00:00:00Z",
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(backwards.status, StatusCode::BAD_REQUEST);

    let response = app
        .get(
            "/api/admin/stats?from=2026-01-01T00:00:00Z&to=2026-01-05T00:00:00Z",
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    assert_eq!(body["currency"], "usd");
    // Jan 1 through Jan 5 inclusive: five days, no gaps.
    assert_eq!(body["daily"].as_array().unwrap().len(), 5);
    assert!(body["total_users"].as_i64().unwrap() >= 1);

    app.cleanup(&[admin_steam_id]).await;
}

#[tokio::test]
async fn trial_conversion_counts_the_cohort_that_actually_started_in_the_window() {
    let Some(db) = support::pool().await else {
        return skip("trial_conversion_counts_the_cohort_that_actually_started_in_the_window");
    };
    // Synthetic, far-past timestamps: real tests running concurrently against
    // this same database write events with `now()`, so a window this test
    // owns outright is the only way to get an exact count.
    let steam_ids = [unique_steam_id(), unique_steam_id(), unique_steam_id()];
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    for steam_id in steam_ids {
        app.login_as(steam_id).await;
    }
    let ids: Vec<uuid::Uuid> = {
        let mut v = Vec::new();
        for steam_id in steam_ids {
            let id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE steam_id = $1")
                .bind(steam_id)
                .fetch_one(&app.db)
                .await
                .unwrap();
            v.push(id);
        }
        v
    };

    // A: started in-window, converted. B: started in-window, never converted.
    // C: started before the window — must not count toward the cohort.
    sqlx::query(
        "INSERT INTO events (user_id, type, metadata, created_at) VALUES
             ($1, 'trial_started', '{}', '2020-06-01T00:00:00Z'),
             ($1, 'purchase', '{}', '2020-06-05T00:00:00Z'),
             ($2, 'trial_started', '{}', '2020-06-02T00:00:00Z'),
             ($3, 'trial_started', '{}', '2020-05-01T00:00:00Z')",
    )
    .bind(ids[0])
    .bind(ids[1])
    .bind(ids[2])
    .execute(&app.db)
    .await
    .unwrap();

    let from = DateTime::parse_from_rfc3339("2020-06-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let to = DateTime::parse_from_rfc3339("2020-06-30T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    let (cohort_size, purchased) =
        dota_coach_backend::repositories::admin::trial_conversion(&app.db, from, to)
            .await
            .unwrap();
    assert_eq!(cohort_size, 2, "only A and B started inside the window");
    assert_eq!(purchased, 1, "only A converted");

    app.cleanup(&steam_ids).await;
}

/// Parses an RFC3339 literal into a `DateTime<Utc>`, for tests that need an
/// exact, synthetic instant rather than `now()`.
fn rfc3339(s: &str) -> chrono::DateTime<chrono::Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&chrono::Utc)
}

#[tokio::test]
async fn revenue_counts_only_settled_payments_inside_the_window() {
    let Some(db) = support::pool().await else {
        return skip("revenue_counts_only_settled_payments_inside_the_window");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    app.login_as(steam_id).await;

    let user_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE steam_id = $1")
        .bind(steam_id)
        .fetch_one(&app.db)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO payments (user_id, provider, amount_cents, currency, status, completed_at) VALUES
             ($1, 'nowpayments', 500, 'usd', 'paid', '2020-07-01T00:00:00Z'),
             ($1, 'nowpayments', 700, 'usd', 'paid', '2020-06-15T00:00:00Z'),
             ($1, 'nowpayments', 999, 'usd', 'refunded', '2020-07-10T00:00:00Z'),
             ($1, 'nowpayments', 111, 'usd', 'pending', NULL)",
    )
    .bind(user_id)
    .execute(&app.db)
    .await
    .unwrap();

    let revenue = dota_coach_backend::repositories::admin::revenue_cents(
        &app.db,
        rfc3339("2020-07-01T00:00:00Z"),
        rfc3339("2020-07-31T00:00:00Z"),
    )
    .await
    .unwrap();

    assert_eq!(
        revenue, 500,
        "only the paid charge settled inside the window should count"
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn active_since_counts_distinct_accounts_not_raw_login_events() {
    let Some(db) = support::pool().await else {
        return skip("active_since_counts_distinct_accounts_not_raw_login_events");
    };
    let steam_ids = [unique_steam_id(), unique_steam_id(), unique_steam_id()];
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    for steam_id in steam_ids {
        app.login_as(steam_id).await;
    }
    let ids: Vec<uuid::Uuid> = {
        let mut v = Vec::new();
        for steam_id in steam_ids {
            v.push(
                sqlx::query_scalar("SELECT id FROM users WHERE steam_id = $1")
                    .bind(steam_id)
                    .fetch_one(&app.db)
                    .await
                    .unwrap(),
            );
        }
        v
    };

    // A logs in three times inside the window, B once, C once but before it.
    sqlx::query(
        "INSERT INTO events (user_id, type, metadata, created_at) VALUES
             ($1, 'login', '{}', '2022-01-10T00:00:00Z'),
             ($1, 'login', '{}', '2022-01-11T00:00:00Z'),
             ($1, 'login', '{}', '2022-01-12T00:00:00Z'),
             ($2, 'login', '{}', '2022-01-11T00:00:00Z'),
             ($3, 'login', '{}', '2021-12-01T00:00:00Z')",
    )
    .bind(ids[0])
    .bind(ids[1])
    .bind(ids[2])
    .execute(&app.db)
    .await
    .unwrap();

    let active = dota_coach_backend::repositories::admin::count_active_since(
        &app.db,
        rfc3339("2022-01-01T00:00:00Z"),
    )
    .await
    .unwrap();

    assert_eq!(active, 2, "A and B, counted once each — not five raw events");

    app.cleanup(&steam_ids).await;
}

#[tokio::test]
async fn the_daily_series_has_no_gaps_on_a_day_with_no_activity() {
    let Some(db) = support::pool().await else {
        return skip("the_daily_series_has_no_gaps_on_a_day_with_no_activity");
    };
    let steam_ids = [unique_steam_id(), unique_steam_id(), unique_steam_id()];
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    for steam_id in steam_ids {
        app.login_as(steam_id).await;
    }

    // Two signups on the 2nd, one on the 4th, nothing on the 1st, 3rd or 5th.
    sqlx::query(
        "UPDATE users SET created_at = '2021-03-02T00:00:00Z' WHERE steam_id IN ($1, $2)",
    )
    .bind(steam_ids[0])
    .bind(steam_ids[1])
    .execute(&app.db)
    .await
    .unwrap();
    sqlx::query("UPDATE users SET created_at = '2021-03-04T00:00:00Z' WHERE steam_id = $1")
        .bind(steam_ids[2])
        .execute(&app.db)
        .await
        .unwrap();

    let user_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE steam_id = $1")
        .bind(steam_ids[0])
        .fetch_one(&app.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO events (user_id, type, metadata, created_at) VALUES ($1, 'login', '{}', '2021-03-03T12:00:00Z')")
        .bind(user_id)
        .execute(&app.db)
        .await
        .unwrap();

    let series = dota_coach_backend::repositories::admin::daily_series(
        &app.db,
        chrono::NaiveDate::from_ymd_opt(2021, 3, 1).unwrap(),
        chrono::NaiveDate::from_ymd_opt(2021, 3, 5).unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(series.len(), 5, "every day in range appears, gap or not");
    let by_day: Vec<(i64, i64)> = series.iter().map(|d| (d.signups, d.logins)).collect();
    assert_eq!(
        by_day,
        vec![(0, 0), (2, 0), (0, 1), (1, 0), (0, 0)],
        "March 1 through 5, in order, zero-filled where nothing happened"
    );

    app.cleanup(&steam_ids).await;
}

// ---------------------------------------------------------------------------
// Vouchers
// ---------------------------------------------------------------------------

fn new_voucher(
    duration_days: i32,
    max_uses: i32,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
) -> dota_coach_backend::repositories::voucher::NewVoucher {
    dota_coach_backend::repositories::voucher::NewVoucher {
        duration_days,
        max_uses,
        expires_at,
        note: None,
        created_by: None,
    }
}

async fn user_id_for(app: &support::TestApp, steam_id: i64) -> uuid::Uuid {
    sqlx::query_scalar("SELECT id FROM users WHERE steam_id = $1")
        .bind(steam_id)
        .fetch_one(&app.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn redeeming_a_valid_code_grants_time_and_records_the_event() {
    let Some(db) = support::pool().await else {
        return skip("redeeming_a_valid_code_grants_time_and_records_the_event");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let voucher = dota_coach_backend::repositories::voucher::create(&app.db, &new_voucher(30, 1, None))
        .await
        .unwrap();

    let response = app
        .post_json(
            "/api/subscribe/redeem",
            &format!(r#"{{"code": "{}"}}"#, voucher.code),
            Some(&session.token),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    assert_eq!(body["subscription"]["status"], "active");
    assert_eq!(body["subscription"]["source"], "voucher");

    let user_id = user_id_for(&app, steam_id).await;
    let event_metadata: serde_json::Value = sqlx::query_scalar(
        "SELECT metadata FROM events WHERE user_id = $1 AND type = 'voucher_redeemed'",
    )
    .bind(user_id)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(event_metadata["code"], voucher.code);
    assert_eq!(event_metadata["duration_days"], 30);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn an_expired_voucher_is_refused() {
    let Some(db) = support::pool().await else {
        return skip("an_expired_voucher_is_refused");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let voucher = dota_coach_backend::repositories::voucher::create(
        &app.db,
        &new_voucher(30, 1, Some(rfc3339("2020-01-01T00:00:00Z"))),
    )
    .await
    .unwrap();

    let response = app
        .post_json(
            "/api/subscribe/redeem",
            &format!(r#"{{"code": "{}"}}"#, voucher.code),
            Some(&session.token),
        )
        .await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert!(response.json()["error"]["message"]
        .as_str()
        .unwrap()
        .contains("expired"));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_voucher_already_at_its_use_limit_is_refused() {
    let Some(db) = support::pool().await else {
        return skip("a_voucher_already_at_its_use_limit_is_refused");
    };
    let steam_ids = [unique_steam_id(), unique_steam_id()];
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let first_session = app.login_as(steam_ids[0]).await;
    let second_session = app.login_as(steam_ids[1]).await;

    let voucher = dota_coach_backend::repositories::voucher::create(&app.db, &new_voucher(7, 1, None))
        .await
        .unwrap();

    let first = app
        .post_json(
            "/api/subscribe/redeem",
            &format!(r#"{{"code": "{}"}}"#, voucher.code),
            Some(&first_session.token),
        )
        .await;
    assert_eq!(first.status, StatusCode::OK, "the first redemption should succeed");

    let second = app
        .post_json(
            "/api/subscribe/redeem",
            &format!(r#"{{"code": "{}"}}"#, voucher.code),
            Some(&second_session.token),
        )
        .await;
    assert_eq!(second.status, StatusCode::BAD_REQUEST);
    assert!(second.json()["error"]["message"]
        .as_str()
        .unwrap()
        .contains("maximum number"));

    app.cleanup(&steam_ids).await;
}

#[tokio::test]
async fn redeeming_the_same_code_twice_is_refused_the_second_time() {
    let Some(db) = support::pool().await else {
        return skip("redeeming_the_same_code_twice_is_refused_the_second_time");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    // max_uses well above 1, so a rejection here can only be "you already
    // redeemed this", never "someone else used it up".
    let voucher = dota_coach_backend::repositories::voucher::create(&app.db, &new_voucher(7, 5, None))
        .await
        .unwrap();

    let first = app
        .post_json(
            "/api/subscribe/redeem",
            &format!(r#"{{"code": "{}"}}"#, voucher.code),
            Some(&session.token),
        )
        .await;
    assert_eq!(first.status, StatusCode::OK);

    let second = app
        .post_json(
            "/api/subscribe/redeem",
            &format!(r#"{{"code": "{}"}}"#, voucher.code),
            Some(&session.token),
        )
        .await;
    assert_eq!(second.status, StatusCode::CONFLICT);
    assert!(second.json()["error"]["message"]
        .as_str()
        .unwrap()
        .contains("already redeemed"));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn concurrent_redemption_of_a_single_use_voucher_lets_exactly_one_through() {
    let Some(db) = support::pool().await else {
        return skip("concurrent_redemption_of_a_single_use_voucher_lets_exactly_one_through");
    };
    let steam_ids: Vec<i64> = (0..5).map(|_| unique_steam_id()).collect();
    let app = app(db.clone(), MockDota::default().into(), StubVerifier::rejecting());
    for steam_id in &steam_ids {
        app.login_as(*steam_id).await;
    }
    let mut user_ids = Vec::new();
    for steam_id in &steam_ids {
        user_ids.push(user_id_for(&app, *steam_id).await);
    }

    let voucher = dota_coach_backend::repositories::voucher::create(&app.db, &new_voucher(7, 1, None))
        .await
        .unwrap();

    // Five different accounts racing the same single-use code — the row
    // lock in `services::voucher::redeem` is what has to serialize this
    // correctly, not the pre-checks, which every task reads before any of
    // them has written anything back.
    let mut handles = Vec::new();
    for user_id in user_ids {
        let pool = db.clone();
        let code = voucher.code.clone();
        handles.push(tokio::spawn(async move {
            let limiter = dota_coach_backend::services::voucher::RateLimiter::new();
            let billing_config = test_config().billing;
            dota_coach_backend::services::voucher::redeem(
                &pool,
                &billing_config,
                &limiter,
                user_id,
                &code,
            )
            .await
        }));
    }

    let mut succeeded = 0;
    let mut used_up = 0;
    for handle in handles {
        match handle.await.unwrap() {
            Ok(_) => succeeded += 1,
            Err(dota_coach_backend::services::voucher::VoucherError::UsedUp) => used_up += 1,
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    assert_eq!(succeeded, 1, "exactly one of the five should have won the race");
    assert_eq!(used_up, 4);

    app.cleanup(&steam_ids).await;
}

#[tokio::test]
async fn redeeming_extends_an_existing_paid_period_rather_than_replacing_it() {
    let Some(db) = support::pool().await else {
        return skip("redeeming_extends_an_existing_paid_period_rather_than_replacing_it");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    // Already ten days into a paid period from a real payment. `login_as`
    // creates no subscription row at all, so this must be an INSERT, not an
    // UPDATE — an UPDATE matching zero rows fails silently and this test
    // would otherwise "pass" while testing nothing.
    sqlx::query(
        "INSERT INTO subscriptions
             (user_id, status, plan, source, trial_started_at, trial_ends_at,
              current_period_start, current_period_end)
         SELECT id, 'active', 'pro', 'payment', now() - interval '30 days', now() - interval '16 days',
                now() - interval '20 days', now() + interval '10 days'
           FROM users WHERE steam_id = $1",
    )
    .bind(steam_id)
    .execute(&app.db)
    .await
    .unwrap();

    let voucher = dota_coach_backend::repositories::voucher::create(&app.db, &new_voucher(30, 1, None))
        .await
        .unwrap();

    let response = app
        .post_json(
            "/api/subscribe/redeem",
            &format!(r#"{{"code": "{}"}}"#, voucher.code),
            Some(&session.token),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);

    let (_, period_end) = app.stored_subscription(steam_id).await.unwrap();
    let period_end = period_end.expect("a paid period end should be set");

    // 10 days already remaining + 30 from the voucher = 40, not a fresh 30
    // from now — redeeming must not have thrown away the paid days.
    let remaining = (period_end - chrono::Utc::now()).num_days();
    assert!(
        (38..=40).contains(&remaining),
        "expected roughly 40 days remaining, got {remaining}"
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_sixth_redeem_attempt_within_a_minute_is_rate_limited() {
    let Some(db) = support::pool().await else {
        return skip("a_sixth_redeem_attempt_within_a_minute_is_rate_limited");
    };
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;

    let mut last_status = StatusCode::OK;
    for _ in 0..6 {
        last_status = app
            .post_json(
                "/api/subscribe/redeem",
                r#"{"code": "DOTA-0000-0000"}"#,
                Some(&session.token),
            )
            .await
            .status;
    }

    assert_eq!(last_status, StatusCode::TOO_MANY_REQUESTS);

    app.cleanup(&[steam_id]).await;
}

// ---------------------------------------------------------------------------
// Admin: generalized extend, enable, and vouchers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn extending_a_paid_account_extends_the_paid_period_not_the_trial() {
    let Some(db) = support::pool().await else {
        return skip("extending_a_paid_account_extends_the_paid_period_not_the_trial");
    };
    let admin_steam_id = unique_steam_id();
    let target_steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let admin_session = app.login_as(admin_steam_id).await;
    make_admin(&app, admin_steam_id).await;
    app.login_as(target_steam_id).await;

    sqlx::query(
        "INSERT INTO subscriptions
             (user_id, status, plan, source, trial_started_at, trial_ends_at, current_period_end)
         SELECT id, 'active', 'pro', 'payment', now() - interval '60 days', now() - interval '46 days',
                now() + interval '5 days'
           FROM users WHERE steam_id = $1",
    )
    .bind(target_steam_id)
    .execute(&app.db)
    .await
    .unwrap();

    let target_id = user_id_for(&app, target_steam_id).await;

    let response = app
        .post_json(
            &format!("/api/admin/users/{target_id}/extend"),
            r#"{"days": 10}"#,
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response.json()["subscription_status"],
        "active",
        "still active, not bounced through trialing"
    );

    let (_, period_end) = app.stored_subscription(target_steam_id).await.unwrap();
    let remaining = (period_end.unwrap() - chrono::Utc::now()).num_days();
    assert!(
        (13..=15).contains(&remaining),
        "expected roughly 15 days (5 already + 10 granted), got {remaining}"
    );

    app.cleanup(&[admin_steam_id, target_steam_id]).await;
}

#[tokio::test]
async fn an_admin_can_enable_a_disabled_account() {
    let Some(db) = support::pool().await else {
        return skip("an_admin_can_enable_a_disabled_account");
    };
    let admin_steam_id = unique_steam_id();
    let target_steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let admin_session = app.login_as(admin_steam_id).await;
    make_admin(&app, admin_steam_id).await;
    let target_session = app.login_as(target_steam_id).await;
    let target_id = user_id_for(&app, target_steam_id).await;

    app.post(
        &format!("/api/admin/users/{target_id}/disable"),
        Some(&admin_session.token),
    )
    .await;
    assert_eq!(
        app.get("/api/players/me", Some(&target_session.token))
            .await
            .status,
        StatusCode::FORBIDDEN,
        "sanity check: disabling actually took effect"
    );

    let response = app
        .post(
            &format!("/api/admin/users/{target_id}/enable"),
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["status"], "active");

    assert_eq!(
        app.get("/api/players/me", Some(&target_session.token))
            .await
            .status,
        StatusCode::OK,
        "the same session works again — no re-login needed"
    );

    let missing = app
        .post(
            "/api/admin/users/00000000-0000-0000-0000-000000000000/enable",
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    app.cleanup(&[admin_steam_id, target_steam_id]).await;
}

#[tokio::test]
async fn an_admin_can_create_single_and_bulk_vouchers() {
    let Some(db) = support::pool().await else {
        return skip("an_admin_can_create_single_and_bulk_vouchers");
    };
    let admin_steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let admin_session = app.login_as(admin_steam_id).await;
    make_admin(&app, admin_steam_id).await;

    let single = app
        .post_json(
            "/api/admin/vouchers",
            r#"{"duration_days": 14, "max_uses": 3, "note": "single"}"#,
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(single.status, StatusCode::OK);
    let single_body = single.json();
    let vouchers = single_body["vouchers"].as_array().unwrap();
    assert_eq!(vouchers.len(), 1);
    let code = vouchers[0]["code"].as_str().unwrap();
    assert!(
        code.starts_with("DOTA-") && code.len() == 14,
        "unexpected code shape: {code}"
    );
    assert_eq!(vouchers[0]["active"], true);
    assert_eq!(vouchers[0]["used_count"], 0);

    let bulk = app
        .post_json(
            "/api/admin/vouchers",
            r#"{"duration_days": 7, "max_uses": 1, "count": 5}"#,
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(bulk.status, StatusCode::OK);
    let bulk_body = bulk.json();
    let bulk_vouchers = bulk_body["vouchers"].as_array().unwrap();
    assert_eq!(bulk_vouchers.len(), 5);
    let codes: std::collections::HashSet<&str> = bulk_vouchers
        .iter()
        .map(|v| v["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes.len(), 5, "five independent codes, no duplicates");

    let bad = app
        .post_json(
            "/api/admin/vouchers",
            r#"{"duration_days": 0, "max_uses": 1}"#,
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);

    app.cleanup(&[admin_steam_id]).await;
}

#[tokio::test]
async fn deactivating_a_voucher_blocks_future_redemption() {
    let Some(db) = support::pool().await else {
        return skip("deactivating_a_voucher_blocks_future_redemption");
    };
    let admin_steam_id = unique_steam_id();
    let target_steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let admin_session = app.login_as(admin_steam_id).await;
    make_admin(&app, admin_steam_id).await;
    let target_session = app.login_as(target_steam_id).await;

    let voucher =
        dota_coach_backend::repositories::voucher::create(&app.db, &new_voucher(14, 5, None))
            .await
            .unwrap();

    let deactivate = app
        .post(
            &format!("/api/admin/vouchers/{}/deactivate", voucher.id),
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(deactivate.status, StatusCode::OK);
    assert_eq!(deactivate.json()["active"], false);

    let redeem = app
        .post_json(
            "/api/subscribe/redeem",
            &format!(r#"{{"code": "{}"}}"#, voucher.code),
            Some(&target_session.token),
        )
        .await;
    assert_eq!(redeem.status, StatusCode::BAD_REQUEST);
    assert!(redeem.json()["error"]["message"]
        .as_str()
        .unwrap()
        .contains("no longer active"));

    let missing = app
        .post(
            "/api/admin/vouchers/00000000-0000-0000-0000-000000000000/deactivate",
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    app.cleanup(&[admin_steam_id, target_steam_id]).await;
}

#[tokio::test]
async fn the_voucher_detail_page_lists_who_redeemed_it() {
    let Some(db) = support::pool().await else {
        return skip("the_voucher_detail_page_lists_who_redeemed_it");
    };
    let admin_steam_id = unique_steam_id();
    let redeemer_ids = [unique_steam_id(), unique_steam_id()];
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let admin_session = app.login_as(admin_steam_id).await;
    make_admin(&app, admin_steam_id).await;

    let mut sessions = Vec::new();
    for steam_id in redeemer_ids {
        sessions.push(app.login_as(steam_id).await);
    }

    let voucher =
        dota_coach_backend::repositories::voucher::create(&app.db, &new_voucher(14, 2, None))
            .await
            .unwrap();

    for session in &sessions {
        let response = app
            .post_json(
                "/api/subscribe/redeem",
                &format!(r#"{{"code": "{}"}}"#, voucher.code),
                Some(&session.token),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK);
    }

    let detail = app
        .get(
            &format!("/api/admin/vouchers/{}", voucher.id),
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(detail.status, StatusCode::OK);
    let body = detail.json();
    assert_eq!(body["used_count"], 2);
    let redemptions = body["redemptions"].as_array().unwrap();
    assert_eq!(redemptions.len(), 2);
    let redeemed_steam_ids: std::collections::HashSet<String> = redemptions
        .iter()
        .map(|r| r["steam_id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        redeemed_steam_ids,
        redeemer_ids.iter().map(|id| id.to_string()).collect(),
    );

    let missing = app
        .get(
            "/api/admin/vouchers/00000000-0000-0000-0000-000000000000",
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    let mut all_steam_ids = vec![admin_steam_id];
    all_steam_ids.extend(redeemer_ids);
    app.cleanup(&all_steam_ids).await;
}

#[tokio::test]
async fn admin_stats_count_voucher_redemptions_and_daily_purchases() {
    let Some(db) = support::pool().await else {
        return skip("admin_stats_count_voucher_redemptions_and_daily_purchases");
    };
    // Synthetic, far-past dates — see `trial_conversion_counts_the_cohort...`
    // for why this is the only way to get an exact count on a shared database.
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    app.login_as(steam_id).await;
    let user_id = user_id_for(&app, steam_id).await;

    sqlx::query(
        "INSERT INTO events (user_id, type, metadata, created_at) VALUES
             ($1, 'voucher_redeemed', '{}', '2019-03-02T00:00:00Z'),
             ($1, 'purchase', '{}', '2019-03-02T00:00:00Z'),
             ($1, 'purchase', '{}', '2019-03-02T00:00:00Z'),
             ($1, 'voucher_redeemed', '{}', '2019-01-01T00:00:00Z')",
    )
    .bind(user_id)
    .execute(&app.db)
    .await
    .unwrap();

    let from = rfc3339("2019-03-01T00:00:00Z");
    let to = rfc3339("2019-03-03T00:00:00Z");

    let voucher_redemptions =
        dota_coach_backend::repositories::admin::count_voucher_redemptions(&app.db, from, to)
            .await
            .unwrap();
    assert_eq!(voucher_redemptions, 1, "the January redemption is outside the window");

    let series = dota_coach_backend::repositories::admin::daily_series(
        &app.db,
        chrono::NaiveDate::from_ymd_opt(2019, 3, 1).unwrap(),
        chrono::NaiveDate::from_ymd_opt(2019, 3, 3).unwrap(),
    )
    .await
    .unwrap();
    let march_2 = series.iter().find(|d| d.date.day() == 2).unwrap();
    assert_eq!(march_2.purchases, 2, "two separate charges, not one distinct user");

    app.cleanup(&[steam_id]).await;
}

// ---------------------------------------------------------------------------
// Admin: audit log
// ---------------------------------------------------------------------------

#[tokio::test]
async fn every_mutating_admin_action_writes_an_audit_entry() {
    let Some(db) = support::pool().await else {
        return skip("every_mutating_admin_action_writes_an_audit_entry");
    };
    let admin_steam_id = unique_steam_id();
    let target_steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let admin_session = app.login_as(admin_steam_id).await;
    make_admin(&app, admin_steam_id).await;
    app.login_as(target_steam_id).await;
    let admin_id = user_id_for(&app, admin_steam_id).await;
    let target_id = user_id_for(&app, target_steam_id).await;

    // `login_as` creates no subscription row; `extend` needs one to extend.
    sqlx::query(
        "INSERT INTO subscriptions (user_id, status, plan, trial_started_at, trial_ends_at)
         VALUES ($1, 'trialing', 'pro', now(), now() + interval '14 days')",
    )
    .bind(target_id)
    .execute(&app.db)
    .await
    .unwrap();

    // One of each mutating action, in order: extend, disable, enable,
    // create a voucher, deactivate it.
    assert_eq!(
        app.post_json(
            &format!("/api/admin/users/{target_id}/extend"),
            r#"{"days": 5}"#,
            Some(&admin_session.token),
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        app.post(
            &format!("/api/admin/users/{target_id}/disable"),
            Some(&admin_session.token),
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        app.post(
            &format!("/api/admin/users/{target_id}/enable"),
            Some(&admin_session.token),
        )
        .await
        .status,
        StatusCode::OK
    );
    let created = app
        .post_json(
            "/api/admin/vouchers",
            r#"{"duration_days": 7, "max_uses": 1}"#,
            Some(&admin_session.token),
        )
        .await;
    assert_eq!(created.status, StatusCode::OK);
    let voucher_id = created.json()["vouchers"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(
        app.post(
            &format!("/api/admin/vouchers/{voucher_id}/deactivate"),
            Some(&admin_session.token),
        )
        .await
        .status,
        StatusCode::OK
    );

    let rows: Vec<(String, String, uuid::Uuid, Option<uuid::Uuid>)> = sqlx::query_as(
        "SELECT action, target_type, target_id, admin_id
           FROM admin_audit_log
          WHERE admin_id = $1
          ORDER BY created_at",
    )
    .bind(admin_id)
    .fetch_all(&app.db)
    .await
    .unwrap();

    assert_eq!(
        rows,
        vec![
            ("extend_access".to_string(), "user".to_string(), target_id, Some(admin_id)),
            ("disable_user".to_string(), "user".to_string(), target_id, Some(admin_id)),
            ("enable_user".to_string(), "user".to_string(), target_id, Some(admin_id)),
            (
                "create_voucher".to_string(),
                "voucher".to_string(),
                voucher_id.parse().unwrap(),
                Some(admin_id)
            ),
            (
                "deactivate_voucher".to_string(),
                "voucher".to_string(),
                voucher_id.parse().unwrap(),
                Some(admin_id)
            ),
        ]
    );

    delete_voucher(&app, &voucher_id).await;
    app.cleanup(&[admin_steam_id, target_steam_id]).await;
}

async fn delete_voucher(app: &support::TestApp, voucher_id: &str) {
    sqlx::query("DELETE FROM vouchers WHERE id = $1::uuid")
        .bind(voucher_id)
        .execute(&app.db)
        .await
        .unwrap();
}

#[tokio::test]
async fn the_audit_log_lists_entries_newest_first_with_the_admins_name() {
    let Some(db) = support::pool().await else {
        return skip("the_audit_log_lists_entries_newest_first_with_the_admins_name");
    };
    let admin_steam_id = unique_steam_id();
    let target_steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());

    let admin_session = app.login_as(admin_steam_id).await;
    make_admin(&app, admin_steam_id).await;
    app.login_as(target_steam_id).await;
    let target_id = user_id_for(&app, target_steam_id).await;

    app.post(
        &format!("/api/admin/users/{target_id}/disable"),
        Some(&admin_session.token),
    )
    .await;
    app.post(
        &format!("/api/admin/users/{target_id}/enable"),
        Some(&admin_session.token),
    )
    .await;

    let response = app
        .get("/api/admin/audit-log?limit=2", Some(&admin_session.token))
        .await;
    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    let entries = body["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    // Newest first: enable was the more recent of the two actions.
    assert_eq!(entries[0]["action"], "enable_user");
    assert_eq!(entries[1]["action"], "disable_user");
    assert!(entries[0]["admin_persona_name"].is_string() || entries[0]["admin_persona_name"].is_null());

    app.cleanup(&[admin_steam_id, target_steam_id]).await;
}
