//! The preliminary training focus: an early signal without a false conclusion.
//!
//! A real focus needs a measure that can be read back out of a single match, a
//! percentile the engine was willing to claim, and a gap worth training. Plenty
//! of honest situations satisfy none or only some of those, and the product's
//! answer used to be silence — a dead end for exactly the players with the
//! least data.
//!
//! These tests pin the line between "we have nothing to say" and "we are
//! guessing". In order of how expensive the mistake would be:
//!
//!   1. a percentile appearing for a sample the engine refused to rank;
//!   2. a mid-pack player being handed a manufactured weakness;
//!   3. a preliminary reading turning up beside, or instead of, a real focus;
//!   4. the note and the reading contradicting each other on screen.

mod support;

use dota_coach_backend::domain::r#match::NormalizedMatch;
use support::{batch, seed_app, skip, Lane, RANKED_ALL_PICK};

/// A carry who is fine at everything the engine can turn into a goal.
///
/// Gold is strong and deaths are low — the two benchmark metrics that map onto
/// a per-match measure — so no focus can be selected. XP is poor, and XP per
/// minute is *not* one of those two, which is precisely the gap this feature
/// fills: the old behaviour said nothing at all to this player.
fn strong_gold_weak_xp(count: i64) -> Vec<NormalizedMatch> {
    let mut history = batch(1_000, count, RANKED_ALL_PICK, Lane::Carry, count / 2);
    for m in history.iter_mut() {
        m.hero_id = 35;
        m.gpm = 850; // comfortably above the stub's 800 top-20% line
        m.xpm = 260; // below the stub's 10th percentile of 250-ish
        m.deaths = 2; // 0.05 per minute over a 40-minute game
    }
    history
}

#[tokio::test]
async fn a_measurable_weakness_outside_the_trainable_metrics_is_still_surfaced() {
    let Some(db) = support::pool().await else {
        return skip("a_measurable_weakness_outside_the_trainable_metrics_is_still_surfaced");
    };

    let (app, session) = seed_app(db, strong_gold_weak_xp(20), 100).await;
    app.choose_role(&session, "carry").await;

    let body = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();

    assert!(body["focus"].is_null(), "nothing clears the bar for a goal");

    let reading = &body["preliminary"];
    assert!(!reading.is_null(), "but something was measured: {body}");
    assert_eq!(reading["metric"], "xp_per_min");
    assert!(
        reading["percentile"].as_f64().unwrap() <= 45.0,
        "only a genuine gap is offered: {}",
        reading["percentile"],
    );
    assert_eq!(reading["confidence"], "adequate");
    assert_eq!(reading["player_sample"], 20);

    // The note and the reading are mutually exclusive: a card saying "here is
    // your weakest area" beside an alert saying "nothing stands out" would
    // contradict itself on screen.
    assert!(body["note"].is_null(), "{}", body["note"]);

    // No goal was set, so nothing claims progress toward one.
    assert!(body["progress"].is_null());
}

#[tokio::test]
async fn a_thin_sample_gets_a_median_comparison_and_no_percentile() {
    let Some(db) = support::pool().await else {
        return skip("a_thin_sample_gets_a_median_comparison_and_no_percentile");
    };

    // Four matches — below the five the engine needs before it will place a
    // player in any distribution at all.
    let (app, session) = seed_app(db, strong_gold_weak_xp(4), 100).await;
    app.choose_role(&session, "carry").await;

    let body = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();

    assert!(body["focus"].is_null());

    let reading = &body["preliminary"];
    assert!(!reading.is_null(), "{body}");
    assert_eq!(reading["confidence"], "insufficient");
    assert_eq!(reading["player_sample"], 4);

    // The rule this whole feature has to respect: the engine withheld a
    // percentile, so nothing downstream may invent one.
    assert!(
        reading["percentile"].is_null(),
        "a four-match average must never be ranked",
    );
    // What it can honestly show is the real published median beside the real
    // measured value.
    assert!(reading["peer_median"].as_f64().is_some());
    assert!(reading["player_value"].as_f64().is_some());
    assert!(
        reading["why"].as_str().unwrap().contains("not a ranking"),
        "{}",
        reading["why"],
    );
    assert!(
        reading["to_confirm"].as_str().unwrap().contains('5'),
        "it says what would change the answer: {}",
        reading["to_confirm"],
    );
}

#[tokio::test]
async fn a_player_who_is_fine_everywhere_is_told_so_rather_than_given_a_weakness() {
    let Some(db) = support::pool().await else {
        return skip("a_player_who_is_fine_everywhere_is_told_so_rather_than_given_a_weakness");
    };

    // Above the median on every metric the stub publishes.
    let mut history = batch(1_000, 20, RANKED_ALL_PICK, Lane::Carry, 12);
    for m in history.iter_mut() {
        m.hero_id = 35;
        m.gpm = 850;
        m.xpm = 900;
        m.deaths = 2;
    }

    let (app, session) = seed_app(db, history, 100).await;
    app.choose_role(&session, "carry").await;

    let body = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();

    assert!(body["focus"].is_null());
    assert!(
        body["preliminary"].is_null(),
        "a mid-pack player has nothing to work on, and inventing one would make \
         the feature a complaint generator: {}",
        body["preliminary"],
    );
    // And the honest empty state is still there for them.
    assert!(body["note"]
        .as_str()
        .unwrap()
        .contains("Nothing stands out"));
}

#[tokio::test]
async fn a_real_focus_is_never_accompanied_by_a_preliminary_one() {
    let Some(db) = support::pool().await else {
        return skip("a_real_focus_is_never_accompanied_by_a_preliminary_one");
    };

    // Poor gold: a benchmark metric that *is* per-match measurable, so it
    // becomes an actual goal with a baseline and a target.
    let mut history = batch(1_000, 20, RANKED_ALL_PICK, Lane::Carry, 8);
    for m in history.iter_mut() {
        m.hero_id = 35;
        m.gpm = 210;
        m.xpm = 260;
    }

    let (app, session) = seed_app(db, history, 100).await;
    app.choose_role(&session, "carry").await;

    let body = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();

    assert!(!body["focus"].is_null(), "{body}");
    assert!(
        body["preliminary"].is_null(),
        "one answer at a time — the whole point of a single focus",
    );
    assert!(body["note"].is_null());
    // The focus keeps everything that makes it checkable.
    assert!(body["focus"]["target_value"].as_f64().is_some());
    assert!(!body["focus"]["score_parts"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn no_eligible_matches_is_still_the_empty_state() {
    let Some(db) = support::pool().await else {
        return skip("no_eligible_matches_is_still_the_empty_state");
    };

    let (app, session) = seed_app(db, strong_gold_weak_xp(20), 100).await;
    // Coached on a role they have never played, so the scope is empty.
    app.choose_role(&session, "hard_support").await;

    let body = app
        .get("/api/coach/training-focus", Some(&session.token))
        .await
        .json();

    assert!(body["focus"].is_null());
    assert!(
        body["preliminary"].is_null(),
        "nothing was measured, so there is nothing to offer",
    );
    assert!(body["note"]
        .as_str()
        .unwrap()
        .contains("No eligible Hard Support matches"));
}
