//! Hero Intelligence.
//!
//! Three surfaces over one pipeline:
//!
//!   `GET /api/heroes`                  the player's own hero pool
//!   `GET /api/heroes/recommendations`  scored candidates, best fit first
//!   `GET /api/hero-intelligence`       both, plus the meta they were scored against
//!
//! The pool endpoint deliberately makes no external call: a player must be
//! able to read their own repertoire when every provider is down.

use std::collections::{HashMap, HashSet};

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::extract::{AppQuery, EntitledUser};
use crate::domain::benchmark::{BenchmarkContext, Segment};
use crate::domain::hero::{HeroFit, HeroMeta, HeroMetaContext, HeroPoolEntry, RankBracket};
use crate::domain::player::DotaPlayer;
use crate::domain::role::CoachableRole;
use crate::domain::scope::MatchScope;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::services::benchmarks::{self, percentile, PlayerValues};
use crate::services::heroes::{self, FitInput, PlayerBaseline, PoolSummary, RECENT_WINDOW};
use crate::services::training;
use crate::state::AppState;
use utoipa::ToSchema;

#[derive(Deserialize)]
pub struct HeroQuery {
    /// Overrides the default taken from `HeroConfig::recommendation_limit`.
    pub limit: Option<usize>,
    /// `all` for every role, a role slug for one, absent for the coaching
    /// role (or every role when none is chosen).
    pub role: Option<String>,
}

/// Which matches the page reads. The same window Coach uses, so a hero's
/// record and tier agree across the two pages for the same role.
#[derive(Serialize, ToSchema)]
pub struct HeroScope {
    /// The role being read, or `null` for every role.
    pub role: Option<CoachableRole>,
    pub role_label: Option<&'static str>,
    /// The role being coached, offered as the default filter.
    pub coaching_role: Option<CoachableRole>,
    pub coaching_role_label: Option<&'static str>,
    /// e.g. "Ranked + All Pick".
    pub population: &'static str,
    /// The newest this-many eligible matches.
    pub window: i64,
}

