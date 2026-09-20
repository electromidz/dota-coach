//! Persistence for coaching sessions.
//!
//! Insert-and-read only. There is no `update` here and there should never be
//! one: a session is a historical fact, and the database enforces that with a
//! trigger rather than trusting this module to remember.
//!
//! The single exception is [`attach_analysis`], which fills in the model's
//! reading after the fact. The trigger checks that it changes nothing else.

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::coaching_session::{
    BenchmarkSnapshot, CoachingSession, HeroSnapshot, MetricSnapshot, SessionDraft,
};
use crate::domain::player_model::PlayerTrait;
use crate::domain::role::CoachableRole;

/// Store a session, assigning the next sequence number for its role.
///
/// The sequence is allocated inside the same statement that inserts, so two
/// concurrent syncs cannot both read "the last one was 11" and then both write
/// 12. `UNIQUE (dota_player_id, role, sequence)` is the backstop if they
/// somehow do — the loser gets a constraint violation rather than a duplicate
/// session number.
pub async fn insert(
    pool: &PgPool,
    dota_player_id: Uuid,
    draft: &SessionDraft,
) -> Result<CoachingSession, sqlx::Error> {
    let row: Row = sqlx::query_as(
        "INSERT INTO coaching_sessions (
             dota_player_id, role, sequence,
             analyzed_match_count, analyzed_match_ids, newest_match_at,
             performance, metrics, strengths, weaknesses,
             benchmark_snapshot, hero_snapshot, training_focus_id
         )
         SELECT
             $1, $2,
             COALESCE(
                 (SELECT MAX(sequence) FROM coaching_sessions
                   WHERE dota_player_id = $1 AND role = $2),
                 0
             ) + 1,
             $3, $4, $5, $6, $7, $8, $9, $10, $11, $12
         RETURNING
             id, role, sequence, analyzed_match_count, analyzed_match_ids,
             newest_match_at, performance, metrics, strengths, weaknesses,
             benchmark_snapshot, hero_snapshot, training_focus_id, analysis_id,
             created_at",
    )
    .bind(dota_player_id)
    .bind(draft.role.slug())
    .bind(draft.analyzed_match_count)
    .bind(&draft.analyzed_match_ids)
    .bind(draft.newest_match_at)
    .bind(draft.performance)
    .bind(sqlx::types::Json(&draft.metrics))
    .bind(sqlx::types::Json(&draft.strengths))
    .bind(sqlx::types::Json(&draft.weaknesses))
    .bind(sqlx::types::Json(&draft.benchmarks))
    .bind(sqlx::types::Json(&draft.heroes))
    .bind(draft.training_focus_id)
    .fetch_one(pool)
    .await?;

    Ok(row.into_domain())
}

/// The newest session for a role — which is also the player's current profile.
///
/// There is no separate profile record on purpose: "where the player is now"
/// is the most recent snapshot, so the current and historical views are the
/// same shape and cannot drift apart.
pub async fn latest(
    pool: &PgPool,
    dota_player_id: Uuid,
    role: CoachableRole,
) -> Result<Option<CoachingSession>, sqlx::Error> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT id, role, sequence, analyzed_match_count, analyzed_match_ids,
                newest_match_at, performance, metrics, strengths, weaknesses,
                benchmark_snapshot, hero_snapshot, training_focus_id, analysis_id,
                created_at
           FROM coaching_sessions
          WHERE dota_player_id = $1 AND role = $2
          ORDER BY sequence DESC
          LIMIT 1",
    )
    .bind(dota_player_id)
    .bind(role.slug())
    .fetch_optional(pool)
    .await?;

    Ok(row.map(Row::into_domain))
}

/// One page of a role's history, newest first.
pub async fn list(
    pool: &PgPool,
    dota_player_id: Uuid,
    role: CoachableRole,
    limit: i64,
    offset: i64,
) -> Result<Vec<CoachingSession>, sqlx::Error> {
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, role, sequence, analyzed_match_count, analyzed_match_ids,
                newest_match_at, performance, metrics, strengths, weaknesses,
                benchmark_snapshot, hero_snapshot, training_focus_id, analysis_id,
                created_at
           FROM coaching_sessions
          WHERE dota_player_id = $1 AND role = $2
          ORDER BY sequence DESC
          LIMIT $3 OFFSET $4",
    )
    .bind(dota_player_id)
    .bind(role.slug())
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Row::into_domain).collect())
}

