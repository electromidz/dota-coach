use sqlx::PgPool;
use uuid::Uuid;

/// One hero's history for a player, straight from SQL.
///
/// Counts and averages only — the classification into Signature/Comfort/
/// Stretch/Risk is a domain decision and happens in `services::heroes`, where
/// it can be tested without a database.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HeroPoolRow {
    pub hero_id: i32,
    pub hero_name: String,
    /// The player's most frequent role on this hero.
    pub role: String,
    pub matches: i64,
    pub wins: i64,
    /// Matches inside the recent window, and wins among them. Reported
    /// separately so a caller can tell a 100% from two games apart from one
    /// over twenty.
    pub recent_matches: i64,
    pub recent_wins: i64,
    pub avg_kda: f32,
    pub avg_gpm: f32,
    pub last_played_at: chrono::DateTime<chrono::Utc>,
}

/// Every hero the player has a stored match on, most-played first.
///
/// `recent_window` is counted per hero rather than globally: "your last five
/// games on this hero" is the question a hero pool answers, and a global
/// cutoff would leave a hero the player has not touched in a month with an
/// empty recent record instead of its actual last five games.
pub async fn all(
    pool: &PgPool,
    dota_player_id: Uuid,
    recent_window: i64,
) -> Result<Vec<HeroPoolRow>, sqlx::Error> {
    sqlx::query_as::<_, HeroPoolRow>(
        "WITH ranked AS (
             SELECT
                 m.hero_id,
                 m.hero_name,
                 m.role,
                 m.won,
                 m.gpm,
                 m.started_at,
                 mm.kda,
                 ROW_NUMBER() OVER (
                     PARTITION BY m.hero_id ORDER BY m.started_at DESC
                 ) AS recency
               FROM matches m
               JOIN match_metrics mm ON mm.match_id = m.id
              WHERE m.dota_player_id = $1
         )
         SELECT
             hero_id,
             MIN(hero_name)                                        AS hero_name,
             -- The role played most often on this hero. MODE breaks ties by
             -- the sort order, which is stable but arbitrary; a tie means the
             -- player genuinely splits the hero between roles either way.
             MODE() WITHIN GROUP (ORDER BY role)                    AS role,
             COUNT(*)                                               AS matches,
             COUNT(*) FILTER (WHERE won)                            AS wins,
             COUNT(*) FILTER (WHERE recency <= $2)                  AS recent_matches,
             COUNT(*) FILTER (WHERE recency <= $2 AND won)          AS recent_wins,
             AVG(kda)::real                                         AS avg_kda,
             AVG(gpm)::real                                         AS avg_gpm,
             MAX(started_at)                                        AS last_played_at
           FROM ranked
          GROUP BY hero_id
          ORDER BY matches DESC, last_played_at DESC",
    )
    .bind(dota_player_id)
    .bind(recent_window)
    .fetch_all(pool)
    .await
}
