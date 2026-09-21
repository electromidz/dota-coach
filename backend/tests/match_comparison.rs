//! The match-detail comparison: this game against same-rank peers.
//!
//! The rules these tests exist to hold down are the ones that are invisible
//! when they break. A percentile claimed for a Turbo game, a trend that plots
//! a different hero, a comparison that 500s because a provider blinked — all
//! of them still render as a page full of numbers.

mod support;

use support::{
    app_with, seed_app, skip, test_config, unique_steam_id, Lane, MockDota, StubBenchmarks,
    StubVerifier, RANKED_ALL_PICK, TURBO,
};

use dota_coach_backend::domain::r#match::NormalizedMatch;

/// Luna on repeat, plus Turbo games on the same hero.
///
/// One hero throughout, so only eligibility and ordering can separate the
/// populations — a trend that quietly mixes them has nowhere to hide.
fn luna_history() -> Vec<NormalizedMatch> {
    let mut ranked = support::batch(1_000, 14, RANKED_ALL_PICK, Lane::Carry, 8);
    let mut turbo = support::batch(5_000, 3, TURBO, Lane::Carry, 2);

    for m in ranked.iter_mut().chain(turbo.iter_mut()) {
        m.hero_id = 35;
    }

    ranked.extend(turbo);
    ranked
}

/// The newest match of a given mode label, as the list reports it.
async fn newest_match_id(
    app: &support::TestApp,
    token: &str,
    mode_label: &str,
) -> Option<(String, serde_json::Value)> {
    let list = app.get("/api/matches?limit=100", Some(token)).await.json();

    list["matches"]
        .as_array()?
        .iter()
        .find(|m| m["mode_label"] == mode_label)
        .map(|m| (m["id"].as_str().unwrap().to_string(), m.clone()))
}

