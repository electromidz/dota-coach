use chrono::{DateTime, Utc};
use sqlx::{AssertSqlSafe, PgPool};
use uuid::Uuid;

use crate::domain::benchmark::Confidence;
use crate::domain::player_model::{
    AnalyzedMatch, ModelConfidence, PatternStatus, RecurringPattern,
};
use crate::domain::scope::MatchScope;
use crate::services::benchmarks::percentile;

/// Every match in a scope with its derived metrics, flattened for detection.
///
/// The whole scope, not a page of it: a pattern is a statement about all of it,
/// and paginating here would make the denominator depend on a page size.
///
/// The scope is what makes a pattern role-specific. "You die too often" read
/// across every role is a different, weaker claim than the same sentence read
/// across the thirty Carry games the player asked to be coached on — and the
/// support games that would otherwise dilute it are not evidence about carrying.
pub async fn history(
    pool: &PgPool,
    dota_player_id: Uuid,
    scope: &MatchScope,
) -> Result<Vec<AnalyzedMatch>, sqlx::Error> {
    sqlx::query_as::<_, AnalyzedMatch>(AssertSqlSafe(format!(
        "{cte}
         SELECT
             m.id                 AS match_id,
             m.started_at,
             m.hero_id,
             m.hero_name,
             m.role,
             m.won,
             m.duration_seconds,
             m.gpm,
             mm.deaths_per_10,
             mm.kill_participation,
             m.tower_damage,
             m.last_hits_at_10,
             m.gold_at_10,
             m.bkb_seconds
           FROM matches m
           JOIN match_metrics mm ON mm.match_id = m.id
           {join}
          WHERE m.dota_player_id = $1
          ORDER BY m.started_at DESC",
        cte = scope.cte(),
        join = scope.join(),
    )))
    .bind(dota_player_id)
    .fetch_all(pool)
    .await
}

pub async fn upsert_model(
    pool: &PgPool,
    dota_player_id: Uuid,
    model_version: i32,
    matches_analyzed: i64,
    confidence: ModelConfidence,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO player_models
             (dota_player_id, model_version, matches_analyzed, confidence, computed_at)
         VALUES ($1, $2, $3, $4, now())
         ON CONFLICT (dota_player_id) DO UPDATE
             SET model_version    = EXCLUDED.model_version,
                 matches_analyzed = EXCLUDED.matches_analyzed,
                 confidence       = EXCLUDED.confidence,
                 computed_at      = now()",
    )
    .bind(dota_player_id)
    .bind(model_version)
    .bind(matches_analyzed)
    .bind(confidence.slug())
    .execute(pool)
    .await?;

    Ok(())
}