#[derive(Serialize, ToSchema)]
pub struct HeroPoolResponse {
    pub scope: HeroScope,
    pub pool: Vec<HeroPoolEntry>,
    pub summary: PoolSummary,
    /// How many recent matches per hero "recent form" is measured over.
    pub recent_window: i64,
    pub note: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct RecommendationsResponse {
    /// Heroes with enough games behind them to recommend, best fit first.
    pub recommendations: Vec<HeroFit>,
    /// Strong meta heroes with too few games to recommend yet.
    pub worth_trying: Vec<HeroFit>,
    pub meta: MetaContext,
}

#[derive(Serialize, ToSchema)]
pub struct HeroIntelligenceResponse {
    pub scope: HeroScope,
    pub pool: Vec<HeroPoolEntry>,
    pub summary: PoolSummary,
    /// Heroes with enough games behind them to recommend, best fit first.
    pub recommendations: Vec<HeroFit>,
    /// Strong meta heroes with too few games to recommend yet.
    pub worth_trying: Vec<HeroFit>,
    /// The strongest heroes in the cohort, whether or not the player plays
    /// them — "the current meta", as its own section.
    pub meta_leaders: Vec<HeroMeta>,
    pub meta: MetaContext,
    pub recent_window: i64,
    pub note: Option<String>,
}

/// What the recommendations were actually scored against.
///
/// Reported on every response so a client never has to assume a rank-aware
/// comparison the data behind it cannot support.
#[derive(Serialize, ToSchema)]
pub struct MetaContext {
    pub available: bool,
    /// Provider name, for attribution. Never a key or a URL.
    pub source: Option<&'static str>,
    pub bracket: Option<RankBracket>,
    pub bracket_label: Option<&'static str>,
    pub segmented_by: Vec<Segment>,
    pub note: Option<String>,
}

/// `GET /api/heroes`
#[utoipa::path(
    get, path = "/api/heroes", tag = "heroes",
    summary = "The player's hero pool",
    description = "Built from stored matches alone, so it keeps answering while the hero meta provider is down.",
    security(("session" = [])),
    params(
        ("role" = Option<String>, Query,
            description = "`all`, or a role slug (e.g. `soft_support`). Absent: the coaching role, or every role if none is chosen. Same match window as Coach.",
            example = "soft_support"),
    ),
    responses(
        (status = 200, description = "Every hero played, classified", body = HeroPoolResponse),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 402, description = "Trial expired and no active subscription", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn pool(
    State(state): State<AppState>,
    EntitledUser(user): EntitledUser,
    AppQuery(query): AppQuery<HeroQuery>,
) -> AppResult<Json<HeroPoolResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let (scope, info) = resolve_scope(&state, &player, query.role.as_deref()).await?;
    let (mut pool, _) = load_pool(&state, &player, &scope).await?;
    canonical_pool_names(&state, &mut pool).await;
    sort_by_tier(&mut pool);
    let summary = heroes::summarize(&pool);

    Ok(Json(HeroPoolResponse {
        scope: info,
        note: empty_pool_note(&pool),
        pool,
        summary,
        recent_window: RECENT_WINDOW,
    }))
}

/// `GET /api/heroes/recommendations`
#[utoipa::path(
    get, path = "/api/heroes/recommendations", tag = "heroes",
    summary = "Heroes worth playing now",
    description = "Ranked by fit score, not by global win rate: a strong meta hero the player has never played can score below a weaker one they are good at. Each entry carries the components behind its score.",
    security(("session" = [])),
    params(
        ("limit" = Option<usize>, Query,
            description = "How many recommendations to return. Defaults to HERO_RECOMMENDATION_LIMIT (8), \
and is clamped to 50 however high it is set.",
            example = 8, minimum = 1, maximum = 50),
        ("role" = Option<String>, Query,
            description = "`all`, or a role slug (e.g. `soft_support`). Absent: the coaching role, or every role if none is chosen. Same match window as Coach.",
            example = "soft_support"),
    ),
    responses(
        (status = 200, description = "Scored recommendations, best first", body = RecommendationsResponse),
        (status = 502, description = "The hero meta provider is unavailable", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 402, description = "Trial expired and no active subscription", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn recommendations(
    State(state): State<AppState>,
    EntitledUser(user): EntitledUser,
    AppQuery(query): AppQuery<HeroQuery>,
) -> AppResult<Json<RecommendationsResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let (scope, info) = resolve_scope(&state, &player, query.role.as_deref()).await?;
    let built = build(&state, &user, query.limit, &scope, info.role).await?;

    Ok(Json(RecommendationsResponse {
        recommendations: built.recommendations,
        worth_trying: built.worth_trying,
        meta: built.meta,
    }))
}

/// `GET /api/hero-intelligence`
#[utoipa::path(
    get, path = "/api/hero-intelligence", tag = "heroes",
    summary = "Pool, meta and recommendations together",
    description = "One call for the hero page. Degrades rather than fails: if the meta provider is down the pool half still answers.",
    security(("session" = [])),
    params(
        ("limit" = Option<usize>, Query,
            description = "How many recommendations to include. Defaults to HERO_RECOMMENDATION_LIMIT (8), \
and is clamped to 50 however high it is set.",
            example = 8, minimum = 1, maximum = 50),
        ("role" = Option<String>, Query,
            description = "`all`, or a role slug (e.g. `soft_support`). Absent: the coaching role, or every role if none is chosen. Same match window as Coach.",
            example = "soft_support"),
    ),
    responses(
        (status = 200, description = "Combined hero intelligence", body = HeroIntelligenceResponse),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 402, description = "Trial expired and no active subscription", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn intelligence(
    State(state): State<AppState>,
    EntitledUser(user): EntitledUser,
    AppQuery(query): AppQuery<HeroQuery>,
) -> AppResult<Json<HeroIntelligenceResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let (scope, info) = resolve_scope(&state, &player, query.role.as_deref()).await?;
    let mut built = build(&state, &user, query.limit, &scope, info.role).await?;
    sort_by_tier(&mut built.pool);

    Ok(Json(HeroIntelligenceResponse {
        scope: info,
        note: empty_pool_note(&built.pool),
        summary: heroes::summarize(&built.pool),
        pool: built.pool,
        recommendations: built.recommendations,
        worth_trying: built.worth_trying,
        meta_leaders: built.meta_leaders,
        meta: built.meta,
        recent_window: RECENT_WINDOW,
    }))
}

pub(crate) struct Built {
    pub pool: Vec<HeroPoolEntry>,
    /// Only heroes with at least `RECOMMEND_MIN_MATCHES` games.
    pub recommendations: Vec<HeroFit>,
    /// Strong meta heroes the player has barely played, by meta strength.
    pub worth_trying: Vec<HeroFit>,
    pub meta_leaders: Vec<HeroMeta>,
    pub meta: MetaContext,
}

/// The one pipeline behind the recommendation surfaces.
///
/// Also read by the coaching layer, which needs the same pool and the same
/// scored candidates as evidence — recomputing them there would be a second
/// place for the two answers to drift apart.
/// `scope` decides which matches the pool and the baseline are read from. The
/// hero pages pass the career scope — a player's repertoire is their whole
/// repertoire — while the coaching layer passes its role scope, so a safe-lane
/// Phantom Assassin record never turns up as evidence in support coaching.
pub(crate) async fn build(
    state: &AppState,
    user: &User,
    limit: Option<usize>,
    scope: &MatchScope,
    role: Option<crate::domain::role::CoachableRole>,
) -> AppResult<Built> {
    let config = &state.config.heroes;
    let limit = limit.unwrap_or(config.recommendation_limit).clamp(1, 50);

    let player = load_linked_player(state, user).await?;
    let (mut pool, baseline) = load_pool(state, &player, scope).await?;
    let names = hero_names(state).await;
    rename(&names, pool.iter_mut().map(|e| (e.hero_id, &mut e.hero_name)));

    // A provider outage degrades the page rather than failing it: the player's
    // own history is local and still worth scoring on.
    let bracket = player.rank_tier.and_then(RankBracket::from_rank_tier);
    let context = HeroMetaContext::for_bracket(bracket);
    let (meta_set, outage_note) = match state.hero_meta.get_hero_meta(&context).await {
        Ok(mut set) => {
            // The meta provider's names can lag a rename; the catalogue wins.
            rename(&names, set.heroes.iter_mut().map(|h| (h.hero_id, &mut h.hero_name)));
            (Some(set), None)
        }
        Err(e) => {
            let note = e.user_note().to_string();
            tracing::warn!(error = %e, ?bracket, "hero meta unavailable");
            (None, Some(note))
        }
    };

    let meta_context = match &meta_set {
        Some(set) => MetaContext {
            available: true,
            source: Some(set.source),
            bracket: set.bracket,
            bracket_label: set.bracket.map(|b| b.label()),
            segmented_by: set.segmented_by.clone(),
            note: set.note.clone(),
        },
        None => MetaContext {
            available: false,
            source: None,
            bracket: None,
            bracket_label: None,
            segmented_by: Vec::new(),
            note: outage_note,
        },
    };

    let by_hero: HashMap<i32, &HeroMeta> = meta_set
        .as_ref()
        .map(|set| set.heroes.iter().map(|h| (h.hero_id, h)).collect())
        .unwrap_or_default();
    for entry in pool.iter_mut() {
        entry.meta_strength = by_hero.get(&entry.hero_id).map(|m| m.meta_strength);
    }

    let benchmarks =
        benchmark_percentiles(state, &player, &pool, config.benchmark_lookups, scope).await;

    // Training-focus compatibility, as a modifier on the finished score. Read
    // only: the heroes page reflects whatever focus is set, it never sets one.
    let (alignment, focus_title) = focus_alignment(state, &player, scope, role).await;

    // Candidates: everything the player has played, plus the strongest heroes
    // they have not. Including the latter is what makes this "which strong
    // heroes fit me" rather than "rank the heroes I already play" — they
    // simply have to survive the experience component to place well.
    let played: HashSet<i32> = pool.iter().map(|e| e.hero_id).collect();
    let newcomers: Vec<&HeroMeta> = top_meta(&meta_set, limit * 2)
        .into_iter()
        .filter(|m| !played.contains(&m.hero_id))
        .collect();

    let mut fits: Vec<HeroFit> = pool
        .iter()
        .map(|entry| {
            heroes::score(
                &FitInput {
                    hero_id: entry.hero_id,
                    hero_name: &entry.hero_name,
                    pool: Some(entry),
                    meta: by_hero.get(&entry.hero_id).copied(),
                    benchmark_percentile: benchmarks.get(&entry.hero_id).copied(),
                    focus_alignment: alignment.get(&entry.hero_id).copied(),
                    focus_title: focus_title.as_deref(),
                    baseline,
                },
                config.fit_weights,
            )
        })
        .collect();

    fits.extend(newcomers.iter().map(|meta| {
        heroes::score(
            &FitInput {
                hero_id: meta.hero_id,
                hero_name: &meta.hero_name,
                pool: None,
                meta: Some(meta),
                benchmark_percentile: None,
                // A hero with no history has no figures to compare, so the
                // modifier cannot apply either way.
                focus_alignment: None,
                focus_title: focus_title.as_deref(),
                baseline,
            },
            config.fit_weights,
        )
    }));

    let (recommendations, worth_trying) =
        heroes::split_recommendations(heroes::rank(fits), limit, WORTH_TRYING_LIMIT);

    Ok(Built {
        pool,
        recommendations,
        worth_trying,
        meta_leaders: top_meta(&meta_set, limit).into_iter().cloned().collect(),
        meta: meta_context,
    })
}

/// How many "worth trying" heroes the page shows.
const WORTH_TRYING_LIMIT: usize = 8;

/// Hero id -> canonical display name, from the provider's hero catalogue.
/// Empty when the catalogue is down; names then stay as stored.
pub(crate) async fn hero_names(state: &AppState) -> HashMap<i32, String> {
    state.dota.heroes().await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "hero catalogue unavailable; keeping stored names");
        HashMap::new()
    })
}

