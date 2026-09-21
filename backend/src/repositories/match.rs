use std::collections::HashSet;

use sqlx::{AssertSqlSafe, PgPool};
use uuid::Uuid;

use crate::domain::r#match::{Match, NewMatch};
use crate::domain::role::CoachableRole;
use crate::domain::scope::MatchScope;

/// A macro rather than a `const` because sqlx only accepts `&'static str`
/// queries; `concat!` keeps the composed SQL a compile-time literal.
macro_rules! columns {
    () => {
        "m.id, m.dota_player_id, m.match_id, m.hero_id, m.hero_name, m.role, m.lane_role, \
         m.won, m.duration_seconds, m.kills, m.deaths, m.assists, m.gpm, m.xpm, \
         m.last_hits, m.denies, m.net_worth, m.hero_damage, m.tower_damage, \
         m.hero_healing, m.game_mode, m.lobby_type, m.party_size, m.started_at, \
         m.detail_synced, m.team_kills, m.team_deaths, m.replay_parsed, \
         m.last_hits_at_10, m.last_hits_at_15, m.gold_at_10, m.gold_at_15, \
         m.xp_at_10, m.xp_at_15, m.bkb_seconds, m.blink_seconds, m.midas_seconds, \
         m.teamfight_participation, m.created_at, m.updated_at"
    };
}

/// How a listed page is ordered.
///
/// Short on purpose. Every variant answers a question a player actually asks of
/// their history — "what did I just play", "how did this start", "where did it
/// go well", "where did it go badly" — and a sort that answers none of those is
/// a control to scroll past rather than a feature. Win/loss is a *filter*, not
/// an ordering: sorting by it would only group rows that the filter removes
/// outright.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchSort {
    #[default]
    Newest,
    Oldest,
    HighestGpm,
    LowestGpm,
    HighestKda,
}

impl MatchSort {
    pub fn slug(self) -> &'static str {
        match self {
            MatchSort::Newest => "newest",
            MatchSort::Oldest => "oldest",
            MatchSort::HighestGpm => "gpm_desc",
            MatchSort::LowestGpm => "gpm_asc",
            MatchSort::HighestKda => "kda_desc",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [
            MatchSort::Newest,
            MatchSort::Oldest,
            MatchSort::HighestGpm,
            MatchSort::LowestGpm,
            MatchSort::HighestKda,
        ]
        .into_iter()
        .find(|sort| sort.slug() == value)
    }

    /// The `ORDER BY` body. A compile-time constant in every arm — nothing a
    /// caller supplies reaches the SQL.
    ///
    /// Recency is the tiebreaker everywhere, so equal values keep a stable,
    /// meaningful order rather than whatever the planner returns. A match with
    /// no computed metrics has no KDA to sort on and sorts last rather than
    /// being treated as zero.
    fn order_by(self) -> &'static str {
        match self {
            MatchSort::Newest => "m.started_at DESC",
            MatchSort::Oldest => "m.started_at ASC",
            MatchSort::HighestGpm => "m.gpm DESC, m.started_at DESC",
            MatchSort::LowestGpm => "m.gpm ASC, m.started_at DESC",
            MatchSort::HighestKda => "mm.kda DESC NULLS LAST, m.started_at DESC",
        }
    }
}

/// Which of a player's matches a listed page shows, and in what order.
///
/// Applied *on top of* whatever [`MatchScope`] the caller chose, never instead
/// of it: the scope decides which population is being browsed, and these decide
/// which rows of it are interesting right now. Filtering after the scope's
/// window is what makes "my losses as Carry" mean "among the games the coach
/// reads" rather than quietly widening the population to find more of them.
///
/// Every field is `None` by default, which is the unfiltered list the page has
/// always shown.
#[derive(Debug, Clone, Default)]
pub struct MatchFilter {
    pub hero_id: Option<i32>,
    /// `Some(true)` for wins, `Some(false)` for losses.
    pub won: Option<bool>,
    pub role: Option<CoachableRole>,
    pub sort: MatchSort,
}

impl MatchFilter {
    /// The stored `matches.role` labels this filter accepts, if any.
    ///
    /// Bound as an array rather than interpolated — these are constants today,
    /// but a bound parameter cannot become an injection tomorrow.
    fn role_labels(&self) -> Option<Vec<String>> {
        self.role
            .map(|role| role.stored_labels().iter().map(|l| l.to_string()).collect())
    }

    /// The predicate over `m`, using three bind slots starting at `first`.
    ///
    /// Each clause is a no-op when its parameter is `NULL`, so one SQL string
    /// serves every combination of filters and the bind positions never shift.
    fn predicate(&self, first: usize) -> String {
        let (hero, won, roles) = (first, first + 1, first + 2);
        format!(
            "AND (${hero}::int IS NULL OR m.hero_id = ${hero})
             AND (${won}::bool IS NULL OR m.won = ${won})
             AND (${roles}::text[] IS NULL OR m.role = ANY(${roles}))"
        )
    }

