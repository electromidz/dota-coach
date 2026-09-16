//! The competitive match population, end to end.
//!
//! The rules under test are enforced in SQL, so a fake would prove nothing:
//! these seed real rows through the real sync path and then read them back
//! through the same repository functions the product uses.
//!
//! What they are here to prevent, in order of how expensive the mistake is:
//!
//!   1. a Turbo game reaching any competitive number at all;
//!   2. the hundred-match window being taken *before* filtering, which silently
//!      shrinks the analysis for exactly the players who play the most Turbo;
//!   3. one role's matches leaking into another role's coaching dataset.

mod support;

use dota_coach_backend::domain::eligibility::{self, ExclusionReason};
use dota_coach_backend::domain::r#match::NormalizedMatch;
use dota_coach_backend::domain::role::CoachableRole;
use dota_coach_backend::domain::scope::MatchScope;
use dota_coach_backend::repositories;
use dota_coach_backend::services::roles;
use sqlx::PgPool;
use support::{
    batch, seed_app, skip, test_config, Lane, ABILITY_DRAFT, PUBLIC_ALL_PICK, RANKED_ALL_PICK,
    TOURNAMENT, TURBO,
};
use uuid::Uuid;

/// Sync a crafted history and hand back the player it belongs to.
async fn seed(db: PgPool, matches: Vec<NormalizedMatch>) -> (PgPool, Uuid) {
    let (app, session) = seed_app(db.clone(), matches, 100).await;
    drop(app);
    (db, session.dota_player_id)
}