/// Record the currently-detected patterns, and retire the ones that are gone.
///
/// Three things happen here that a plain overwrite would lose:
///
///   * `first_detected_at` is preserved across every later recomputation, so
///     "since March" survives;
///   * a pattern that has stopped clearing the threshold is marked resolved
///     rather than deleted — it is invisible in the data that resolved it, so
///     deleting it would erase the fact that the player fixed something;
///   * a resolved pattern that comes back has its `resolved_at` cleared, which
///     is a regression worth being able to see.
pub async fn sync_patterns(
    pool: &PgPool,
    dota_player_id: Uuid,
    detected: &[RecurringPattern],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    for pattern in detected {
        sqlx::query(
            "INSERT INTO player_patterns
                 (dota_player_id, pattern_id, occurrences, measured, rate, recent_rate,
                  status, first_seen_at, last_seen_at, first_detected_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now(), now())
             ON CONFLICT (dota_player_id, pattern_id) DO UPDATE
                 SET occurrences = EXCLUDED.occurrences,
                     measured    = EXCLUDED.measured,
                     rate        = EXCLUDED.rate,
                     recent_rate = EXCLUDED.recent_rate,
                     status      = EXCLUDED.status,
                     first_seen_at = EXCLUDED.first_seen_at,
                     last_seen_at  = EXCLUDED.last_seen_at,
                     -- Deliberately not EXCLUDED: the first sighting is the
                     -- one fact here that must survive recomputation.
                     resolved_at = NULL,
                     updated_at  = now()",
        )
        .bind(dota_player_id)
        .bind(&pattern.id)
        .bind(pattern.occurrences)
        .bind(pattern.measured)
        .bind(pattern.rate)
        .bind(pattern.recent_rate)
        .bind(pattern.status.slug())
        .bind(pattern.first_seen_at)
        .bind(pattern.last_seen_at)
        .execute(&mut *tx)
        .await?;
    }

    let still_present: Vec<String> = detected.iter().map(|p| p.id.clone()).collect();
    sqlx::query(
        "UPDATE player_patterns
            SET status      = 'resolved',
                resolved_at = COALESCE(resolved_at, now()),
                updated_at  = now()
          WHERE dota_player_id = $1
            AND status <> 'resolved'
            AND NOT (pattern_id = ANY($2))",
    )
    .bind(dota_player_id)
    .bind(&still_present)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Stored patterns, with the detector's own wording reattached.
///
/// Labels and descriptions live in code, not in the database: they are
/// presentation, and a copy in every row would go stale the moment one is
/// reworded.
pub async fn list_patterns(
    pool: &PgPool,
    dota_player_id: Uuid,
) -> Result<Vec<RecurringPattern>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        pattern_id: String,
        occurrences: i64,
        measured: i64,
        rate: f32,
        recent_rate: Option<f32>,
        status: String,
        first_seen_at: Option<DateTime<Utc>>,
        last_seen_at: Option<DateTime<Utc>>,
        first_detected_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT pattern_id, occurrences, measured, rate, recent_rate, status,
                first_seen_at, last_seen_at, first_detected_at
           FROM player_patterns
          WHERE dota_player_id = $1
          ORDER BY rate DESC, pattern_id",
    )
    .bind(dota_player_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            // A slug with no detector behind it means the detector was
            // removed. Dropping the row is better than rendering a pattern
            // nobody can explain.
            let detector = crate::services::player_model::patterns::DETECTORS
                .iter()
                .find(|d| d.id == row.pattern_id)?;
            let status = PatternStatus::parse(&row.status)?;

            Some(RecurringPattern {
                id: row.pattern_id,
                label: detector.label.to_string(),
                description: detector.description.to_string(),
                occurrences: row.occurrences,
                measured: row.measured,
                rate: row.rate,
                recent_rate: row.recent_rate,
                // Not stored: it is only meaningful next to the rate it was
                // measured with, which the detector recomputes.
                recent_measured: 0,
                status,
                status_label: status.label(),
                confidence: confidence_for(row.measured),
                statement: stored_statement(
                    detector.label,
                    &row.status,
                    row.occurrences,
                    row.measured,
                    row.rate,
                ),
                examples: Vec::new(),
                first_seen_at: row.first_seen_at,
                last_seen_at: row.last_seen_at,
                first_detected_at: Some(row.first_detected_at),
            })
        })
        .collect())
}

fn confidence_for(measured: i64) -> Confidence {
    percentile::confidence_for(measured)
}

/// The sentence for a pattern read back from storage.
///
/// A resolved pattern is phrased in the past tense: saying "dies too often in
/// 12 of 20 matches" about something the player has fixed would be actively
/// misleading.
fn stored_statement(
    label: &str,
    status: &str,
    occurrences: i64,
    measured: i64,
    rate: f32,
) -> String {
    let matches = if measured == 1 { "match" } else { "matches" };

    if status == "resolved" {
        format!(
            "{label}: no longer meets the threshold. It last stood at {occurrences} of {measured} measurable {matches} ({:.0}%).",
            rate * 100.0
        )
    } else {
        format!(
            "{label} in {occurrences} of the {measured} {matches} this could be measured in ({:.0}%).",
            rate * 100.0
        )
    }
}