    /// True when this would list exactly what an unfiltered query would.
    pub fn is_empty(&self) -> bool {
        self.hero_id.is_none() && self.won.is_none() && self.role.is_none()
    }
}

/// Match ids already stored for this player. The sync planner diffs against
/// this so nothing is fetched or inserted twice.
pub async fn existing_match_ids(
    pool: &PgPool,
    dota_player_id: Uuid,
) -> Result<HashSet<i64>, sqlx::Error> {
    let ids: Vec<i64> =
        sqlx::query_scalar("SELECT match_id FROM matches WHERE dota_player_id = $1")
            .bind(dota_player_id)
            .fetch_all(pool)
            .await?;

    Ok(ids.into_iter().collect())
}

/// Insert matches, skipping any that already exist.
///
/// Returns how many rows were actually created. `UNIQUE (dota_player_id,
/// match_id)` does the deduplication, so concurrent syncs cannot produce
/// duplicates even if both pass the planner's check.
pub async fn insert_new(pool: &PgPool, matches: &[NewMatch]) -> Result<u64, sqlx::Error> {
    if matches.is_empty() {
        return Ok(0);
    }

    let mut tx = pool.begin().await?;
    let mut inserted = 0;

    for m in matches {
        let d = &m.data;
        let rows = sqlx::query(
            "INSERT INTO matches (
                 dota_player_id, match_id, hero_id, hero_name, role, lane_role, won,
                 duration_seconds, kills, deaths, assists, gpm, xpm, last_hits, denies,
                 net_worth, hero_damage, tower_damage, hero_healing, game_mode, lobby_type,
                 party_size, started_at, detail_synced, team_kills, team_deaths,
                 replay_parsed, last_hits_at_10, last_hits_at_15, gold_at_10, gold_at_15,
                 xp_at_10, xp_at_15, bkb_seconds, blink_seconds, midas_seconds,
                 teamfight_participation
             )
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
                     $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
                     $31, $32, $33, $34, $35, $36, $37)
             ON CONFLICT (dota_player_id, match_id) DO NOTHING",
        )
        .bind(m.dota_player_id)
        .bind(d.match_id)
        .bind(d.hero_id)
        .bind(&m.hero_name)
        .bind(&m.role)
        .bind(d.lane_role)
        .bind(d.won)
        .bind(d.duration_seconds)
        .bind(d.kills)
        .bind(d.deaths)
        .bind(d.assists)
        .bind(d.gpm)
        .bind(d.xpm)
        .bind(d.last_hits)
        .bind(d.denies)
        .bind(d.net_worth)
        .bind(d.hero_damage)
        .bind(d.tower_damage)
        .bind(d.hero_healing)
        .bind(d.game_mode)
        .bind(d.lobby_type)
        .bind(d.party_size)
        .bind(d.started_at)
        .bind(d.from_details)
        .bind(d.team_kills)
        .bind(d.team_deaths)
        .bind(d.replay_parsed)
        .bind(d.last_hits_at_10)
        .bind(d.last_hits_at_15)
        .bind(d.gold_at_10)
        .bind(d.gold_at_15)
        .bind(d.xp_at_10)
        .bind(d.xp_at_15)
        .bind(d.bkb_seconds)
        .bind(d.blink_seconds)
        .bind(d.midas_seconds)
        .bind(d.teamfight_participation)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        inserted += rows;
    }

    tx.commit().await?;
    Ok(inserted)
}

pub async fn list_by_player(
    pool: &PgPool,
    dota_player_id: Uuid,
    filter: &MatchFilter,
    limit: i64,
    offset: i64,
) -> Result<Vec<Match>, sqlx::Error> {
    sqlx::query_as::<_, Match>(AssertSqlSafe(format!(
        "SELECT {cols}, mm.kda AS metrics_kda
           FROM matches m
           LEFT JOIN match_metrics mm ON mm.match_id = m.id
          WHERE m.dota_player_id = $1
                {predicate}
          ORDER BY {order}
          LIMIT $2 OFFSET $3",
        cols = columns!(),
        predicate = filter.predicate(4),
        order = filter.sort.order_by(),
    )))
    .bind(dota_player_id)
    .bind(limit)
    .bind(offset)
    .bind(filter.hero_id)
    .bind(filter.won)
    .bind(filter.role_labels())
    .fetch_all(pool)
    .await
}