#[tokio::test]
async fn a_ranked_match_is_placed_against_the_players_own_bracket() {
    let Some(db) = support::pool().await else {
        return skip("a_ranked_match_is_placed_against_the_players_own_bracket");
    };

    let (app, session) = seed_app(db, luna_history(), 100).await;
    let (id, _) = newest_match_id(&app, &session.token, "Ranked All Pick")
        .await
        .expect("a ranked match was seeded");

    let body = app
        .get(
            &format!("/api/matches/{id}/comparison"),
            Some(&session.token),
        )
        .await
        .json();

    assert_eq!(body["comparable"], true);
    // The stubbed provider reports rank_tier 55 — Legend.
    assert_eq!(body["bracket"]["used"], "legend");
    assert_eq!(body["bracket"]["fell_back"], false);
    assert_eq!(body["context"]["segmented_by"][1], "rank_bracket");
    assert!(body["note"].is_null());

    // A single match gets a percentile. The benchmark engine's sample floor is
    // about estimating a player, and this is not that.
    let gpm = body["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["metric"] == "gold_per_min")
        .expect("gold_per_min row");
    assert!(!gpm["this_match"]["percentile"].is_null());
    assert!(!gpm["peer_median"].is_null());

    // And the average beside it still carries the confidence of its sample.
    assert!(!gpm["hero_average"]["confidence"].is_null());

    // The one-glance number, and what it is a median of.
    assert!(!body["standing"]["this_match"].is_null());
    assert!(body["standing"]["metrics_counted"].as_u64().unwrap() > 0);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_turbo_match_is_shown_but_never_given_a_percentile() {
    let Some(db) = support::pool().await else {
        return skip("a_turbo_match_is_shown_but_never_given_a_percentile");
    };

    let (app, session) = seed_app(db, luna_history(), 100).await;
    let (id, _) = newest_match_id(&app, &session.token, "Turbo")
        .await
        .expect("a Turbo match was seeded");

    let body = app
        .get(
            &format!("/api/matches/{id}/comparison"),
            Some(&session.token),
        )
        .await
        .json();

    assert_eq!(body["comparable"], false);
    assert!(
        body["note"].as_str().unwrap().contains("Turbo"),
        "{}",
        body["note"]
    );

    let gpm = body["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["metric"] == "gold_per_min")
        .expect("gold_per_min row");

    // The player's own figure is theirs to see...
    assert!(!gpm["this_match"]["value"].is_null());
    // ...but it is not placed in a distribution drawn from ranked pubs.
    assert!(gpm["this_match"]["percentile"].is_null());
    assert!(body["standing"]["this_match"].is_null());

    // Nothing is read out of an empty percentile map.
    assert_eq!(body["pros"].as_array().unwrap().len(), 0);
    assert_eq!(body["cons"].as_array().unwrap().len(), 0);
    assert!(body["suggestion"].is_null());

    // The hero average and the trend come from the eligible games, so they
    // survive: they are a different population from the match on screen.
    assert!(!gpm["hero_average"]["percentile"].is_null());
    assert!(!body["trend"].as_array().unwrap().is_empty());

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn the_trend_covers_this_hero_and_marks_the_match_being_viewed() {
    let Some(db) = support::pool().await else {
        return skip("the_trend_covers_this_hero_and_marks_the_match_being_viewed");
    };

    let (app, session) = seed_app(db, luna_history(), 100).await;
    let (id, _) = newest_match_id(&app, &session.token, "Ranked All Pick")
        .await
        .expect("a ranked match was seeded");

    let body = app
        .get(
            &format!("/api/matches/{id}/comparison"),
            Some(&session.token),
        )
        .await
        .json();

    let trend = body["trend"].as_array().unwrap();
    assert!(!trend.is_empty());
    // Ten is the window; fourteen ranked games were seeded.
    assert!(trend.len() <= 10, "{}", trend.len());

    // The viewed match is the newest ranked one, so it heads the window.
    assert_eq!(trend[0]["is_current"], true);
    assert_eq!(trend[0]["match_id"], id);
    assert_eq!(
        trend.iter().filter(|p| p["is_current"] == true).count(),
        1,
        "exactly one point is the match being viewed",
    );

    // Newest first, the same order the match list uses.
    let times: Vec<&str> = trend
        .iter()
        .map(|p| p["started_at"].as_str().unwrap())
        .collect();
    let mut sorted = times.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(times, sorted);

    // There are older games, so there is something to have improved from.
    assert!(!body["delta_vs_previous"].is_null());

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_provider_outage_degrades_the_comparison_rather_than_failing_it() {
    let Some(db) = support::pool().await else {
        return skip("a_provider_outage_degrades_the_comparison_rather_than_failing_it");
    };

    let mut config = test_config();
    config.dota.sync_match_limit = 500;
    config.roles.analysis_match_limit = 100;

    let app = app_with(
        db,
        MockDota::with_matches(luna_history()),
        StubVerifier::rejecting(),
        StubBenchmarks::unavailable(),
        config,
    );
    let session = app.login_as(unique_steam_id()).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let (id, _) = newest_match_id(&app, &session.token, "Ranked All Pick")
        .await
        .expect("a ranked match was seeded");

    let response = app
        .get(
            &format!("/api/matches/{id}/comparison"),
            Some(&session.token),
        )
        .await;

    // The match is local data. A peer provider being down is not a reason to
    // stop a player reading their own game.
    assert_eq!(response.status, axum::http::StatusCode::OK);

    let body = response.json();
    assert_eq!(body["comparable"], false);
    assert!(body["note"].as_str().unwrap().contains("unavailable"));

    let gpm = body["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["metric"] == "gold_per_min")
        .expect("gold_per_min row");
    assert!(!gpm["this_match"]["value"].is_null());
    assert!(gpm["this_match"]["percentile"].is_null());
    assert!(gpm["peer_median"].is_null());

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn another_players_match_is_not_found_rather_than_forbidden() {
    let Some(db) = support::pool().await else {
        return skip("another_players_match_is_not_found_rather_than_forbidden");
    };

    let bob_steam = unique_steam_id();
    let (app, alice) = seed_app(db, luna_history(), 100).await;
    let bob = app.login_as(bob_steam).await;

    let (id, _) = newest_match_id(&app, &alice.token, "Ranked All Pick")
        .await
        .expect("a ranked match was seeded");

    let response = app
        .get(&format!("/api/matches/{id}/comparison"), Some(&bob.token))
        .await;

    // 404, not 403: whether the id exists is not Bob's business — the same
    // rule `/api/matches/{id}` already holds.
    assert_eq!(response.status, axum::http::StatusCode::NOT_FOUND);
    assert_eq!(response.error_code(), "NOT_FOUND");

    app.cleanup(&[bob_steam]).await;
}