fn rename<'a>(names: &HashMap<i32, String>, entries: impl Iterator<Item = (i32, &'a mut String)>) {
    for (id, name) in entries {
        if let Some(canonical) = names.get(&id) {
            if name != canonical {
                *name = canonical.clone();
            }
        }
    }
}

async fn canonical_pool_names(state: &AppState, pool: &mut [HeroPoolEntry]) {
    let names = hero_names(state).await;
    rename(&names, pool.iter_mut().map(|e| (e.hero_id, &mut e.hero_name)));
}

/// Signature, Comfort, Risk, Stretch; most played first within a tier. Risk
/// sits before Stretch: heroes costing games matter more than untried ones.
fn sort_by_tier(pool: &mut [HeroPoolEntry]) {
    use crate::domain::hero::HeroTier;
    let rank = |tier: HeroTier| match tier {
        HeroTier::Signature => 0,
        HeroTier::Comfort => 1,
        HeroTier::Risk => 2,
        HeroTier::Stretch => 3,
    };
    pool.sort_by(|a, b| {
        rank(a.tier)
            .cmp(&rank(b.tier))
            .then(b.matches.cmp(&a.matches))
            .then(a.hero_id.cmp(&b.hero_id))
    });
}

/// The match scope for a role filter, and how to describe it.
///
/// Absent means the coaching role when one is chosen, else every role. Uses
/// Coach's own window, so the same filter reads the same matches on both pages.
pub(crate) async fn resolve_scope(
    state: &AppState,
    player: &DotaPlayer,
    requested: Option<&str>,
) -> AppResult<(MatchScope, HeroScope)> {
    let resolved = crate::api::scope::resolve(state, player, requested).await?;
    Ok((
        resolved.matches,
        HeroScope {
            role: resolved.role,
            role_label: resolved.role.map(CoachableRole::label),
            coaching_role: resolved.coaching_role,
            coaching_role_label: resolved.coaching_role.map(CoachableRole::label),
            population: "Ranked + All Pick",
            window: resolved.window,
        },
    ))
}