/// One page of matches from a [`MatchScope`].
///
/// The metrics join is inner here where the unscoped list has it outer: the
/// scope's own window already requires computed metrics, so an outer join would
/// promise rows the window cannot contain.
pub async fn list_by_player_scoped(
    pool: &PgPool,
    dota_player_id: Uuid,
    scope: &MatchScope,
    filter: &MatchFilter,
    limit: i64,
    offset: i64,
) -> Result<Vec<Match>, sqlx::Error> {
    sqlx::query_as::<_, Match>(AssertSqlSafe(format!(
        "{cte}
         SELECT {cols}, mm.kda AS metrics_kda
           FROM matches m
           JOIN match_metrics mm ON mm.match_id = m.id
           {join}
          WHERE m.dota_player_id = $1
                {predicate}
          ORDER BY {order}
          LIMIT $2 OFFSET $3",
        cte = scope.cte(),
        cols = columns!(),
        join = scope.join(),
        predicate = filter.predicate(4),
        order = filter.sort.order_by(),
    )))
    .bind(dota_player_id)
    .bind(limit)
    .bind(offset)
    .bind(filter.hero_id)
    .bind(filter.won)
    .bind(filter.role_labels())
    .fetch_all(pool)
    .await
}

pub async fn count_by_player(
    pool: &PgPool,
    dota_player_id: Uuid,
    filter: &MatchFilter,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT COUNT(*)
           FROM matches m
          WHERE m.dota_player_id = $1
                {predicate}",
        predicate = filter.predicate(2),
    )))
    .bind(dota_player_id)
    .bind(filter.hero_id)
    .bind(filter.won)
    .bind(filter.role_labels())
    .fetch_one(pool)
    .await
}

/// How many matches the scope contains, which is what the pagination of a
/// scoped list has to divide.
pub async fn count_by_player_scoped(
    pool: &PgPool,
    dota_player_id: Uuid,
    scope: &MatchScope,
    filter: &MatchFilter,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(AssertSqlSafe(format!(
        "{cte}
         SELECT COUNT(*)
           FROM matches m
           {join}
          WHERE m.dota_player_id = $1
                {predicate}",
        cte = scope.cte(),
        join = scope.join(),
        predicate = filter.predicate(2),
    )))
    .bind(dota_player_id)
    .bind(filter.hero_id)
    .bind(filter.won)
    .bind(filter.role_labels())
    .fetch_one(pool)
    .await
}

/// Which heroes and roles appear in a population, with how often.
///
/// Computed over the *scope alone*, never over the current filters: a hero
/// dropdown that shrinks to the hero already selected is a dropdown a user
/// cannot get back out of. It is also why these are real counts from the
/// player's own rows rather than a hero list shipped to the client — a filter
/// that offers a hero the player has never touched is offering an empty page.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HeroFacet {
    pub hero_id: i32,
    pub hero_name: String,
    pub matches: i64,
}

/// The estimator's stored label and its count. Mapped to a coachable role by
/// the caller, which is where that mapping already lives.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RoleFacet {
    pub role: String,
    pub matches: i64,
}

pub async fn hero_facets(
    pool: &PgPool,
    dota_player_id: Uuid,
    scope: &MatchScope,
) -> Result<Vec<HeroFacet>, sqlx::Error> {
    sqlx::query_as::<_, HeroFacet>(AssertSqlSafe(facet_sql(
        scope,
        "m.hero_id, m.hero_name, COUNT(*) AS matches",
        "m.hero_id, m.hero_name",
        "matches DESC, m.hero_name ASC",
    )))
    .bind(dota_player_id)
    .fetch_all(pool)
    .await
}

pub async fn role_facets(
    pool: &PgPool,
    dota_player_id: Uuid,
    scope: &MatchScope,
) -> Result<Vec<RoleFacet>, sqlx::Error> {
    sqlx::query_as::<_, RoleFacet>(AssertSqlSafe(facet_sql(
        scope,
        "m.role, COUNT(*) AS matches",
        "m.role",
        "matches DESC, m.role ASC",
    )))
    .bind(dota_player_id)
    .fetch_all(pool)
    .await
}

/// One grouped count over a scope. Every fragment is a caller-side literal.
fn facet_sql(scope: &MatchScope, select: &str, group_by: &str, order_by: &str) -> String {
    // A career scope reads every stored row and needs no window; anything
    // narrower joins the same CTE the scoped aggregates do, so the counts
    // beside a filter describe exactly the page it will produce.
    let (cte, join) = if scope.is_career() {
        (String::new(), "")
    } else {
        (scope.cte(), scope.join())
    };

    format!(
        "{cte}
         SELECT {select}
           FROM matches m
           {join}
          WHERE m.dota_player_id = $1
          GROUP BY {group_by}
          ORDER BY {order_by}"
    )
}

