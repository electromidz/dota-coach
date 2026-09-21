//! Benchmarking inside the coaching context.
//!
//! The product asks for a Top 20% comparison against the player's Rank, Role
//! and Hero. Two of those three are available from the current provider —
//! hero, and rank via `/benchmarks?bracket=` — and the tests here are mostly
//! about saying which: a percentile whose peer group is quietly wider than the
//! one implied is worse than no percentile, because it looks like an answer.
//!
//! What *is* genuinely role-segmented is our own half of the comparison, and
//! that is asserted too — a carry's figures must not carry a support's games
//! into the distribution lookup.

mod support;

use support::{
    app_with, batch, seed_app, skip, test_config, Lane, MockDota, StubBenchmarks, StubVerifier,
    RANKED_ALL_PICK, TURBO,
};

/// Luna played in two roles, plus Turbo games that belong to neither.
fn luna_in_two_roles() -> Vec<dota_coach_backend::domain::r#match::NormalizedMatch> {
    let mut history = batch(1_000, 18, RANKED_ALL_PICK, Lane::Carry, 10);
    let mut supports = batch(2_000, 12, RANKED_ALL_PICK, Lane::Support, 6);
    let mut turbo = batch(3_000, 20, TURBO, Lane::Carry, 20);

    for m in history
        .iter_mut()
        .chain(supports.iter_mut())
        .chain(turbo.iter_mut())
    {
        m.hero_id = 35; // Luna throughout, so only the scope can separate them.
    }

    history.extend(supports);
    history.extend(turbo);
    history
}

