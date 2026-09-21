//! Coaching sessions: storage, immutability and isolation.
//!
//! These rules are the ones that fail silently. A session that can be updated
//! still renders as a page of numbers — just not the numbers that were
//! measured — and a progress chart built on rewritten history looks exactly
//! like a progress chart built on real history.

mod support;

use support::{app, skip, unique_steam_id, MockDota, StubVerifier};

use dota_coach_backend::domain::coaching_session::{MetricSnapshot, MetricUnit, SessionDraft};
use dota_coach_backend::domain::role::CoachableRole;
use dota_coach_backend::repositories::coaching_session;
use sqlx::PgPool;
use uuid::Uuid;

fn metric(key: &str, value: f32, unit: MetricUnit, higher_is_better: bool) -> MetricSnapshot {
    MetricSnapshot {
        key: key.to_string(),
        label: key.to_string(),
        value,
        sample: 20,
        unit,
        higher_is_better,
    }
}

fn draft(role: CoachableRole, performance: f32, match_ids: Vec<Uuid>) -> SessionDraft {
    SessionDraft {
        role,
        analyzed_match_count: match_ids.len() as i32,
        analyzed_match_ids: match_ids,
        newest_match_at: Some(chrono::Utc::now()),
        performance: Some(performance),
        metrics: vec![
            metric("role.performance", performance, MetricUnit::Score, true),
            metric("overall.deaths", 5.8, MetricUnit::Per10, false),
            metric("overall.gold_per_min", 512.0, MetricUnit::PerMinute, true),
        ],
        strengths: Vec::new(),
        weaknesses: Vec::new(),
        benchmarks: Vec::new(),
        heroes: Vec::new(),
        training_focus_id: None,
    }
}

/// A signed-in player with nothing synced. Sessions are inserted directly:
/// Phase 1 has no route that creates one.
async fn player(db: PgPool) -> (support::TestApp, support::Session, i64) {
    let steam_id = unique_steam_id();
    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let session = app.login_as(steam_id).await;
    (app, session, steam_id)
}

