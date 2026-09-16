//! Choosing a role to be coached on.
//!
//! One rule matters more than the rest and is asserted from several angles:
//! **the recommendation is advice, and the player's choice is the scope.** A
//! system that quietly coached the role it preferred would be a worse product
//! than one that gave no advice at all, because the user would have no way to
//! tell it was happening.

mod support;

use support::{batch, seed_app, skip, Lane, RANKED_ALL_PICK, TURBO};

/// A history where Soft Support is measurably the stronger role.
///
/// Carry is padded with won Turbo games, which is the shape that would flip a
/// recommendation built on an unfiltered history.
fn support_is_stronger() -> Vec<dota_coach_backend::domain::r#match::NormalizedMatch> {
    let mut history = batch(1_000, 20, RANKED_ALL_PICK, Lane::Support, 14);
    history.extend(batch(2_000, 20, RANKED_ALL_PICK, Lane::Carry, 6));
    history.extend(batch(3_000, 30, TURBO, Lane::Carry, 30));
    history
}

#[tokio::test]
async fn the_selection_screen_offers_every_role_and_advises_one() {
    let Some(db) = support::pool().await else {
        return skip("the_selection_screen_offers_every_role_and_advises_one");
    };

    let (app, session) = seed_app(db, support_is_stronger(), 100).await;
    let body = app
        .get("/api/coach/roles", Some(&session.token))
        .await
        .json();

    // Nothing has been chosen yet.
    assert!(body["profile"].is_null());

    // All five are selectable, including roles with no matches behind them: a
    // player is allowed to want to get better at something they do not play.
    let selectable: Vec<&str> = body["selectable_roles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["role"].as_str().unwrap())
        .collect();
    assert_eq!(
        selectable,
        vec!["carry", "mid", "offlane", "soft_support", "hard_support"],
    );

    // The advice is measured, and Turbo had no part in it.
    assert_eq!(body["analysis"]["recommendation"]["role"], "soft_support");
    assert_eq!(body["analysis"]["analyzed_matches"], 40);
    assert_eq!(body["scope"]["population"], "ranked_public_all_pick");

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn the_player_may_choose_against_the_recommendation_and_that_choice_is_the_scope() {
    let Some(db) = support::pool().await else {
        return skip(
            "the_player_may_choose_against_the_recommendation_and_that_choice_is_the_scope",
        );
    };

    let (app, session) = seed_app(db, support_is_stronger(), 100).await;

    let advised = app
        .get("/api/coach/roles", Some(&session.token))
        .await
        .json();
    assert_eq!(
        advised["analysis"]["recommendation"]["role"],
        "soft_support"
    );

    // "Play Carry anyway."
    let chosen = app
        .post_json(
            "/api/coach/role",
            r#"{"role":"carry"}"#,
            Some(&session.token),
        )
        .await;
    assert_eq!(chosen.status, 200, "{}", chosen.body);

    let profile = &chosen.json()["profile"];
    assert_eq!(
        profile["selected_role"], "carry",
        "the player's choice is the coaching role",
    );
    assert_eq!(profile["recommended_role"], "soft_support");
    assert_eq!(
        profile["overrode_recommendation"], true,
        "the disagreement is recorded, not resolved in the system's favour",
    );
    assert_eq!(profile["analyzed_matches"], 40);

    // And it survives the round trip: a later read still says Carry, and the
    // recommendation still says Support.
    let reread = app
        .get("/api/coach/roles", Some(&session.token))
        .await
        .json();
    assert_eq!(reread["profile"]["selected_role"], "carry");
    assert_eq!(reread["analysis"]["recommendation"]["role"], "soft_support");

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn accepting_the_recommendation_is_not_recorded_as_an_override() {
    let Some(db) = support::pool().await else {
        return skip("accepting_the_recommendation_is_not_recorded_as_an_override");
    };

    let (app, session) = seed_app(db, support_is_stronger(), 100).await;

    let body = app
        .post_json(
            "/api/coach/role",
            r#"{"role":"soft_support"}"#,
            Some(&session.token),
        )
        .await
        .json();

    assert_eq!(body["profile"]["selected_role"], "soft_support");
    assert_eq!(body["profile"]["overrode_recommendation"], false);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn changing_role_replaces_the_choice_rather_than_accumulating_profiles() {
    let Some(db) = support::pool().await else {
        return skip("changing_role_replaces_the_choice_rather_than_accumulating_profiles");
    };

    let (app, session) = seed_app(db, support_is_stronger(), 100).await;

    let first = app
        .post_json(
            "/api/coach/role",
            r#"{"role":"carry"}"#,
            Some(&session.token),
        )
        .await
        .json();
    let second = app
        .post_json("/api/coach/role", r#"{"role":"mid"}"#, Some(&session.token))
        .await
        .json();

    assert_eq!(second["profile"]["selected_role"], "mid");
    assert_eq!(
        first["profile"]["id"], second["profile"]["id"],
        "one profile per player, updated in place",
    );

    // A new role is a new stretch of work, so the clock restarts.
    assert_ne!(
        first["profile"]["selected_at"], second["profile"]["selected_at"],
        "selecting a different role restarts the baseline",
    );

    // Re-confirming the same role is not a new stretch, and must not reset it.
    let again = app
        .post_json("/api/coach/role", r#"{"role":"mid"}"#, Some(&session.token))
        .await
        .json();
    assert_eq!(
        again["profile"]["selected_at"], second["profile"]["selected_at"],
        "re-confirming the same role keeps the original baseline",
    );

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_role_that_cannot_be_coached_is_rejected() {
    let Some(db) = support::pool().await else {
        return skip("a_role_that_cannot_be_coached_is_rejected");
    };

    let (app, session) = seed_app(db, support_is_stronger(), 100).await;

    for body in [
        // `Core` is a real estimator label and deliberately not coachable: it
        // cannot say which lane, so it cannot scope a coaching dataset.
        r#"{"role":"core"}"#,
        r#"{"role":"jungle"}"#,
        r#"{"role":""}"#,
        r#"{"position":1}"#,
    ] {
        let response = app
            .post_json("/api/coach/role", body, Some(&session.token))
            .await;
        assert_eq!(
            response.status, 400,
            "{body} was accepted: {}",
            response.body
        );
    }

    // Nothing was stored by any of them.
    let after = app
        .get("/api/coach/roles", Some(&session.token))
        .await
        .json();
    assert!(after["profile"].is_null());

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_role_with_no_matches_may_still_be_chosen() {
    let Some(db) = support::pool().await else {
        return skip("a_role_with_no_matches_may_still_be_chosen");
    };

    // Nothing but Carry games in the history.
    let (app, session) = seed_app(db, batch(1_000, 15, RANKED_ALL_PICK, Lane::Carry, 8), 100).await;

    let body = app
        .post_json(
            "/api/coach/role",
            r#"{"role":"hard_support"}"#,
            Some(&session.token),
        )
        .await
        .json();

    // Accepted: "I want to learn this" is a legitimate coaching goal, and the
    // empty dataset is a fact for the coaching screen to report rather than a
    // reason to refuse the choice.
    assert_eq!(body["profile"]["selected_role"], "hard_support");
    assert_eq!(body["profile"]["recommended_role"], "carry");
    assert_eq!(body["profile"]["overrode_recommendation"], true);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn role_selection_requires_a_session() {
    let Some(db) = support::pool().await else {
        return skip("role_selection_requires_a_session");
    };

    let (app, _session) = seed_app(db, batch(1_000, 3, RANKED_ALL_PICK, Lane::Carry, 2), 100).await;

    assert_eq!(app.get("/api/coach/roles", None).await.status, 401);
    assert_eq!(
        app.post_json("/api/coach/role", r#"{"role":"carry"}"#, None)
            .await
            .status,
        401
    );

    app.cleanup(&[]).await;
}
