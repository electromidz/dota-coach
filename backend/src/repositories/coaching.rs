use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::coaching::{
    AnalysisScope, CoachingAnalysis, Evidence, Insight, InsightKind, PlanStep,
};
use crate::domain::role::CoachableRole;

/// A macro rather than a `const`: sqlx needs a `&'static str` query, and
/// `concat!` only composes literals.
macro_rules! analysis_columns {
    () => {
        "id, match_id, scope, role, model, summary, evidence, generated_at"
    };
}

/// A validated analysis on its way into storage.
pub struct NewAnalysis<'a> {
    pub dota_player_id: Uuid,
    pub match_id: Option<Uuid>,
    pub scope: AnalysisScope,
    /// The role this analysis is about. `None` only for an analysis that is
    /// genuinely not role-scoped.
    pub role: Option<CoachableRole>,
    pub context_hash: &'a str,
    pub model: &'a str,
    pub summary: &'a str,
    pub evidence: &'a [Evidence],
    pub insights: &'a [Insight],
    pub plan: &'a [PlanStep],
}

/// Store an analysis and its insights atomically.
///
/// A transaction because a summary with no insights, or insights with no
/// parent, would both be read as a coherent answer by everything downstream.
/// A repeat of the same question replaces the stored answer rather than
/// erroring: the second answer is as valid as the first, and the unique key
/// exists to bound growth, not to refuse writes.
pub async fn insert(pool: &PgPool, analysis: &NewAnalysis<'_>) -> Result<Uuid, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let evidence = serde_json::to_value(analysis.evidence).unwrap_or(serde_json::Value::Null);

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO coaching_analyses
             (dota_player_id, match_id, scope, role, context_hash, model, summary, evidence,
              generated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now())
         ON CONFLICT (dota_player_id, match_id, context_hash) DO UPDATE
             SET role         = EXCLUDED.role,
                 model        = EXCLUDED.model,
                 summary      = EXCLUDED.summary,
                 evidence     = EXCLUDED.evidence,
                 generated_at = now()
         RETURNING id",
    )
    .bind(analysis.dota_player_id)
    .bind(analysis.match_id)
    .bind(scope_slug(analysis.scope))
    .bind(analysis.role.map(CoachableRole::slug))
    .bind(analysis.context_hash)
    .bind(analysis.model)
    .bind(analysis.summary)
    .bind(&evidence)
    .fetch_one(&mut *tx)
    .await?;

    // Replacing an answer must not leave the previous answer's insights or plan
    // behind. Both are rewritten wholesale, so a shorter second answer cannot
    // inherit steps from a longer first one.
    sqlx::query("DELETE FROM coaching_insights WHERE analysis_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM coaching_plan_steps WHERE analysis_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    for (position, insight) in analysis.insights.iter().enumerate() {
        sqlx::query(
            "INSERT INTO coaching_insights
                 (analysis_id, position, kind, title, explanation, evidence_refs)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(position as i32)
        .bind(insight.kind.slug())
        .bind(&insight.title)
        .bind(&insight.explanation)
        .bind(&insight.evidence)
        .execute(&mut *tx)
        .await?;
    }

    for step in analysis.plan {
        sqlx::query(
            "INSERT INTO coaching_plan_steps
                 (analysis_id, position, title, action, evidence_refs)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(step.position as i32)
        .bind(&step.title)
        .bind(&step.action)
        .bind(&step.evidence)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(id)
}

/// The answer to this exact question, if it has already been asked.
pub async fn find_by_hash(
    pool: &PgPool,
    dota_player_id: Uuid,
    match_id: Option<Uuid>,
    context_hash: &str,
) -> Result<Option<CoachingAnalysis>, sqlx::Error> {
    let row = sqlx::query_as::<_, AnalysisRow>(concat!(
        "SELECT ",
        analysis_columns!(),
        " FROM coaching_analyses
          WHERE dota_player_id = $1
            AND match_id IS NOT DISTINCT FROM $2
            AND context_hash = $3"
    ))
    .bind(dota_player_id)
    .bind(match_id)
    .bind(context_hash)
    .fetch_optional(pool)
    .await?;

    hydrate(pool, row).await
}

/// The most recent analysis of one scope, whatever question produced it.
///
/// This is what `GET /api/coach` reads: showing the last answer costs nothing,
/// where regenerating on every page load would spend a model call per refresh.
///
/// The role is part of the filter, not a detail of the cache key. Without it a
/// player who switched from Carry to Support would be shown their last Carry
/// analysis under a Support heading — stale advice about the wrong role, which
/// is worse than no advice at all.
pub async fn latest(
    pool: &PgPool,
    dota_player_id: Uuid,
    match_id: Option<Uuid>,
    role: Option<CoachableRole>,
) -> Result<Option<CoachingAnalysis>, sqlx::Error> {
    let row = sqlx::query_as::<_, AnalysisRow>(concat!(
        "SELECT ",
        analysis_columns!(),
        " FROM coaching_analyses
          WHERE dota_player_id = $1
            AND match_id IS NOT DISTINCT FROM $2
            AND role IS NOT DISTINCT FROM $3
          ORDER BY generated_at DESC
          LIMIT 1"
    ))
    .bind(dota_player_id)
    .bind(match_id)
    .bind(role.map(CoachableRole::slug))
    .fetch_optional(pool)
    .await?;

    hydrate(pool, row).await
}

/// How many analyses this player has generated since a moment.
///
/// Counts stored analyses rather than attempts: a cache hit costs nothing and
/// must not consume the daily budget.
pub async fn count_since(
    pool: &PgPool,
    dota_player_id: Uuid,
    since: DateTime<Utc>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*)
           FROM coaching_analyses
          WHERE dota_player_id = $1
            AND generated_at >= $2",
    )
    .bind(dota_player_id)
    .bind(since)
    .fetch_one(pool)
    .await
}

