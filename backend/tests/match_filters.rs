//! Filtering and sorting the match list, end to end.
//!
//! The list is server-paginated, so every one of these rules lives in SQL and a
//! fake would prove nothing. What they are here to prevent:
//!
//!   1. a filter that narrows the *page* but not `total`, so the pager offers
//!      pages that do not exist;
//!   2. a sort applied after pagination, which reorders twenty rows and calls
//!      it an ordering of two hundred;
//!   3. a filter value the server does not understand being ignored, so the
//!      caller is handed a list it did not ask for and cannot tell apart;
//!   4. the filter options drifting away from the matches they filter.

mod support;

use axum::http::StatusCode;
use dota_coach_backend::domain::r#match::NormalizedMatch;
use support::{batch, seed_app, skip, Lane, RANKED_ALL_PICK, TURBO};

/// Luna as a carry and Lion as a mid, with known results and gold rates.
///
/// Deliberately lopsided: four Luna carries against three Lion mids, one win
/// among the mids, so every filter has a different answer and a query that
/// quietly ignored one would still be visibly wrong.
fn history() -> Vec<NormalizedMatch> {
    // 3 wins of 4 — `batch` wins the first `n` of the batch, so the loss is
    // the newest and poorest of them. Gold descends with the id, which makes
    // "newest" and "highest GPM" genuinely different orderings.
    let mut carries = batch(1_000, 4, RANKED_ALL_PICK, Lane::Carry, 3);
    for (i, m) in carries.iter_mut().enumerate() {
        m.hero_id = 35;
        m.gpm = 700 - (i as i32) * 100;
    }

    // 1 win of 3, and the whole batch is newer than the carries.
    let mut mids = batch(2_000, 3, RANKED_ALL_PICK, Lane::Mid, 1);
    for m in mids.iter_mut() {
        m.hero_id = 26;
        m.gpm = 250;
    }

    // One Turbo game, so the scope and the filters can be told apart.
    let mut turbo = batch(3_000, 1, TURBO, Lane::Carry, 1);
    for m in turbo.iter_mut() {
        m.hero_id = 35;
        m.gpm = 999;
    }

    carries.into_iter().chain(mids).chain(turbo).collect()
}

#[tokio::test]
async fn each_filter_narrows_the_total_and_not_only_the_page() {
    let Some(db) = support::pool().await else {
        return skip("each_filter_narrows_the_total_and_not_only_the_page");
    };
    let (app, session) = seed_app(db, history(), 100).await;

    // Eight stored matches: 5 on Luna (4 ranked + 1 Turbo), 3 on Lion.
    let all = app.get("/api/matches", Some(&session.token)).await.json();
    assert_eq!(all["total"], 8);
    assert_eq!(all["filtered"], false);
    assert_eq!(all["sort"], "newest");

    let by_hero = app
        .get("/api/matches?hero_id=26", Some(&session.token))
        .await
        .json();
    assert_eq!(by_hero["total"], 3, "every Lion game, not just this page");
    assert_eq!(by_hero["filtered"], true);
    assert!(by_hero["matches"]
        .as_array()
        .unwrap()
        .iter()
        .all(|m| m["hero_id"] == 26));

    let by_role = app
        .get("/api/matches?role=carry", Some(&session.token))
        .await
        .json();
    assert_eq!(by_role["total"], 5);

    let losses = app
        .get("/api/matches?result=loss", Some(&session.token))
        .await
        .json();
    // 1 carry loss + 2 mid losses. The Turbo game was a win.
    assert_eq!(losses["total"], 3);
    assert!(losses["matches"]
        .as_array()
        .unwrap()
        .iter()
        .all(|m| m["won"] == false));

    let wins = app
        .get("/api/matches?result=win", Some(&session.token))
        .await
        .json();
    assert_eq!(wins["total"], 5);
}