/// The modes of the matches behind a set of ids, so a leak is visible.
async fn modes_of(db: &PgPool, ids: &[Uuid]) -> Vec<(Option<i32>, Option<i32>)> {
    sqlx::query_as::<_, (Option<i32>, Option<i32>)>(
        "SELECT game_mode, lobby_type FROM matches WHERE id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(db)
    .await
    .unwrap()
}

async fn roles_of(db: &PgPool, ids: &[Uuid]) -> Vec<String> {
    sqlx::query_scalar("SELECT role FROM matches WHERE id = ANY($1)")
        .bind(ids)
        .fetch_all(db)
        .await
        .unwrap()
}

#[tokio::test]
async fn only_ranked_and_public_all_pick_reach_the_competitive_population() {
    let Some(db) = support::pool().await else {
        return skip("only_ranked_and_public_all_pick_reach_the_competitive_population");
    };

    let mut history = batch(1_000, 30, RANKED_ALL_PICK, Lane::Carry, 18);
    history.extend(batch(2_000, 20, PUBLIC_ALL_PICK, Lane::Carry, 10));
    history.extend(batch(3_000, 50, TURBO, Lane::Carry, 45));
    history.extend(batch(4_000, 7, ABILITY_DRAFT, Lane::Carry, 7));
    history.extend(batch(5_000, 3, TOURNAMENT, Lane::Carry, 3));

    let (db, player) = seed(db, history).await;

    let career = repositories::metrics::player_stats(&db, player)
        .await
        .unwrap();
    assert_eq!(career.matches, 110, "the whole history is still stored");

    let competitive =
        repositories::metrics::player_stats_scoped(&db, player, &MatchScope::competitive(100))
            .await
            .unwrap();
    assert_eq!(
        competitive.matches, 50,
        "30 ranked + 20 public All Pick, and nothing else",
    );

    // The Turbo games were the wins, so a leak would show up in the win rate.
    assert_eq!(competitive.wins, 28);

    let summary = eligibility::summarize(
        &repositories::metrics::mode_counts(&db, player)
            .await
            .unwrap(),
    );
    assert_eq!(summary.total_matches, 110);
    assert_eq!(summary.eligible_matches, 50);

    let reasons: Vec<(ExclusionReason, i64)> = summary
        .excluded
        .iter()
        .map(|g| (g.reason, g.matches))
        .collect();
    assert!(reasons.contains(&(ExclusionReason::Turbo, 50)));
    assert!(reasons.contains(&(ExclusionReason::OtherGameMode, 7)));
    assert!(reasons.contains(&(ExclusionReason::NonPublicLobby, 3)));
}

#[tokio::test]
async fn the_window_limit_applies_after_filtering_not_before() {
    let Some(db) = support::pool().await else {
        return skip("the_window_limit_applies_after_filtering_not_before");
    };

    // 150 matches: 100 eligible, then 50 Turbo played *after* them, so a
    // limit-then-filter implementation would take the 50 Turbo games plus the
    // 50 newest eligible ones and report half a window.
    let mut history = batch(1_000, 100, RANKED_ALL_PICK, Lane::Carry, 50);
    history.extend(batch(9_000, 50, TURBO, Lane::Carry, 50));

    let (db, player) = seed(db, history).await;

    let ids = repositories::metrics::scoped_match_ids(&db, player, &MatchScope::competitive(100))
        .await
        .unwrap();

    assert_eq!(
        ids.len(),
        100,
        "the window must be filled from eligible matches"
    );
    for (game_mode, _) in modes_of(&db, &ids).await {
        assert_ne!(game_mode, Some(23), "no Turbo match may enter the window");
    }
}

#[tokio::test]
async fn a_history_with_fewer_eligible_matches_than_the_window_uses_all_of_them() {
    let Some(db) = support::pool().await else {
        return skip("a_history_with_fewer_eligible_matches_than_the_window_uses_all_of_them");
    };

    // The spec's example: 100 raw, 60 Turbo, 40 eligible.
    let mut history = batch(1_000, 40, RANKED_ALL_PICK, Lane::Carry, 22);
    history.extend(batch(9_000, 60, TURBO, Lane::Carry, 60));

    let (db, player) = seed(db, history).await;

    let stats =
        repositories::metrics::player_stats_scoped(&db, player, &MatchScope::competitive(100))
            .await
            .unwrap();

    assert_eq!(
        stats.matches, 40,
        "all 40 eligible matches, and never topped up with Turbo",
    );
    assert_eq!(stats.wins, 22);
}

#[tokio::test]
async fn a_role_scope_admits_only_that_role_and_only_eligible_modes() {
    let Some(db) = support::pool().await else {
        return skip("a_role_scope_admits_only_that_role_and_only_eligible_modes");
    };

    let mut history = batch(1_000, 12, RANKED_ALL_PICK, Lane::Carry, 6);
    history.extend(batch(2_000, 10, RANKED_ALL_PICK, Lane::Support, 5));
    history.extend(batch(3_000, 8, PUBLIC_ALL_PICK, Lane::Mid, 4));
    // The two things that must never appear in Carry coaching.
    history.extend(batch(4_000, 9, TURBO, Lane::Carry, 9));
    history.extend(batch(5_000, 6, RANKED_ALL_PICK, Lane::UnclassifiedCore, 6));

    let (db, player) = seed(db, history).await;

    let ids = repositories::metrics::scoped_match_ids(
        &db,
        player,
        &MatchScope::for_role(CoachableRole::Carry, 100),
    )
    .await
    .unwrap();

    assert_eq!(ids.len(), 12, "only the eligible Carry games");

    for role in roles_of(&db, &ids).await {
        assert_eq!(role, "Carry", "another role leaked into the Carry dataset");
    }
    for (game_mode, lobby_type) in modes_of(&db, &ids).await {
        assert!(
            eligibility::is_eligible(game_mode, lobby_type),
            "an ineligible mode leaked into the Carry dataset",
        );
    }

    // And the hero rollup narrows with it, rather than carrying every hero the
    // player has ever touched into the role's analysis.
    let heroes = repositories::metrics::hero_stats_scoped(
        &db,
        player,
        &MatchScope::for_role(CoachableRole::Carry, 100),
        20,
    )
    .await
    .unwrap();
    assert_eq!(heroes.iter().map(|h| h.matches).sum::<i64>(), 12);
}

#[tokio::test]
async fn turbo_cannot_influence_the_role_recommendation() {
    let Some(db) = support::pool().await else {
        return skip("turbo_cannot_influence_the_role_recommendation");
    };

    // Support is the stronger role on eligible matches. Carry is padded with a
    // pile of won Turbo games, which is exactly the shape that would flip a
    // recommendation built on an unfiltered history.
    let mut history = batch(1_000, 20, RANKED_ALL_PICK, Lane::Support, 14);
    history.extend(batch(2_000, 20, RANKED_ALL_PICK, Lane::Carry, 6));
    history.extend(batch(3_000, 40, TURBO, Lane::Carry, 40));

    let (db, player) = seed(db, history).await;
    let weights = test_config().roles.score_weights;

    let competitive = roles::analyze(
        &repositories::metrics::role_totals(&db, player, &MatchScope::competitive(100))
            .await
            .unwrap(),
        weights,
    );

    assert_eq!(
        competitive.analyzed_matches, 40,
        "Turbo is not in the sample"
    );
    assert_eq!(
        competitive.recommendation.as_ref().unwrap().role,
        CoachableRole::SoftSupport,
    );

    // The same analysis over the unfiltered career is what the filter exists to
    // prevent: it sees the Turbo wins and recommends Carry.
    let career = roles::analyze(
        &repositories::metrics::role_totals(&db, player, &MatchScope::career())
            .await
            .unwrap(),
        weights,
    );
    assert_eq!(career.analyzed_matches, 80);
    assert_eq!(
        career.recommendation.as_ref().unwrap().role,
        CoachableRole::Carry,
        "the contrast is the point of the test: unfiltered, Turbo decides",
    );
}

#[tokio::test]
async fn unattributable_matches_are_counted_rather_than_assigned_a_lane() {
    let Some(db) = support::pool().await else {
        return skip("unattributable_matches_are_counted_rather_than_assigned_a_lane");
    };

    let mut history = batch(1_000, 10, RANKED_ALL_PICK, Lane::Carry, 5);
    history.extend(batch(2_000, 15, RANKED_ALL_PICK, Lane::UnclassifiedCore, 8));

    let (db, player) = seed(db, history).await;

    let analysis = roles::analyze(
        &repositories::metrics::role_totals(&db, player, &MatchScope::competitive(100))
            .await
            .unwrap(),
        test_config().roles.score_weights,
    );

    assert_eq!(analysis.analyzed_matches, 25);
    assert_eq!(
        analysis.unclassified_matches, 15,
        "an unparsed core match is not evidence about any one lane",
    );

    let carry = analysis
        .roles
        .iter()
        .find(|r| r.role == CoachableRole::Carry)
        .unwrap();
    assert_eq!(carry.matches, 10);
}

// ---------------------------------------------------------------------------
// The HTTP surface: what the dashboard actually reads
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_stats_endpoint_reads_only_eligible_matches_and_says_so() {
    let Some(db) = support::pool().await else {
        return skip("the_stats_endpoint_reads_only_eligible_matches_and_says_so");
    };

    let mut history = batch(1_000, 24, RANKED_ALL_PICK, Lane::Carry, 12);
    history.extend(batch(2_000, 16, RANKED_ALL_PICK, Lane::Support, 12));
    // Every Turbo game is a win, so any leak moves the win rate visibly.
    history.extend(batch(3_000, 30, TURBO, Lane::Carry, 30));
    history.extend(batch(4_000, 5, ABILITY_DRAFT, Lane::Mid, 5));

    let (app, session) = seed_app(db, history, 100).await;
    let body = app.get("/api/stats", Some(&session.token)).await.json();

    assert_eq!(body["overall"]["matches"], 40, "24 Carry + 16 Support");
    assert_eq!(body["overall"]["wins"], 24, "no Turbo win leaked in");

    assert_eq!(body["scope"]["population"], "ranked_public_all_pick");
    assert_eq!(body["scope"]["analyzed_matches"], 40);
    assert_eq!(body["scope"]["window_limit"], 100);
    assert_eq!(body["scope"]["confidence"], "moderate");

    // Every stored match is accounted for, not silently dropped.
    assert_eq!(body["eligibility"]["total_matches"], 75);
    assert_eq!(body["eligibility"]["eligible_matches"], 40);
    let excluded = body["eligibility"]["excluded"].as_array().unwrap();
    let turbo = excluded
        .iter()
        .find(|group| group["reason"] == "turbo")
        .expect("Turbo is reported by name");
    assert_eq!(turbo["matches"], 30);

    // Hero and role rollups come from the same population.
    let hero_matches: i64 = body["heroes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["matches"].as_i64().unwrap())
        .sum();
    assert_eq!(hero_matches, 40);
    assert_eq!(body["role_analysis"]["analyzed_matches"], 40);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn the_competitive_match_list_serves_the_same_population_as_the_statistics() {
    let Some(db) = support::pool().await else {
        return skip("the_competitive_match_list_serves_the_same_population_as_the_statistics");
    };

    let mut history = batch(1_000, 12, RANKED_ALL_PICK, Lane::Carry, 6);
    history.extend(batch(2_000, 8, TURBO, Lane::Carry, 8));

    let (app, session) = seed_app(db, history, 100).await;

    // The default list is the player's real history: their Turbo games are
    // theirs, and hiding them from their own match list would be odd.
    let all = app
        .get("/api/matches?limit=100", Some(&session.token))
        .await
        .json();
    assert_eq!(all["total"], 20);
    assert_eq!(all["scope"], "all");

    let competitive = app
        .get(
            "/api/matches?limit=100&scope=competitive",
            Some(&session.token),
        )
        .await
        .json();
    assert_eq!(competitive["total"], 12);
    assert_eq!(competitive["scope"], "competitive");
    for m in competitive["matches"].as_array().unwrap() {
        assert_ne!(m["game_mode"], 23, "a Turbo match reached the trend data");
    }

    // The number the dashboard charts and the number it prints must agree.
    let stats = app.get("/api/stats", Some(&session.token)).await.json();
    assert_eq!(stats["scope"]["analyzed_matches"], competitive["total"]);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn an_unknown_list_scope_is_rejected() {
    let Some(db) = support::pool().await else {
        return skip("an_unknown_list_scope_is_rejected");
    };

    let (app, session) = seed_app(db, batch(1_000, 3, RANKED_ALL_PICK, Lane::Carry, 2), 100).await;

    let response = app
        .get("/api/matches?scope=ranked", Some(&session.token))
        .await;
    assert_eq!(response.status, 400, "{}", response.body);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn the_match_list_says_what_each_game_was_and_whether_coaching_reads_it() {
    let Some(db) = support::pool().await else {
        return skip("the_match_list_says_what_each_game_was_and_whether_coaching_reads_it");
    };

    let mut history = batch(1_000, 6, RANKED_ALL_PICK, Lane::Carry, 3);
    history.extend(batch(2_000, 4, PUBLIC_ALL_PICK, Lane::Carry, 2));
    history.extend(batch(3_000, 5, TURBO, Lane::Carry, 5));
    history.extend(batch(4_000, 2, TOURNAMENT, Lane::Carry, 1));

    let (app, session) = seed_app(db, history, 100).await;

    // The full history is still the full history — a player's Turbo games are
    // theirs to see — but each row now says what it is.
    let body = app
        .get("/api/matches?limit=100", Some(&session.token))
        .await
        .json();

    let rows = body["matches"].as_array().unwrap();
    assert_eq!(rows.len(), 17);

    let labelled = |label: &str| rows.iter().filter(|m| m["mode_label"] == label).count();
    assert_eq!(labelled("Ranked All Pick"), 6);
    assert_eq!(labelled("All Pick"), 4);
    assert_eq!(labelled("Turbo"), 5);
    assert_eq!(labelled("Private lobby"), 2);

    // And the flag reconciles the list with every analysis screen: ten of the
    // seventeen rows are what the dashboard counted.
    assert_eq!(rows.iter().filter(|m| m["eligible"] == true).count(), 10);
    for row in rows.iter().filter(|m| m["mode_label"] == "Turbo") {
        assert_eq!(row["eligible"], false);
    }

    let stats = app.get("/api/stats", Some(&session.token)).await.json();
    assert_eq!(stats["scope"]["analyzed_matches"], 10);

    app.cleanup(&[]).await;
}