/// The strongest heroes in the cohort, by meta strength.
fn top_meta(set: &Option<crate::services::hero_meta::HeroMetaSet>, limit: usize) -> Vec<&HeroMeta> {
    let Some(set) = set else {
        return Vec::new();
    };

    let mut heroes: Vec<&HeroMeta> = set.heroes.iter().collect();
    heroes.sort_by(|a, b| {
        b.meta_strength
            .partial_cmp(&a.meta_strength)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.hero_id.cmp(&b.hero_id))
    });
    heroes.truncate(limit);
    heroes
}

/// How each hero compares to the player's own average for their current focus.
///
/// Empty when there is no focus, or when the history is too thin to say —
/// which is most of the time early on, and correctly means the modifier does
/// nothing rather than guessing.
async fn focus_alignment(
    state: &AppState,
    player: &DotaPlayer,
    scope: &MatchScope,
    role: Option<crate::domain::role::CoachableRole>,
) -> (HashMap<i32, f32>, Option<String>) {
    let focus = match repositories::training::active(&state.db, player.id, role).await {
        Ok(Some(focus)) => focus,
        Ok(None) => return (HashMap::new(), None),
        Err(e) => {
            tracing::warn!(error = %e, "training focus lookup failed");
            return (HashMap::new(), None);
        }
    };

    let history = match repositories::player_model::history(&state.db, player.id, scope).await {
        Ok(history) => history,
        Err(e) => {
            tracing::warn!(error = %e, "history lookup for focus alignment failed");
            return (HashMap::new(), None);
        }
    };

    let alignment = training::hero_alignment(focus.measure, focus.pattern_id.as_deref(), &history);
    (alignment, Some(focus.title))
}