#[tokio::test]
async fn a_session_is_stored_with_its_numbers_intact() {
    let Some(db) = support::pool().await else {
        return skip("a_session_is_stored_with_its_numbers_intact");
    };
    let (app, session, steam_id) = player(db).await;

    let ids: Vec<Uuid> = (0..20).map(|_| Uuid::new_v4()).collect();
    let stored = coaching_session::insert(
        &app.db,
        session.dota_player_id,
        &draft(CoachableRole::Carry, 54.0, ids.clone()),
    )
    .await
    .unwrap();

    assert_eq!(stored.sequence, 1);
    assert_eq!(stored.role, CoachableRole::Carry);
    assert_eq!(stored.performance, Some(54.0));
    assert_eq!(stored.analyzed_match_count, 20);
    assert_eq!(stored.analyzed_match_ids, ids);
    assert_eq!(
        stored.analysis_id, None,
        "a session is measured, not written"
    );

    // The point of the whole table: these came back as numbers.
    let deaths = stored.metric("overall.deaths").unwrap();
    assert_eq!(deaths.value, 5.8);
    assert_eq!(deaths.unit, MetricUnit::Per10);
    assert!(!deaths.higher_is_better);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_stored_session_cannot_be_updated() {
    let Some(db) = support::pool().await else {
        return skip("a_stored_session_cannot_be_updated");
    };
    let (app, session, steam_id) = player(db).await;

    let stored = coaching_session::insert(
        &app.db,
        session.dota_player_id,
        &draft(CoachableRole::Carry, 54.0, vec![Uuid::new_v4()]),
    )
    .await
    .unwrap();

    // Rewriting a past performance score is exactly the thing that would make
    // a progress chart lie, so the database refuses it rather than trusting
    // every future caller to know better.
    let result = sqlx::query("UPDATE coaching_sessions SET performance = 99 WHERE id = $1")
        .bind(stored.id)
        .execute(&app.db)
        .await;

    let error = result.expect_err("a measured column must not be updatable");
    assert!(
        error.to_string().contains("immutable"),
        "unexpected error: {error}"
    );

    // And the stored value is untouched.
    let reread = coaching_session::find_owned(&app.db, stored.id, session.dota_player_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reread.performance, Some(54.0));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn an_analysis_may_be_attached_once_and_changes_nothing_else() {
    let Some(db) = support::pool().await else {
        return skip("an_analysis_may_be_attached_once_and_changes_nothing_else");
    };
    let (app, session, steam_id) = player(db).await;

    let stored = coaching_session::insert(
        &app.db,
        session.dota_player_id,
        &draft(CoachableRole::Carry, 54.0, vec![Uuid::new_v4()]),
    )
    .await
    .unwrap();

    // A real analysis row, so the foreign key holds.
    let analysis_id: Uuid = sqlx::query_scalar(
        "INSERT INTO coaching_analyses
             (dota_player_id, scope, context_hash, model, summary, evidence)
         VALUES ($1, 'role', 'hash-1', 'test-model', 'A summary.', '[]'::jsonb)
         RETURNING id",
    )
    .bind(session.dota_player_id)
    .fetch_one(&app.db)
    .await
    .unwrap();

    assert!(
        coaching_session::attach_analysis(&app.db, stored.id, session.dota_player_id, analysis_id)
            .await
            .unwrap(),
        "the first attach is the permitted transition",
    );

    let reread = coaching_session::find_owned(&app.db, stored.id, session.dota_player_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reread.analysis_id, Some(analysis_id));
    // Attaching prose must not have moved a measured number.
    assert_eq!(reread.performance, Some(54.0));
    assert_eq!(reread.metrics.len(), 3);

    // A second attach finds nothing to update rather than overwriting.
    let second: Uuid = sqlx::query_scalar(
        "INSERT INTO coaching_analyses
             (dota_player_id, scope, context_hash, model, summary, evidence)
         VALUES ($1, 'role', 'hash-2', 'test-model', 'Another.', '[]'::jsonb)
         RETURNING id",
    )
    .bind(session.dota_player_id)
    .fetch_one(&app.db)
    .await
    .unwrap();

    assert!(
        !coaching_session::attach_analysis(&app.db, stored.id, session.dota_player_id, second)
            .await
            .unwrap(),
        "a session's analysis is not replaceable",
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn an_update_that_smuggles_a_change_alongside_an_analysis_is_rejected() {
    let Some(db) = support::pool().await else {
        return skip("an_update_that_smuggles_a_change_alongside_an_analysis_is_rejected");
    };
    let (app, session, steam_id) = player(db).await;

    let stored = coaching_session::insert(
        &app.db,
        session.dota_player_id,
        &draft(CoachableRole::Carry, 54.0, vec![Uuid::new_v4()]),
    )
    .await
    .unwrap();

    let analysis_id: Uuid = sqlx::query_scalar(
        "INSERT INTO coaching_analyses
             (dota_player_id, scope, context_hash, model, summary, evidence)
         VALUES ($1, 'role', 'hash-3', 'test-model', 'A summary.', '[]'::jsonb)
         RETURNING id",
    )
    .bind(session.dota_player_id)
    .fetch_one(&app.db)
    .await
    .unwrap();

    // The attack the trigger exists for: "attach the AI summary" quietly
    // becoming "rewrite history".
    let result = sqlx::query(
        "UPDATE coaching_sessions SET analysis_id = $2, performance = 99 WHERE id = $1",
    )
    .bind(stored.id)
    .bind(analysis_id)
    .execute(&app.db)
    .await;

    let error = result.expect_err("attaching an analysis may not change a measured column");
    assert!(
        error.to_string().contains("immutable"),
        "unexpected error: {error}"
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn sessions_number_independently_per_role() {
    let Some(db) = support::pool().await else {
        return skip("sessions_number_independently_per_role");
    };
    let (app, session, steam_id) = player(db).await;

    for expected in 1..=3 {
        let stored = coaching_session::insert(
            &app.db,
            session.dota_player_id,
            &draft(
                CoachableRole::Carry,
                50.0 + expected as f32,
                vec![Uuid::new_v4()],
            ),
        )
        .await
        .unwrap();
        assert_eq!(stored.sequence, expected);
    }

    // A different role starts its own count. Carry session #3 and Mid session
    // #1 describe different datasets and must not share a numbering.
    let mid = coaching_session::insert(
        &app.db,
        session.dota_player_id,
        &draft(CoachableRole::Mid, 48.0, vec![Uuid::new_v4()]),
    )
    .await
    .unwrap();
    assert_eq!(mid.sequence, 1);

    assert_eq!(
        coaching_session::count(&app.db, session.dota_player_id, CoachableRole::Carry)
            .await
            .unwrap(),
        3
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn history_reads_newest_first_and_keeps_each_snapshot_verbatim() {
    let Some(db) = support::pool().await else {
        return skip("history_reads_newest_first_and_keeps_each_snapshot_verbatim");
    };
    let (app, session, steam_id) = player(db).await;

    for performance in [54.0, 57.0, 61.0] {
        coaching_session::insert(
            &app.db,
            session.dota_player_id,
            &draft(CoachableRole::Carry, performance, vec![Uuid::new_v4()]),
        )
        .await
        .unwrap();
    }

    let history =
        coaching_session::list(&app.db, session.dota_player_id, CoachableRole::Carry, 10, 0)
            .await
            .unwrap();

    assert_eq!(history.len(), 3);
    assert_eq!(history[0].performance, Some(61.0));
    // The whole product promise: session #1 still says 54 after the player
    // improved, rather than being rewritten with today's figure.
    assert_eq!(history[2].performance, Some(54.0));
    assert_eq!(history[2].sequence, 1);

    // "Current profile" is simply the newest row — no second record to drift.
    let current = coaching_session::latest(&app.db, session.dota_player_id, CoachableRole::Carry)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.performance, Some(61.0));
    assert_eq!(current.sequence, 3);

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_player_cannot_read_another_players_session() {
    let Some(db) = support::pool().await else {
        return skip("a_player_cannot_read_another_players_session");
    };
    let alice_steam = unique_steam_id();
    let bob_steam = unique_steam_id();

    let app = app(db, MockDota::default().into(), StubVerifier::rejecting());
    let alice = app.login_as(alice_steam).await;
    let bob = app.login_as(bob_steam).await;

    let alice_session = coaching_session::insert(
        &app.db,
        alice.dota_player_id,
        &draft(CoachableRole::Carry, 54.0, vec![Uuid::new_v4()]),
    )
    .await
    .unwrap();

    // Bob knows the id and asks for it directly. Ownership is in the WHERE, so
    // this is indistinguishable from an id that does not exist.
    assert!(
        coaching_session::find_owned(&app.db, alice_session.id, bob.dota_player_id)
            .await
            .unwrap()
            .is_none()
    );

    // Nor can he annotate it.
    let analysis_id: Uuid = sqlx::query_scalar(
        "INSERT INTO coaching_analyses
             (dota_player_id, scope, context_hash, model, summary, evidence)
         VALUES ($1, 'role', 'bob-hash', 'test-model', 'Mine now.', '[]'::jsonb)
         RETURNING id",
    )
    .bind(bob.dota_player_id)
    .fetch_one(&app.db)
    .await
    .unwrap();

    assert!(!coaching_session::attach_analysis(
        &app.db,
        alice_session.id,
        bob.dota_player_id,
        analysis_id
    )
    .await
    .unwrap());

    // And Bob's own history is empty.
    assert_eq!(
        coaching_session::count(&app.db, bob.dota_player_id, CoachableRole::Carry)
            .await
            .unwrap(),
        0
    );

    app.cleanup(&[alice_steam, bob_steam]).await;
}

#[tokio::test]
async fn a_roles_history_never_contains_another_roles_sessions() {
    let Some(db) = support::pool().await else {
        return skip("a_roles_history_never_contains_another_roles_sessions");
    };
    let (app, session, steam_id) = player(db).await;

    coaching_session::insert(
        &app.db,
        session.dota_player_id,
        &draft(CoachableRole::Carry, 61.0, vec![Uuid::new_v4()]),
    )
    .await
    .unwrap();
    coaching_session::insert(
        &app.db,
        session.dota_player_id,
        &draft(CoachableRole::HardSupport, 44.0, vec![Uuid::new_v4()]),
    )
    .await
    .unwrap();

    let carry =
        coaching_session::list(&app.db, session.dota_player_id, CoachableRole::Carry, 10, 0)
            .await
            .unwrap();

    assert_eq!(carry.len(), 1);
    assert!(carry.iter().all(|s| s.role == CoachableRole::Carry));
    assert_eq!(carry[0].performance, Some(61.0));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_session_survives_losing_the_focus_it_referenced() {
    let Some(db) = support::pool().await else {
        return skip("a_session_survives_losing_the_focus_it_referenced");
    };
    let (app, session, steam_id) = player(db).await;

    let focus_id: Uuid = sqlx::query_scalar(
        "INSERT INTO training_focus
             (dota_player_id, focus_key, title, why, source, measure,
              higher_is_better, baseline_value, target_value, score, status)
         VALUES ($1, 'benchmark.gold_per_min', 'Farm faster', 'Because.',
                 'benchmark', 'gold_per_min', true, 460, 520, 70, 'active')
         RETURNING id",
    )
    .bind(session.dota_player_id)
    .fetch_one(&app.db)
    .await
    .unwrap();

    let mut d = draft(CoachableRole::Carry, 54.0, vec![Uuid::new_v4()]);
    d.training_focus_id = Some(focus_id);
    let stored = coaching_session::insert(&app.db, session.dota_player_id, &d)
        .await
        .unwrap();
    assert_eq!(stored.training_focus_id, Some(focus_id));

    // Deleting a focus must not delete the history that mentioned it — hence
    // ON DELETE SET NULL rather than CASCADE.
    sqlx::query("DELETE FROM training_focus WHERE id = $1")
        .bind(focus_id)
        .execute(&app.db)
        .await
        .unwrap();

    let reread = coaching_session::find_owned(&app.db, stored.id, session.dota_player_id)
        .await
        .unwrap()
        .expect("the session outlives the focus");
    assert_eq!(reread.training_focus_id, None);
    assert_eq!(reread.performance, Some(54.0));

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_sessions_focus_cannot_be_repointed_at_another() {
    let Some(db) = support::pool().await else {
        return skip("a_sessions_focus_cannot_be_repointed_at_another");
    };
    let (app, session, steam_id) = player(db).await;

    let stored = coaching_session::insert(
        &app.db,
        session.dota_player_id,
        &draft(CoachableRole::Carry, 54.0, vec![Uuid::new_v4()]),
    )
    .await
    .unwrap();

    let focus_id: Uuid = sqlx::query_scalar(
        "INSERT INTO training_focus
             (dota_player_id, focus_key, title, why, source, measure,
              higher_is_better, baseline_value, target_value, score, status)
         VALUES ($1, 'benchmark.gold_per_min', 'Farm faster', 'Because.',
                 'benchmark', 'gold_per_min', true, 460, 520, 70, 'active')
         RETURNING id",
    )
    .bind(session.dota_player_id)
    .fetch_one(&app.db)
    .await
    .unwrap();

    // Clearing the reference is the foreign key's job and is allowed.
    // *Setting* one after the fact would rewrite what the session says the
    // player was working on at the time.
    let result = sqlx::query("UPDATE coaching_sessions SET training_focus_id = $2 WHERE id = $1")
        .bind(stored.id)
        .bind(focus_id)
        .execute(&app.db)
        .await;

    let error = result.expect_err("a focus reference is not repointable");
    assert!(
        error.to_string().contains("training focus"),
        "unexpected error: {error}"
    );

    app.cleanup(&[steam_id]).await;
}

#[tokio::test]
async fn a_session_with_nothing_measured_is_still_a_session() {
    let Some(db) = support::pool().await else {
        return skip("a_session_with_nothing_measured_is_still_a_session");
    };
    let (app, session, steam_id) = player(db).await;

    // A brand-new player: no score, no metrics, no matches. This must store
    // rather than error, because the alternative is that a first session can
    // only exist once a player is already good enough to measure.
    let empty = SessionDraft {
        role: CoachableRole::Carry,
        analyzed_match_count: 0,
        analyzed_match_ids: Vec::new(),
        newest_match_at: None,
        performance: None,
        metrics: Vec::new(),
        strengths: Vec::new(),
        weaknesses: Vec::new(),
        benchmarks: Vec::new(),
        heroes: Vec::new(),
        training_focus_id: None,
    };

    let stored = coaching_session::insert(&app.db, session.dota_player_id, &empty)
        .await
        .unwrap();

    assert_eq!(stored.performance, None);
    assert!(stored.metrics.is_empty());
    assert!(stored.analyzed_match_ids.is_empty());
    assert_eq!(stored.newest_match_at, None);

    app.cleanup(&[steam_id]).await;
}