pub async fn last_generated_at(
    pool: &PgPool,
    dota_player_id: Uuid,
) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    sqlx::query_scalar("SELECT MAX(generated_at) FROM coaching_analyses WHERE dota_player_id = $1")
        .bind(dota_player_id)
        .fetch_one(pool)
        .await
}

#[derive(sqlx::FromRow)]
struct AnalysisRow {
    id: Uuid,
    match_id: Option<Uuid>,
    scope: String,
    role: Option<String>,
    model: String,
    summary: String,
    evidence: serde_json::Value,
    generated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct PlanStepRow {
    position: i32,
    title: String,
    action: String,
    evidence_refs: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct InsightRow {
    kind: String,
    title: String,
    explanation: String,
    evidence_refs: Vec<String>,
}

/// Attach the insights to an analysis row.
async fn hydrate(
    pool: &PgPool,
    row: Option<AnalysisRow>,
) -> Result<Option<CoachingAnalysis>, sqlx::Error> {
    let Some(row) = row else {
        return Ok(None);
    };

    let insights = sqlx::query_as::<_, InsightRow>(
        "SELECT kind, title, explanation, evidence_refs
           FROM coaching_insights
          WHERE analysis_id = $1
          ORDER BY position",
    )
    .bind(row.id)
    .fetch_all(pool)
    .await?;

    let plan = sqlx::query_as::<_, PlanStepRow>(
        "SELECT position, title, action, evidence_refs
           FROM coaching_plan_steps
          WHERE analysis_id = $1
          ORDER BY position",
    )
    .bind(row.id)
    .fetch_all(pool)
    .await?;

    let role = row.role.as_deref().and_then(CoachableRole::parse);

    Ok(Some(CoachingAnalysis {
        id: row.id,
        scope: parse_scope(&row.scope),
        role,
        role_label: role.map(CoachableRole::label),
        match_id: row.match_id,
        model: row.model,
        summary: row.summary,
        insights: insights
            .into_iter()
            .filter_map(|i| {
                // A kind that no longer parses means the enum shrank under a
                // stored row. Dropping it is better than inventing a mapping.
                let kind = InsightKind::parse(&i.kind)?;
                Some(Insight {
                    kind,
                    kind_label: kind.label(),
                    title: i.title,
                    explanation: i.explanation,
                    evidence: i.evidence_refs,
                })
            })
            .collect(),
        plan: plan
            .into_iter()
            .map(|step| PlanStep {
                position: step.position.max(1) as u32,
                title: step.title,
                action: step.action,
                evidence: step.evidence_refs,
            })
            .collect(),
        evidence: serde_json::from_value(row.evidence).unwrap_or_default(),
        generated_at: row.generated_at,
    }))
}

fn scope_slug(scope: AnalysisScope) -> &'static str {
    match scope {
        AnalysisScope::Player => "player",
        AnalysisScope::Match => "match",
        AnalysisScope::Role => "role",
    }
}

fn parse_scope(value: &str) -> AnalysisScope {
    match value {
        "match" => AnalysisScope::Match,
        "role" => AnalysisScope::Role,
        _ => AnalysisScope::Player,
    }
}