/// Fetch a match **scoped to its owner**.
///
/// Ownership is part of the query rather than a check afterwards: there is no
/// code path that loads someone else's match and then decides what to do.
pub async fn find_owned(
    pool: &PgPool,
    id: Uuid,
    dota_player_id: Uuid,
) -> Result<Option<Match>, sqlx::Error> {
    sqlx::query_as::<_, Match>(concat!(
        "SELECT ",
        columns!(),
        ", mm.kda AS metrics_kda
           FROM matches m
           LEFT JOIN match_metrics mm ON mm.match_id = m.id
          WHERE m.id = $1 AND m.dota_player_id = $2"
    ))
    .bind(id)
    .bind(dota_player_id)
    .fetch_optional(pool)
    .await
}

/// Fetch matches by id, for recomputing their metrics.
pub async fn find_many(pool: &PgPool, ids: &[Uuid]) -> Result<Vec<Match>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_as::<_, Match>(concat!(
        "SELECT ",
        columns!(),
        ", mm.kda AS metrics_kda
           FROM matches m
           LEFT JOIN match_metrics mm ON mm.match_id = m.id
          WHERE m.id = ANY($1)"
    ))
    .bind(ids)
    .fetch_all(pool)
    .await
}

/// Stored matches that predate a fact the metrics engine now needs, or that
/// were stored with one missing.
///
/// Deduplication means an existing match is never re-fetched by the normal
/// sync path, so without this a schema addition would only ever apply to
/// matches synced after it landed — and a row written from an incomplete
/// provider response would stay wrong forever.
///
/// The three conditions are different kinds of incomplete:
///
///   - `team_kills` is the fact the metrics engine gained after the first
///     matches were already stored;
///   - a zero duration is never a real match, only a field that never arrived;
///   - a null `game_mode` means the mode was never recorded, which the metrics
///     layer needs in order to tell a Turbo game from a ranked one.
///
/// A row the detail endpoint genuinely cannot complete is re-fetched on each
/// sync. That is bounded by `limit` and by the sync cooldown, and is the right
/// trade: the alternative is storing a zero as though it were measured.
pub async fn missing_facts(
    pool: &PgPool,
    dota_player_id: Uuid,
    limit: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT match_id
           FROM matches
          WHERE dota_player_id = $1
            AND (team_kills IS NULL
                 OR duration_seconds = 0
                 OR game_mode IS NULL)
          ORDER BY started_at DESC
          LIMIT $2",
    )
    .bind(dota_player_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Write freshly fetched facts onto an existing match row.
pub async fn update_facts(
    pool: &PgPool,
    dota_player_id: Uuid,
    d: &crate::domain::r#match::NormalizedMatch,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE matches SET
             denies = COALESCE($3, denies),
             net_worth = COALESCE($4, net_worth),
             hero_damage = COALESCE($5, hero_damage),
             tower_damage = COALESCE($6, tower_damage),
             hero_healing = COALESCE($7, hero_healing),
             team_kills = COALESCE($8, team_kills),
             team_deaths = COALESCE($9, team_deaths),
             replay_parsed = $10,
             last_hits_at_10 = COALESCE($11, last_hits_at_10),
             last_hits_at_15 = COALESCE($12, last_hits_at_15),
             gold_at_10 = COALESCE($13, gold_at_10),
             gold_at_15 = COALESCE($14, gold_at_15),
             xp_at_10 = COALESCE($15, xp_at_10),
             xp_at_15 = COALESCE($16, xp_at_15),
             bkb_seconds = COALESCE($17, bkb_seconds),
             blink_seconds = COALESCE($18, blink_seconds),
             midas_seconds = COALESCE($19, midas_seconds),
             teamfight_participation = COALESCE($20, teamfight_participation),
             -- Facts the summary endpoint drops when every game mode is
             -- requested. A stored zero duration is never real, so the detail
             -- value replaces it; a real duration is never overwritten.
             duration_seconds = CASE
                 WHEN duration_seconds = 0 THEN $21
                 ELSE duration_seconds
             END,
             game_mode = COALESCE($22, game_mode),
             lobby_type = COALESCE($23, lobby_type),
             detail_synced = TRUE
           WHERE dota_player_id = $1 AND match_id = $2",
    )
    .bind(dota_player_id)
    .bind(d.match_id)
    .bind(d.denies)
    .bind(d.net_worth)
    .bind(d.hero_damage)
    .bind(d.tower_damage)
    .bind(d.hero_healing)
    .bind(d.team_kills)
    .bind(d.team_deaths)
    .bind(d.replay_parsed)
    .bind(d.last_hits_at_10)
    .bind(d.last_hits_at_15)
    .bind(d.gold_at_10)
    .bind(d.gold_at_15)
    .bind(d.xp_at_10)
    .bind(d.xp_at_15)
    .bind(d.bkb_seconds)
    .bind(d.blink_seconds)
    .bind(d.midas_seconds)
    .bind(d.teamfight_participation)
    .bind(d.duration_seconds)
    .bind(d.game_mode)
    .bind(d.lobby_type)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
