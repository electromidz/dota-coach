use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::role::CoachableRole;
use crate::domain::training::{FocusMeasure, FocusSource, FocusStatus, TrainingFocus};
use crate::services::training;

/// A macro rather than a `const`: sqlx needs a `&'static str` query, and
/// `concat!` only composes literals.
macro_rules! columns {
    () => {
        "id, focus_key, title, why, source, measure, pattern_id, higher_is_better,
         baseline_value, target_value, score, status, started_at, ended_at"
    };
}

/// The focus the player is currently working on **in one role**.
///
/// Scoped because the focus is derived from role-scoped evidence: a Carry
/// focus selected from Carry matches would be nonsense advice for a player
/// working on Hard Support, and switching roles must not cost a player the
/// goal they had in the role they will come back to.
pub async fn active(
    pool: &PgPool,
    dota_player_id: Uuid,
    role: Option<CoachableRole>,
) -> Result<Option<TrainingFocus>, sqlx::Error> {
    let row = sqlx::query_as::<_, Row>(concat!(
        "SELECT ",
        columns!(),
        " FROM training_focus
          WHERE dota_player_id = $1
            AND role IS NOT DISTINCT FROM $2
            AND status = 'active'"
    ))
    .bind(dota_player_id)
    .bind(role.map(CoachableRole::slug))
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(Row::into_domain))
}

/// Everything the player has worked on in this role, newest first.
pub async fn history(
    pool: &PgPool,
    dota_player_id: Uuid,
    role: Option<CoachableRole>,
    limit: i64,
) -> Result<Vec<TrainingFocus>, sqlx::Error> {
    let rows = sqlx::query_as::<_, Row>(concat!(
        "SELECT ",
        columns!(),
        " FROM training_focus
          WHERE dota_player_id = $1
            AND role IS NOT DISTINCT FROM $2
          ORDER BY started_at DESC
          LIMIT $3"
    ))
    .bind(dota_player_id)
    .bind(role.map(CoachableRole::slug))
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().filter_map(Row::into_domain).collect())
}

/// Start a focus, retiring whatever was active.
///
/// One statement each, in a transaction: the partial unique index allows
/// exactly one active row per player, so closing the old one and opening the
/// new one cannot be two separate decisions.
/// Returns the stored id and start time — the database clock, not the
/// caller's, so the value a client sees is the value that was written.
pub async fn start(
    pool: &PgPool,
    dota_player_id: Uuid,
    role: Option<CoachableRole>,
    focus: &TrainingFocus,
) -> Result<(Uuid, DateTime<Utc>), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Only this role's active focus is retired. Another role's goal is not
    // finished merely because the player is working on something else today.
    sqlx::query(
        "UPDATE training_focus
            SET status     = 'retired',
                ended_at   = COALESCE(ended_at, now()),
                updated_at = now()
          WHERE dota_player_id = $1
            AND role IS NOT DISTINCT FROM $2
            AND status = 'active'",
    )
    .bind(dota_player_id)
    .bind(role.map(CoachableRole::slug))
    .execute(&mut *tx)
    .await?;

    let (id, started_at): (Uuid, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO training_focus
             (dota_player_id, role, focus_key, title, why, source, measure, pattern_id,
              higher_is_better, baseline_value, target_value, score, status, started_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'active', now())
         RETURNING id, started_at",
    )
    .bind(dota_player_id)
    .bind(role.map(CoachableRole::slug))
    .bind(&focus.key)
    .bind(&focus.title)
    .bind(&focus.why)
    .bind(focus.source.slug())
    .bind(focus.measure.slug())
    .bind(&focus.pattern_id)
    .bind(focus.higher_is_better)
    .bind(focus.baseline_value)
    .bind(focus.target_value)
    .bind(focus.score)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok((id, started_at))
}

/// Close the active focus, either way.
pub async fn close(
    pool: &PgPool,
    dota_player_id: Uuid,
    role: Option<CoachableRole>,
    status: FocusStatus,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE training_focus
            SET status     = $3,
                ended_at   = now(),
                updated_at = now()
          WHERE dota_player_id = $1
            AND role IS NOT DISTINCT FROM $2
            AND status = 'active'",
    )
    .bind(dota_player_id)
    .bind(role.map(CoachableRole::slug))
    .bind(status.slug())
    .execute(pool)
    .await?;

    Ok(())
}

#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    focus_key: String,
    title: String,
    why: String,
    source: String,
    measure: String,
    pattern_id: Option<String>,
    higher_is_better: bool,
    baseline_value: f32,
    target_value: f32,
    score: f32,
    status: String,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
}

impl Row {
    /// Rehydrate a stored focus.
    ///
    /// The live figures — current value, progress, whether the target is met —
    /// are deliberately left empty: they are functions of the history as it
    /// stands now, and a stored copy would be a second, staler answer. The
    /// handler fills them in against the current matches.
    fn into_domain(self) -> Option<TrainingFocus> {
        // A row whose enums no longer parse came from a definition this build
        // does not have. Dropping it beats rendering a focus nobody can
        // explain.
        let source = FocusSource::parse(&self.source)?;
        let measure = FocusMeasure::parse(&self.measure)?;
        let status = FocusStatus::parse(&self.status)?;

        Some(TrainingFocus {
            id: Some(self.id),
            key: self.focus_key,
            title: self.title,
            why: self.why,
            source,
            measure,
            measure_label: measure.label(),
            pattern_id: self.pattern_id,
            higher_is_better: self.higher_is_better,
            baseline_value: self.baseline_value,
            target_value: self.target_value,
            current_value: None,
            progress: None,
            target_met: false,
            status,
            status_label: status.label(),
            score: self.score,
            score_parts: Vec::new(),
            confidence: training::confidence_for(0),
            sample: 0,
            started_at: Some(self.started_at),
            ended_at: self.ended_at,
        })
    }
}