pub async fn count(
    pool: &PgPool,
    dota_player_id: Uuid,
    role: CoachableRole,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM coaching_sessions WHERE dota_player_id = $1 AND role = $2",
    )
    .bind(dota_player_id)
    .bind(role.slug())
    .fetch_one(pool)
    .await
}

/// One session, if it belongs to this player.
///
/// Ownership is in the `WHERE`, not a check after the fetch: there is no code
/// path that reads a session and then decides whether it was allowed to. A
/// session belonging to someone else is indistinguishable from one that does
/// not exist, which is the same rule `repositories::r#match::find_owned` holds.
pub async fn find_owned(
    pool: &PgPool,
    id: Uuid,
    dota_player_id: Uuid,
) -> Result<Option<CoachingSession>, sqlx::Error> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT id, role, sequence, analyzed_match_count, analyzed_match_ids,
                newest_match_at, performance, metrics, strengths, weaknesses,
                benchmark_snapshot, hero_snapshot, training_focus_id, analysis_id,
                created_at
           FROM coaching_sessions
          WHERE id = $1 AND dota_player_id = $2",
    )
    .bind(id)
    .bind(dota_player_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(Row::into_domain))
}

/// Record that a model has interpreted this session.
///
/// The only permitted mutation, and the database polices its scope: the
/// trigger rejects an update that changes any measured column, and rejects a
/// second attempt once `analysis_id` is set. Scoped to the owning player so a
/// session cannot be annotated by someone else's analysis.
///
/// Returns false when nothing was updated — wrong owner, or already attached.
pub async fn attach_analysis(
    pool: &PgPool,
    id: Uuid,
    dota_player_id: Uuid,
    analysis_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let affected = sqlx::query(
        "UPDATE coaching_sessions
            SET analysis_id = $3
          WHERE id = $1 AND dota_player_id = $2 AND analysis_id IS NULL",
    )
    .bind(id)
    .bind(dota_player_id)
    .bind(analysis_id)
    .execute(pool)
    .await?
    .rows_affected();

    Ok(affected == 1)
}

/// The stored row, before the role slug and JSONB columns are interpreted.
#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    role: String,
    sequence: i32,
    analyzed_match_count: i32,
    analyzed_match_ids: Vec<Uuid>,
    newest_match_at: Option<chrono::DateTime<chrono::Utc>>,
    performance: Option<f32>,
    metrics: sqlx::types::Json<Vec<MetricSnapshot>>,
    strengths: sqlx::types::Json<Vec<PlayerTrait>>,
    weaknesses: sqlx::types::Json<Vec<PlayerTrait>>,
    benchmark_snapshot: sqlx::types::Json<Vec<BenchmarkSnapshot>>,
    hero_snapshot: sqlx::types::Json<Vec<HeroSnapshot>>,
    training_focus_id: Option<Uuid>,
    analysis_id: Option<Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl Row {
    fn into_domain(self) -> CoachingSession {
        // A stored role that no longer parses would mean the enum changed
        // under a historical row. Falling back to Carry would silently refile
        // someone's Support history, so the raw slug decides and an unknown
        // one is impossible by construction — `slug()` wrote it.
        let role = CoachableRole::parse(&self.role).unwrap_or(CoachableRole::Carry);

        CoachingSession {
            id: self.id,
            role,
            role_label: role.label(),
            sequence: self.sequence,
            analyzed_match_count: self.analyzed_match_count,
            analyzed_match_ids: self.analyzed_match_ids,
            newest_match_at: self.newest_match_at,
            performance: self.performance,
            metrics: self.metrics.0,
            strengths: self.strengths.0,
            weaknesses: self.weaknesses.0,
            benchmarks: self.benchmark_snapshot.0,
            heroes: self.hero_snapshot.0,
            training_focus_id: self.training_focus_id,
            analysis_id: self.analysis_id,
            created_at: self.created_at,
        }
    }
}
