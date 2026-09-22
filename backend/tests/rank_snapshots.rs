//! The rank-history store, against a real database.
//!
//! These are SQL guarantees, not Rust ones, and every one of them is invisible
//! to a unit test: the daily collapse is enforced by a unique index on an
//! expression, the `since` bound by a `WHERE` clause, and the cleanup by a
//! foreign key. A fake would assert that the code I wrote does what I wrote.

mod support;

use chrono::{Duration, Utc};
use dota_coach_backend::repositories;
use support::{app, skip, unique_steam_id, MockDota, StubVerifier};

/// The `dota_account_id` derived from a test SteamID64.
const STEAM_ID64_BASE: i64 = 76_561_197_960_265_728;

#[tokio::test]
async fn two_readings_on_the_same_day_collapse_to_the_later_one() {
    let Some(db) = support::pool().await else {
        return skip("two_readings_on_the_same_day_collapse_to_the_later_one");
    };
    let steam_id = unique_steam_id();
    let account_id = steam_id - STEAM_ID64_BASE;
    let app = app(
        db.clone(),
        MockDota::default().into(),
        StubVerifier::rejecting(),
    );
    app.login_as(steam_id).await;

    repositories::rank_snapshots::insert_snapshot(&db, account_id, Some(44), None)
        .await
        .unwrap();
    repositories::rank_snapshots::insert_snapshot(&db, account_id, Some(45), Some(166))
        .await
        .unwrap();

    let history = repositories::rank_snapshots::list_for_player(
        &db,
        account_id,
        Utc::now() - Duration::days(1),
    )
    .await
    .unwrap();

    assert_eq!(
        history.len(),
        1,
        "a player syncing six times in an afternoon gets one point, not six"
    );
    assert_eq!(history[0].rank_tier, Some(45), "the later reading wins");
    assert_eq!(history[0].leaderboard_rank, Some(166));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_null_rank_is_stored_rather_than_skipped() {
    let Some(db) = support::pool().await else {
        return skip("a_null_rank_is_stored_rather_than_skipped");
    };
    let steam_id = unique_steam_id();
    let account_id = steam_id - STEAM_ID64_BASE;
    let app = app(
        db.clone(),
        MockDota::default().into(),
        StubVerifier::rejecting(),
    );
    app.login_as(steam_id).await;

    // A private profile reports no medal. Recording "we looked and there was
    // nothing" is what lets the trajectory break the line instead of drawing
    // a confident path across a stretch nobody observed.
    repositories::rank_snapshots::insert_snapshot(&db, account_id, None, None)
        .await
        .unwrap();

    let history = repositories::rank_snapshots::list_for_player(
        &db,
        account_id,
        Utc::now() - Duration::days(1),
    )
    .await
    .unwrap();

    assert_eq!(history.len(), 1);
    assert_eq!(history[0].rank_tier, None);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn history_is_returned_oldest_first_and_bounded_by_since() {
    let Some(db) = support::pool().await else {
        return skip("history_is_returned_oldest_first_and_bounded_by_since");
    };
    let steam_id = unique_steam_id();
    let account_id = steam_id - STEAM_ID64_BASE;
    let app = app(
        db.clone(),
        MockDota::default().into(),
        StubVerifier::rejecting(),
    );
    app.login_as(steam_id).await;

    // Deliberately inserted newest-first, so passing would mean the query
    // sorts rather than that the rows happened to arrive in order.
    for (tier, days_ago) in [(46_i16, 1_i64), (45, 10), (44, 40), (43, 200)] {
        app.insert_rank_snapshot(
            account_id,
            Some(tier),
            Utc::now() - Duration::days(days_ago),
        )
        .await;
    }

    let recent = repositories::rank_snapshots::list_for_player(
        &db,
        account_id,
        Utc::now() - Duration::days(180),
    )
    .await
    .unwrap();

    assert_eq!(
        recent.iter().map(|s| s.rank_tier).collect::<Vec<_>>(),
        vec![Some(44), Some(45), Some(46)],
        "oldest first, and the 200-day-old reading is outside the window"
    );

    let everything = repositories::rank_snapshots::list_for_player(
        &db,
        account_id,
        Utc::now() - Duration::days(365),
    )
    .await
    .unwrap();

    assert_eq!(everything.len(), 4, "a wider window reaches further back");

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn one_players_history_is_never_another_players() {
    let Some(db) = support::pool().await else {
        return skip("one_players_history_is_never_another_players");
    };
    let mine = unique_steam_id();
    let theirs = unique_steam_id();
    let app = app(
        db.clone(),
        MockDota::default().into(),
        StubVerifier::rejecting(),
    );
    app.login_as(mine).await;
    app.login_as(theirs).await;

    app.insert_rank_snapshot(mine - STEAM_ID64_BASE, Some(45), Utc::now())
        .await;
    app.insert_rank_snapshot(theirs - STEAM_ID64_BASE, Some(70), Utc::now())
        .await;

    let history = repositories::rank_snapshots::list_for_player(
        &db,
        mine - STEAM_ID64_BASE,
        Utc::now() - Duration::days(1),
    )
    .await
    .unwrap();

    assert_eq!(history.len(), 1);
    assert_eq!(history[0].rank_tier, Some(45));

    app.cleanup(&[mine, theirs]).await;
}

/// Deleting the account takes its rank history with it — through a foreign key
/// that points at `dota_players.dota_account_id`, not at the `id` UUID every
/// other child table uses. Worth pinning: that key is the one deliberate
/// deviation in this schema, and a cascade is easy to lose when changing one.
#[tokio::test]
async fn deleting_an_account_removes_its_rank_history() {
    let Some(db) = support::pool().await else {
        return skip("deleting_an_account_removes_its_rank_history");
    };
    let steam_id = unique_steam_id();
    let account_id = steam_id - STEAM_ID64_BASE;
    let app = app(
        db.clone(),
        MockDota::default().into(),
        StubVerifier::rejecting(),
    );
    app.login_as(steam_id).await;

    app.insert_rank_snapshot(account_id, Some(45), Utc::now())
        .await;

    app.cleanup(&[steam_id]).await;

    let orphans: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM rank_snapshots WHERE dota_account_id = $1")
            .bind(account_id)
            .fetch_one(&db)
            .await
            .unwrap();

    assert_eq!(orphans, 0);
}
