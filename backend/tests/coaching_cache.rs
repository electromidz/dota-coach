//! The coaching cache, and the guarantees that make it safe.
//!
//! A cache that can serve last week's coaching is worse than no cache, because
//! a stale answer is indistinguishable from a correct one. These tests are
//! about the two properties that prevent it: the database stays the source of
//! truth, and a value whose inputs moved becomes unreachable rather than being
//! relied on for eviction.

mod support;

use support::{
    app_with_config, batch, seed_app, skip, test_config, Lane, MockDota, StubVerifier,
    RANKED_ALL_PICK,
};

use dota_coach_backend::domain::r#match::NormalizedMatch;
use sqlx::PgPool;
use uuid::Uuid;

fn carry_history(count: i64) -> Vec<NormalizedMatch> {
    let mut all = batch(1_000, count, RANKED_ALL_PICK, Lane::Carry, count / 2);
    for m in all.iter_mut() {
        m.hero_id = 35;
    }
    all
}

async fn cached_rows(db: &PgPool, player: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM coaching_cache WHERE dota_player_id = $1")
        .bind(player)
        .fetch_one(db)
        .await
        .unwrap()
}

async fn cached_key(db: &PgPool, player: Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT key FROM coaching_cache WHERE dota_player_id = $1")
        .bind(player)
        .fetch_optional(db)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_repeated_read_is_served_from_the_cache_and_says_the_same_thing() {
    let Some(db) = support::pool().await else {
        return skip("a_repeated_read_is_served_from_the_cache_and_says_the_same_thing");
    };

    let (app, session) = seed_app(db, carry_history(20), 100).await;
    app.choose_role(&session, "carry").await;

    let first = app.get("/api/coach", Some(&session.token)).await.json();
    assert_eq!(cached_rows(&app.db, session.dota_player_id).await, 1);

    let second = app.get("/api/coach", Some(&session.token)).await.json();

    // A cache that changes the answer is not a cache.
    assert_eq!(first["evidence"], second["evidence"]);
    assert_eq!(first["patterns"], second["patterns"]);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn dropping_the_cache_changes_nothing_but_speed() {
    let Some(db) = support::pool().await else {
        return skip("dropping_the_cache_changes_nothing_but_speed");
    };

    let (app, session) = seed_app(db, carry_history(20), 100).await;
    app.choose_role(&session, "carry").await;

    let before = app.get("/api/coach", Some(&session.token)).await.json();

    // The property the whole design rests on: everything in here is
    // reconstructable, so deleting it costs time and nothing else.
    // Scoped to this player: the suite shares one database, and an unscoped
    // delete would be wiping other tests' entries out from under them.
    sqlx::query("DELETE FROM coaching_cache WHERE dota_player_id = $1")
        .bind(session.dota_player_id)
        .execute(&app.db)
        .await
        .unwrap();
    assert_eq!(cached_rows(&app.db, session.dota_player_id).await, 0);

    let after = app.get("/api/coach", Some(&session.token)).await.json();

    assert_eq!(before["evidence"], after["evidence"]);
    assert_eq!(before["patterns"], after["patterns"]);
    // And it repopulated on the way past.
    assert_eq!(cached_rows(&app.db, session.dota_player_id).await, 1);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_poisoned_entry_is_never_served() {
    let Some(db) = support::pool().await else {
        return skip("a_poisoned_entry_is_never_served");
    };

    let (app, session) = seed_app(db, carry_history(20), 100).await;
    app.choose_role(&session, "carry").await;

    let real = app.get("/api/coach", Some(&session.token)).await.json();
    let key = cached_key(&app.db, session.dota_player_id)
        .await
        .expect("an entry was written");

    // Someone writes nonsense into the cache under a valid key.
    sqlx::query("UPDATE coaching_cache SET payload = $2 WHERE key = $1")
        .bind(&key)
        .bind(serde_json::json!({"evidence": "not a list", "patterns": []}))
        .execute(&app.db)
        .await
        .unwrap();

    let after = app.get("/api/coach", Some(&session.token)).await.json();

    // A value that no longer deserializes is a miss, not an error and not a
    // corrupted page.
    assert_eq!(after["evidence"], real["evidence"]);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_new_sync_makes_the_old_entry_unreachable() {
    let Some(db) = support::pool().await else {
        return skip("a_new_sync_makes_the_old_entry_unreachable");
    };

    let dota = MockDota::with_matches(carry_history(20));
    let mut config = test_config();
    config.dota.sync_match_limit = 500;
    config.roles.analysis_match_limit = 100;
    config.dota.sync_cooldown_seconds = 0;

    let app = app_with_config(db, dota.clone(), StubVerifier::rejecting(), config);
    let steam_id = support::unique_steam_id();
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;
    app.choose_role(&session, "carry").await;

    let before = app.get("/api/coach", Some(&session.token)).await.json();
    let first_key = cached_key(&app.db, session.dota_player_id).await.unwrap();

    // Ten more games. The player's numbers move, so the cached answer is now
    // wrong — and the mechanism that saves us is the key, not an eviction.
    let mut grown = carry_history(20);
    let mut extra = batch(9_000, 10, RANKED_ALL_PICK, Lane::Carry, 10);
    for m in extra.iter_mut() {
        m.hero_id = 35;
    }
    grown.extend(extra);
    dota.set_matches(grown);
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let after = app.get("/api/coach", Some(&session.token)).await.json();
    let second_key = cached_key(&app.db, session.dota_player_id).await.unwrap();

    assert_ne!(
        first_key, second_key,
        "the fingerprint must move with the data"
    );
    assert_ne!(
        before["evidence"], after["evidence"],
        "a stale entry must not be served after new matches land",
    );

    // The superseded row is pruned rather than accumulating one per sync.
    assert_eq!(cached_rows(&app.db, session.dota_player_id).await, 1);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn changing_role_never_serves_the_other_roles_evidence() {
    let Some(db) = support::pool().await else {
        return skip("changing_role_never_serves_the_other_roles_evidence");
    };

    let mut history = carry_history(20);
    let mut supports = batch(2_000, 18, RANKED_ALL_PICK, Lane::Support, 9);
    for m in supports.iter_mut() {
        m.hero_id = 35;
    }
    history.extend(supports);

    let (app, session) = seed_app(db, history, 100).await;

    app.choose_role(&session, "carry").await;
    let carry = app.get("/api/coach", Some(&session.token)).await.json();

    app.choose_role(&session, "soft_support").await;
    let support = app.get("/api/coach", Some(&session.token)).await.json();

    // The role is in the key, so this cannot collide even before the
    // fingerprint is considered. A support player served their carry evidence
    // would look exactly like a correct answer.
    assert_eq!(carry["role"], "carry");
    assert_eq!(support["role"], "soft_support");
    assert_ne!(carry["evidence"], support["evidence"]);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn one_players_cache_is_never_served_to_another() {
    let Some(db) = support::pool().await else {
        return skip("one_players_cache_is_never_served_to_another");
    };

    let bob_steam = support::unique_steam_id();
    let (app, alice) = seed_app(db, carry_history(20), 100).await;
    app.choose_role(&alice, "carry").await;
    app.get("/api/coach", Some(&alice.token)).await;

    let bob = app.login_as(bob_steam).await;
    // Bob has no matches and no role, so he gets the precondition rather than
    // Alice's cached answer.
    let response = app.get("/api/coach", Some(&bob.token)).await;
    assert_eq!(response.status, axum::http::StatusCode::CONFLICT);

    assert_eq!(cached_rows(&app.db, bob.dota_player_id).await, 0);
    assert_eq!(cached_rows(&app.db, alice.dota_player_id).await, 1);

    app.cleanup(&[bob_steam]).await;
}

#[tokio::test]
async fn caching_can_be_switched_off_without_changing_the_answer() {
    let Some(db) = support::pool().await else {
        return skip("caching_can_be_switched_off_without_changing_the_answer");
    };

    let mut config = test_config();
    config.roles.analysis_match_limit = 100;
    config.dota.sync_match_limit = 500;
    config.coach.cache_enabled = false;

    let app = app_with_config(
        db,
        MockDota::with_matches(carry_history(20)),
        StubVerifier::rejecting(),
        config,
    );
    let steam_id = support::unique_steam_id();
    let session = app.login_as(steam_id).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;
    app.choose_role(&session, "carry").await;

    let body = app.get("/api/coach", Some(&session.token)).await.json();

    assert!(!body["evidence"].as_array().unwrap().is_empty());
    // Nothing stored, and the page is unaffected.
    assert_eq!(cached_rows(&app.db, session.dota_player_id).await, 0);

    app.cleanup(&[steam_id]).await;
}