#[tokio::test]
async fn the_benchmark_follows_the_role_being_coached() {
    let Some(db) = support::pool().await else {
        return skip("the_benchmark_follows_the_role_being_coached");
    };

    let (app, session) = seed_app(db, luna_in_two_roles(), 100).await;
    app.choose_role(&session, "carry").await;

    let body = app.get("/api/benchmark", Some(&session.token)).await.json();

    // 18 carry games on Luna. Not 30 (both roles), not 50 (with Turbo).
    assert_eq!(body["sample"], 18);
    assert_eq!(body["context"]["role"], "carry");
    assert_eq!(body["context"]["role_label"], "Carry");

    // The player's own half of the comparison says what it covers.
    let player_population = body["context"]["population"]["player"].as_str().unwrap();
    assert!(player_population.contains("Carry"), "{player_population}");
    assert!(
        player_population.contains("All Pick"),
        "{player_population}"
    );

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn the_comparison_states_which_dimensions_it_could_not_segment_on() {
    let Some(db) = support::pool().await else {
        return skip("the_comparison_states_which_dimensions_it_could_not_segment_on");
    };

    let (app, session) = seed_app(db, luna_in_two_roles(), 100).await;
    app.choose_role(&session, "carry").await;

    let body = app.get("/api/benchmark", Some(&session.token)).await.json();
    let context = &body["context"];

    // All four are asked for; the provider delivers hero and rank.
    let requested: Vec<&str> = context["requested"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert_eq!(
        requested,
        vec!["hero", "role", "rank_bracket", "patch"],
        "the spec's four dimensions are still what we ask for",
    );

    let segmented: Vec<&str> = context["segmented_by"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert_eq!(segmented, vec!["hero", "rank_bracket"]);

    // And each missing one is named, with a reason rather than a shrug.
    let unavailable: Vec<&str> = context["unavailable"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["segment"].as_str().unwrap())
        .collect();
    assert_eq!(unavailable, vec!["role", "patch"]);

    for entry in context["unavailable"].as_array().unwrap() {
        let reason = entry["reason"].as_str().unwrap();
        assert!(
            reason.len() > 20,
            "{} has no usable reason: {reason}",
            entry["segment"],
        );
    }

    // The provider reports rank_tier 55 — Legend — and the comparison says so
    // rather than leaving the reader to infer which peers these are.
    assert!(!context["rank_tier"].is_null());
    assert_eq!(context["bracket"]["used"], "legend");
    assert_eq!(context["bracket"]["label"], "Legend");
    assert_eq!(context["bracket"]["fell_back"], false);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn the_two_populations_are_never_claimed_to_match() {
    let Some(db) = support::pool().await else {
        return skip("the_two_populations_are_never_claimed_to_match");
    };

    let (app, session) = seed_app(db, luna_in_two_roles(), 100).await;
    app.choose_role(&session, "carry").await;

    let body = app.get("/api/benchmark", Some(&session.token)).await.json();
    let population = &body["context"]["population"];

    // Rank matches now, but the provider still does not publish which game
    // modes or patch its distribution covers, so the two populations are not
    // known to be the same one. Turning this true would need a source that says.
    assert_eq!(population["comparable"], false);
    assert!(population["peers"]
        .as_str()
        .unwrap()
        .contains("does not publish"));
    // The bracket it *does* cover is named, so "peers" is not a black box.
    assert!(
        population["peers"].as_str().unwrap().contains("Legend"),
        "{}",
        population["peers"],
    );
    assert!(!population["note"].as_str().unwrap().is_empty());

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn the_top_20_line_and_the_gap_to_it_are_reported() {
    let Some(db) = support::pool().await else {
        return skip("the_top_20_line_and_the_gap_to_it_are_reported");
    };

    let (app, session) = seed_app(db, luna_in_two_roles(), 100).await;
    app.choose_role(&session, "carry").await;

    let body = app.get("/api/benchmark", Some(&session.token)).await.json();
    let gpm = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["metric"] == "gold_per_min")
        .expect("gold per minute");

    // The stub's distribution: median 500, top 20% at 800.
    assert_eq!(gpm["peer_median"], 500.0);
    assert_eq!(gpm["top_20_value"], 800.0);
    assert!(!gpm["gap_to_top_20"].is_null());
    assert!(!gpm["percentile"].is_null(), "18 matches clears the floor");

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn an_unavailable_provider_withholds_percentiles_without_losing_the_caveats() {
    let Some(db) = support::pool().await else {
        return skip("an_unavailable_provider_withholds_percentiles_without_losing_the_caveats");
    };

    let app = app_with(
        db,
        MockDota::with_matches(luna_in_two_roles()),
        StubVerifier::rejecting(),
        StubBenchmarks::unavailable(),
        {
            let mut config = test_config();
            config.dota.sync_match_limit = 500;
            config
        },
    );
    let session = app.login_as(support::unique_steam_id()).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;
    app.choose_role(&session, "carry").await;

    let body = app.get("/api/benchmark", Some(&session.token)).await.json();

    assert!(body["note"].as_str().unwrap().contains("unavailable"));

    let gpm = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["metric"] == "gold_per_min")
        .expect("the player's own figure survives an outage");

    // The player's own number is local and still theirs to see; everything that
    // would need the provider is absent rather than invented.
    assert!(gpm["player_value"].as_f64().unwrap() > 0.0);
    assert!(gpm["peer_median"].is_null());
    assert!(gpm["top_20_value"].is_null());
    assert!(gpm["percentile"].is_null());

    // The caveats do not disappear when the provider does — which is exactly
    // when a reader most needs to know what they are looking at.
    assert_eq!(body["context"]["role"], "carry");
    assert_eq!(body["context"]["population"]["comparable"], false);
    assert_eq!(body["context"]["unavailable"].as_array().unwrap().len(), 4);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn the_role_filter_can_be_widened_or_named_explicitly() {
    let Some(db) = support::pool().await else {
        return skip("the_role_filter_can_be_widened_or_named_explicitly");
    };

    let (app, session) = seed_app(db, luna_in_two_roles(), 100).await;
    app.choose_role(&session, "carry").await;

    // Opting out of the role scope still excludes Turbo: game-mode eligibility
    // is not a preference.
    let all = app
        .get("/api/benchmark?role=all", Some(&session.token))
        .await
        .json();
    assert_eq!(all["sample"], 30, "18 carry + 12 support, no Turbo");
    assert!(all["context"]["role"].is_null());

    let support_only = app
        .get("/api/benchmark?role=soft_support", Some(&session.token))
        .await
        .json();
    assert_eq!(support_only["sample"], 12);
    assert_eq!(support_only["context"]["role"], "soft_support");

    let bad = app
        .get("/api/benchmark?role=jungle", Some(&session.token))
        .await;
    assert_eq!(bad.status, 400, "{}", bad.body);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_role_with_no_matches_says_so_rather_than_benchmarking_another_one() {
    let Some(db) = support::pool().await else {
        return skip("a_role_with_no_matches_says_so_rather_than_benchmarking_another_one");
    };

    let (app, session) =
        seed_app(db, batch(1_000, 20, RANKED_ALL_PICK, Lane::Carry, 10), 100).await;
    app.choose_role(&session, "hard_support").await;

    let body = app.get("/api/benchmark", Some(&session.token)).await.json();

    assert_eq!(body["sample"], 0);
    assert!(body["results"].as_array().unwrap().is_empty());
    assert!(
        body["note"]
            .as_str()
            .unwrap()
            .contains("No eligible Hard Support"),
        "{}",
        body["note"],
    );

    app.cleanup(&[]).await;
}

// ---------------------------------------------------------------------------
// Aiming at another rank bracket
// ---------------------------------------------------------------------------
//
// A player asking "what does Ancient look like?" is asking a progression
// question, and the only useful answer puts both rungs on one scale. The rules
// below are what keep that honest:
//
//   1. the target is *additive* — the player's own bracket, and every
//      percentile measured against it, must not move because the reader got
//      curious about Divine;
//   2. the numbers genuinely come from the target bracket's distribution;
//   3. a bracket the provider has nothing for is absent, never the all-ranks
//      figures wearing that bracket's name;
//   4. "clearing" a median honours direction, or a player would be told their
//      deaths are fine for being high.

/// The gold-per-minute row from a benchmark response.
fn gold(body: &serde_json::Value, key: &str) -> serde_json::Value {
    body[key]
        .as_array()
        .or_else(|| body[key]["metrics"].as_array())
        .expect("a metric list")
        .iter()
        .find(|r| r["metric"] == "gold_per_min")
        .expect("gold per minute")
        .clone()
}

#[tokio::test]
async fn the_default_target_is_the_next_rank_up() {
    let Some(db) = support::pool().await else {
        return skip("the_default_target_is_the_next_rank_up");
    };

    let (app, session) = seed_app(db, luna_in_two_roles(), 100).await;

    // The seeded player is Legend, so they arrive aimed at Ancient with no
    // interaction — the progression is the point of the page.
    let body = app.get("/api/benchmark", Some(&session.token)).await.json();

    assert_eq!(body["context"]["bracket"]["used"], "legend");
    assert_eq!(body["target"]["bracket"]["used"], "ancient");
    assert_eq!(body["target"]["label"], "Ancient");

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_target_is_added_beside_the_players_own_bracket_not_instead_of_it() {
    let Some(db) = support::pool().await else {
        return skip("a_target_is_added_beside_the_players_own_bracket_not_instead_of_it");
    };

    let (app, session) = seed_app(db, luna_in_two_roles(), 100).await;

    let alone = app
        .get("/api/benchmark?bracket=none", Some(&session.token))
        .await
        .json();
    let aiming = app
        .get("/api/benchmark?bracket=divine", Some(&session.token))
        .await
        .json();

    // Where the player actually stands is the same fact either way. This is
    // the whole rule: a percentile is about their peers, and their peers do
    // not change when they look upwards.
    assert!(alone["target"].is_null());
    assert_eq!(aiming["target"]["bracket"]["used"], "divine");
    assert_eq!(alone["context"]["bracket"]["used"], "legend");
    assert_eq!(aiming["context"]["bracket"]["used"], "legend");
    assert_eq!(alone["sample"], aiming["sample"]);

    let (before, after) = (gold(&alone, "results"), gold(&aiming, "results"));
    assert_eq!(before["player_value"], after["player_value"]);
    assert_eq!(before["percentile"], after["percentile"]);
    assert_eq!(before["peer_median"], after["peer_median"]);

    // And the target is a genuinely different distribution, not a relabelling.
    let target = gold(&aiming["target"], "metrics");
    assert_ne!(target["peer_median"], after["peer_median"]);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn the_gap_to_the_target_is_signed_by_direction_not_by_size() {
    let Some(db) = support::pool().await else {
        return skip("the_gap_to_the_target_is_signed_by_direction_not_by_size");
    };

    let (app, session) = seed_app(db, luna_in_two_roles(), 100).await;
    let body = app
        .get("/api/benchmark?bracket=ancient", Some(&session.token))
        .await
        .json();

    let target = &body["target"];
    let metrics = target["metrics"].as_array().unwrap();

    for metric in metrics {
        let Some(gap) = metric["gap_to_median"].as_f64() else {
            continue;
        };
        // Positive always means work to do, whichever way the metric runs.
        assert_eq!(
            metric["cleared"].as_bool().unwrap(),
            gap <= 0.0,
            "{} claims cleared={} on a gap of {gap}",
            metric["metric"],
            metric["cleared"],
        );
    }

    let cleared = target["metrics_cleared"].as_i64().unwrap();
    let compared = target["metrics_compared"].as_i64().unwrap();
    assert_eq!(
        cleared,
        metrics.iter().filter(|m| m["cleared"] == true).count() as i64,
        "the headline count is the rows it summarises",
    );
    assert!(compared > 0 && cleared <= compared);

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn a_bracket_with_no_data_is_left_out_rather_than_substituted() {
    let Some(db) = support::pool().await else {
        return skip("a_bracket_with_no_data_is_left_out_rather_than_substituted");
    };

    // The provider publishes nothing for Immortal on this hero — routine above
    // Divine, and the case where a silent substitution would be most tempting.
    let mut config = test_config();
    config.dota.sync_match_limit = 500;
    config.roles.analysis_match_limit = 100;

    let app = app_with(
        db,
        MockDota::with_matches(luna_in_two_roles()),
        StubVerifier::rejecting(),
        StubBenchmarks::without_bracket(dota_coach_backend::domain::hero::RankBracket::Immortal),
        config,
    );
    let session = app.login_as(support::unique_steam_id()).await;
    app.post("/api/players/me/sync", Some(&session.token)).await;

    let body = app
        .get("/api/benchmark?bracket=immortal", Some(&session.token))
        .await
        .json();

    // All-ranks medians under the heading "Immortal" would be the fabrication
    // this whole engine refuses. Absent is the honest answer.
    assert!(body["target"].is_null(), "{}", body["target"]);

    // The player's own comparison is untouched by the target's absence.
    assert_eq!(body["context"]["bracket"]["used"], "legend");
    assert!(!body["results"].as_array().unwrap().is_empty());

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn nobody_is_aimed_at_a_rank_that_does_not_exist() {
    let Some(db) = support::pool().await else {
        return skip("nobody_is_aimed_at_a_rank_that_does_not_exist");
    };

    let (app, session) = seed_app(db, luna_in_two_roles(), 100).await;

    // Explicitly opting out, and asking for the bracket already being used,
    // both mean "no second column" — the latter because a duplicate of your
    // own numbers is noise dressed as a comparison.
    for query in ["?bracket=none", "?bracket=legend"] {
        let body = app
            .get(&format!("/api/benchmark{query}"), Some(&session.token))
            .await
            .json();
        assert!(body["target"].is_null(), "{query}: {}", body["target"]);
        assert!(!body["results"].as_array().unwrap().is_empty());
    }

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn every_bracket_is_offered_with_the_players_own_marked() {
    let Some(db) = support::pool().await else {
        return skip("every_bracket_is_offered_with_the_players_own_marked");
    };

    let (app, session) = seed_app(db, luna_in_two_roles(), 100).await;
    let body = app.get("/api/benchmark", Some(&session.token)).await.json();

    let brackets = body["brackets"].as_array().unwrap();
    let slugs: Vec<&str> = brackets
        .iter()
        .map(|b| b["value"].as_str().unwrap())
        .collect();

    // Rank order, all eight, so the client needs no list of its own.
    assert_eq!(
        slugs,
        vec!["herald", "guardian", "crusader", "archon", "legend", "ancient", "divine", "immortal"],
    );

    let own: Vec<&str> = brackets
        .iter()
        .filter(|b| b["is_player_rank"] == true)
        .map(|b| b["value"].as_str().unwrap())
        .collect();
    assert_eq!(own, vec!["legend"], "exactly one bracket is the player's");

    app.cleanup(&[]).await;
}

#[tokio::test]
async fn an_unknown_bracket_is_rejected_rather_than_defaulted() {
    let Some(db) = support::pool().await else {
        return skip("an_unknown_bracket_is_rejected_rather_than_defaulted");
    };

    let (app, session) = seed_app(db, luna_in_two_roles(), 100).await;

    for slug in ["titan", "ancien", "9", "all"] {
        let response = app
            .get(
                &format!("/api/benchmark?bracket={slug}"),
                Some(&session.token),
            )
            .await;
        assert_eq!(
            response.status, 400,
            "'{slug}' must not quietly become the default target: {}",
            response.body,
        );
    }

    // The single-metric route takes the same parameter, and narrows both sides.
    let one = app
        .get(
            "/api/benchmark/gold_per_min?bracket=divine",
            Some(&session.token),
        )
        .await
        .json();
    assert_eq!(one["results"].as_array().unwrap().len(), 1);
    assert_eq!(one["target"]["metrics"].as_array().unwrap().len(), 1);
    assert_eq!(one["target"]["bracket"]["used"], "divine");

    app.cleanup(&[]).await;
}