#[tokio::test]
async fn filters_combine_rather_than_replacing_one_another() {
    let Some(db) = support::pool().await else {
        return skip("filters_combine_rather_than_replacing_one_another");
    };
    let (app, session) = seed_app(db, history(), 100).await;

    // Mid losses: two of the three Lion games.
    let role_and_result = app
        .get("/api/matches?role=mid&result=loss", Some(&session.token))
        .await
        .json();
    assert_eq!(role_and_result["total"], 2);

    // All three at once, and they agree: Lion is only ever played mid here.
    let all_three = app
        .get(
            "/api/matches?hero_id=26&role=mid&result=loss",
            Some(&session.token),
        )
        .await
        .json();
    assert_eq!(all_three["total"], 2);

    // A combination nothing satisfies is an empty page, not an error — Luna is
    // never played mid.
    let impossible = app
        .get("/api/matches?hero_id=35&role=mid", Some(&session.token))
        .await;
    assert_eq!(impossible.status, StatusCode::OK);
    let body = impossible.json();
    assert_eq!(body["total"], 0);
    assert!(body["matches"].as_array().unwrap().is_empty());
    assert_eq!(body["filtered"], true);
    // The options stay populated, so the client can offer a way back out.
    assert!(!body["filters"]["heroes"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn a_filter_applies_inside_the_scope_rather_than_widening_it() {
    let Some(db) = support::pool().await else {
        return skip("a_filter_applies_inside_the_scope_rather_than_widening_it");
    };
    let (app, session) = seed_app(db, history(), 100).await;

    // Five Luna games exist, but only four are Ranked or public All Pick. A
    // hero filter must not drag the Turbo one back in to find a fifth.
    let scoped = app
        .get(
            "/api/matches?scope=competitive&hero_id=35",
            Some(&session.token),
        )
        .await
        .json();

    assert_eq!(scoped["total"], 4);
    assert!(scoped["matches"]
        .as_array()
        .unwrap()
        .iter()
        .all(|m| m["eligible"] == true));

    // And the options offered inside that scope describe that scope.
    let heroes = scoped["filters"]["heroes"].as_array().unwrap();
    let luna = heroes.iter().find(|h| h["value"] == "35").unwrap();
    assert_eq!(
        luna["matches"], 4,
        "the Turbo game is not in this population"
    );
}

#[tokio::test]
async fn sorting_orders_the_whole_list_not_the_page() {
    let Some(db) = support::pool().await else {
        return skip("sorting_orders_the_whole_list_not_the_page");
    };
    let (app, session) = seed_app(db, history(), 100).await;

    // Page one of two. If the sort ran after pagination, the highest-GPM match
    // on the first page would be the best of the *newest three* — a mid game
    // at 250 — rather than the best of all eight.
    let by_gpm = app
        .get("/api/matches?sort=gpm_desc&limit=3", Some(&session.token))
        .await
        .json();
    assert_eq!(by_gpm["sort"], "gpm_desc");
    assert_eq!(by_gpm["matches"][0]["gpm"], 999);
    assert_eq!(by_gpm["matches"][1]["gpm"], 700);
    assert_eq!(by_gpm["matches"][2]["gpm"], 600);

    let lowest = app
        .get("/api/matches?sort=gpm_asc&limit=1", Some(&session.token))
        .await
        .json();
    assert_eq!(lowest["matches"][0]["gpm"], 250);

    // Oldest first is the reverse end of the default, not a reversed page.
    let newest = app
        .get("/api/matches?limit=1", Some(&session.token))
        .await
        .json();
    let oldest = app
        .get("/api/matches?sort=oldest&limit=1", Some(&session.token))
        .await
        .json();
    assert_ne!(
        newest["matches"][0]["match_id"],
        oldest["matches"][0]["match_id"]
    );
    assert_eq!(oldest["matches"][0]["match_id"], 1_000);
}

#[tokio::test]
async fn a_filter_and_a_sort_apply_together() {
    let Some(db) = support::pool().await else {
        return skip("a_filter_and_a_sort_apply_together");
    };
    let (app, session) = seed_app(db, history(), 100).await;

    // Carry wins, richest first: the Turbo win at 999, then the three ranked
    // carry wins. The 400 carry is the one loss and must not appear.
    let body = app
        .get(
            "/api/matches?role=carry&result=win&sort=gpm_desc",
            Some(&session.token),
        )
        .await
        .json();

    let gpms: Vec<i64> = body["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["gpm"].as_i64().unwrap())
        .collect();
    assert_eq!(gpms, vec![999, 700, 600, 500]);
    assert_eq!(body["total"], 4);
}

#[tokio::test]
async fn filter_options_come_from_the_players_own_matches() {
    let Some(db) = support::pool().await else {
        return skip("filter_options_come_from_the_players_own_matches");
    };
    let (app, session) = seed_app(db, history(), 100).await;

    let body = app.get("/api/matches", Some(&session.token)).await.json();
    let heroes = body["filters"]["heroes"].as_array().unwrap();
    let roles = body["filters"]["roles"].as_array().unwrap();

    // Exactly the two heroes played, most-played first, with real counts.
    assert_eq!(heroes.len(), 2);
    assert_eq!(heroes[0]["value"], "35");
    assert_eq!(heroes[0]["matches"], 5);
    assert_eq!(heroes[1]["value"], "26");
    assert_eq!(heroes[1]["matches"], 3);

    // Position order rather than count order, so the list does not reshuffle
    // as the history grows.
    let slugs: Vec<&str> = roles.iter().map(|r| r["value"].as_str().unwrap()).collect();
    assert_eq!(slugs, vec!["carry", "mid"]);
    assert_eq!(roles[0]["matches"], 5);
    assert_eq!(roles[1]["matches"], 3);
}

#[tokio::test]
async fn options_do_not_collapse_to_whatever_is_already_selected() {
    let Some(db) = support::pool().await else {
        return skip("options_do_not_collapse_to_whatever_is_already_selected");
    };
    let (app, session) = seed_app(db, history(), 100).await;

    // Filtering to Lion must still offer Luna, or the dropdown is a trap.
    let body = app
        .get("/api/matches?hero_id=26", Some(&session.token))
        .await
        .json();

    let heroes = body["filters"]["heroes"].as_array().unwrap();
    assert_eq!(heroes.len(), 2);
    assert!(heroes.iter().any(|h| h["value"] == "35"));
}

#[tokio::test]
async fn an_unusable_filter_value_is_rejected_rather_than_ignored() {
    let Some(db) = support::pool().await else {
        return skip("an_unusable_filter_value_is_rejected_rather_than_ignored");
    };
    let (app, session) = seed_app(db, history(), 100).await;

    for query in [
        "?role=jungle",
        "?role=roamer",
        "?result=draw",
        "?sort=best",
        "?hero_id=0",
        "?hero_id=-3",
    ] {
        let response = app
            .get(&format!("/api/matches{query}"), Some(&session.token))
            .await;

        assert_eq!(
            response.status,
            StatusCode::BAD_REQUEST,
            "{query} should be rejected, not silently dropped"
        );
        assert_eq!(response.error_code(), "BAD_REQUEST");
    }
}

#[tokio::test]
async fn the_unfiltered_list_is_unchanged() {
    let Some(db) = support::pool().await else {
        return skip("the_unfiltered_list_is_unchanged");
    };
    let (app, session) = seed_app(db, history(), 100).await;

    // Explicit `all` values mean the same thing as their absence: the filters
    // are additive, and their neutral position is the list that was always
    // there.
    let bare = app.get("/api/matches", Some(&session.token)).await.json();
    let neutral = app
        .get(
            "/api/matches?role=all&result=all&sort=newest",
            Some(&session.token),
        )
        .await
        .json();

    assert_eq!(bare["total"], neutral["total"]);
    assert_eq!(bare["filtered"], false);
    assert_eq!(neutral["filtered"], false);
    assert_eq!(bare["matches"][0]["id"], neutral["matches"][0]["id"]);
}

#[tokio::test]
async fn pagination_still_divides_the_filtered_list() {
    let Some(db) = support::pool().await else {
        return skip("pagination_still_divides_the_filtered_list");
    };
    let (app, session) = seed_app(db, history(), 100).await;

    let first = app
        .get(
            "/api/matches?role=carry&limit=2&page=1",
            Some(&session.token),
        )
        .await
        .json();
    assert_eq!(first["total"], 5);
    assert_eq!(first["total_pages"], 3);
    assert_eq!(first["matches"].as_array().unwrap().len(), 2);

    let last = app
        .get(
            "/api/matches?role=carry&limit=2&page=3",
            Some(&session.token),
        )
        .await
        .json();
    assert_eq!(last["matches"].as_array().unwrap().len(), 1);

    // No page holds a row from another one.
    assert_ne!(first["matches"][0]["id"], last["matches"][0]["id"]);
}
