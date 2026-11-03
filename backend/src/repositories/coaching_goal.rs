//! Goals stored with the analysis that set them.

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::coaching::PlanGoal;

pub async fn insert_all(
    pool: &PgPool,
    analysis_id: Uuid,
    goals: &[PlanGoal],
) -> Result<(), sqlx::Error> {
    if goals.is_empty() {
        return Ok(());
    }
    let positions: Vec<i32> = goals.iter().map(|g| g.position).collect();
    let evidence: Vec<&str> = goals.iter().map(|g| g.evidence_id.as_str()).collect();
    let metrics: Vec<&str> = goals.iter().map(|g| g.metric.as_str()).collect();
    let labels: Vec<&str> = goals.iter().map(|g| g.label.as_str()).collect();
    let heroes: Vec<Option<i32>> = goals.iter().map(|g| g.hero_id).collect();
    let higher: Vec<bool> = goals.iter().map(|g| g.higher_is_better).collect();
    let baselines: Vec<f32> = goals.iter().map(|g| g.baseline).collect();
    let targets: Vec<f32> = goals.iter().map(|g| g.target).collect();
    let target_labels: Vec<&str> = goals.iter().map(|g| g.target_label.as_str()).collect();

    sqlx::query(
        "INSERT INTO coaching_goals
             (analysis_id, position, evidence_id, metric, label, hero_id,
              higher_is_better, baseline, target, target_label)
         SELECT $1, t.*
           FROM UNNEST($2::int[], $3::text[], $4::text[], $5::text[], $6::int[],
                       $7::bool[], $8::real[], $9::real[], $10::text[])
             AS t(position, evidence_id, metric, label, hero_id,
                  higher_is_better, baseline, target, target_label)
         ON CONFLICT (analysis_id, position) DO NOTHING",
    )
    .bind(analysis_id)
    .bind(&positions)
    .bind(&evidence)
    .bind(&metrics)
    .bind(&labels)
    .bind(&heroes)
    .bind(&higher)
    .bind(&baselines)
    .bind(&targets)
    .bind(&target_labels)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn for_analysis(pool: &PgPool, analysis_id: Uuid) -> Result<Vec<PlanGoal>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        position: i32,
        evidence_id: String,
        metric: String,
        label: String,
        hero_id: Option<i32>,
        higher_is_better: bool,
        baseline: f32,
        target: f32,
        target_label: String,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT position, evidence_id, metric, label, hero_id, higher_is_better,
                baseline, target, target_label
           FROM coaching_goals
          WHERE analysis_id = $1
          ORDER BY position",
    )
    .bind(analysis_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| PlanGoal {
            position: r.position,
            evidence_id: r.evidence_id,
            metric: r.metric,
            label: r.label,
            hero_id: r.hero_id,
            higher_is_better: r.higher_is_better,
            baseline: r.baseline,
            target: r.target,
            target_label: r.target_label,
        })
        .collect())
}