/// Peer percentiles for the player's most-played heroes.
///
/// Capped, because each hero is a potential upstream call. The cap is applied
/// to the *most played* heroes, which are also the only ones with a sample big
/// enough for a percentile to be claimed at all — so the budget is spent where
/// it can produce an answer.
async fn benchmark_percentiles(
    state: &AppState,
    player: &DotaPlayer,
    pool: &[HeroPoolEntry],
    lookups: usize,
    scope: &MatchScope,
) -> HashMap<i32, f32> {
    let candidates: Vec<&HeroPoolEntry> = pool
        .iter()
        .filter(|entry| entry.matches >= percentile::MIN_SAMPLE)
        .take(lookups)
        .collect();

    let fetches = candidates.iter().map(|entry| async move {
        let averages =
            repositories::metrics::hero_averages(&state.db, player.id, entry.hero_id, scope).await;
        let averages = match averages {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!(error = %e, hero_id = entry.hero_id, "hero averages failed");
                return None;
            }
        };

        let context = BenchmarkContext {
            hero_id: entry.hero_id,
            role: None,
            rank_tier: player.rank_tier,
            // Hero fit is scored against the player's own bracket: a hero that
            // suits them is a hero that suits them *where they play*.
            bracket: None,
            patch: None,
        };
        let distribution = match state.benchmarks.get_distribution(&context).await {
            Ok(d) => d,
            Err(e) => {
                tracing::debug!(error = %e, hero_id = entry.hero_id, "no distribution for hero");
                return None;
            }
        };

        let values = PlayerValues {
            values: crate::api::handlers::benchmark::player_values(&averages),
            sample: averages.sample,
        };
        let results = benchmarks::compare(&values, &distribution);

        // The mean of the percentiles the engine was willing to claim. A
        // metric it withheld is absent here too, rather than counted as 50.
        let claimed: Vec<f32> = results.iter().filter_map(|r| r.percentile).collect();
        (!claimed.is_empty()).then(|| {
            (
                entry.hero_id,
                claimed.iter().sum::<f32>() / claimed.len() as f32,
            )
        })
    });

    futures::future::join_all(fetches)
        .await
        .into_iter()
        .flatten()
        .collect()
}

async fn load_pool(
    state: &AppState,
    player: &DotaPlayer,
    scope: &MatchScope,
) -> AppResult<(Vec<HeroPoolEntry>, PlayerBaseline)> {
    let rows = repositories::hero_pool::all(&state.db, player.id, RECENT_WINDOW, scope).await?;
    // The baseline every hero is compared against comes from the same scope:
    // measuring a hero against a career average the pool does not contain
    // would make "above your own average" mean two different things at once.
    let stats = repositories::metrics::player_stats_scoped(&state.db, player.id, scope).await?;

    let baseline = PlayerBaseline {
        win_rate: stats.win_rate,
        avg_kda: stats.avg_kda,
    };

    Ok((heroes::build_pool(&rows, baseline), baseline))
}

fn empty_pool_note(pool: &[HeroPoolEntry]) -> Option<String> {
    pool.is_empty()
        .then(|| "Sync some matches first — there is no hero history to read yet.".to_string())
}

async fn load_linked_player(state: &AppState, user: &User) -> AppResult<DotaPlayer> {
    repositories::dota_player::find_by_user_id(&state.db, user.id)
        .await?
        .ok_or(AppError::DotaAccountNotLinked)
}
