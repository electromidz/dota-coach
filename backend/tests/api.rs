//! Phase 3 behaviour: authentication, identity scoping, synchronization.
//!
//! The Dota provider and Valve are stubbed; Postgres is real. Set
//! `DATABASE_URL` (or `TEST_DATABASE_URL`) to run these — without one they
//! print a skip notice rather than passing vacuously.

mod support;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use support::{
    app, app_with, app_with_config, sample_matches, skip, test_config, unique_steam_id, Failure,
    MockDota, StubBenchmarks, StubVerifier,
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
