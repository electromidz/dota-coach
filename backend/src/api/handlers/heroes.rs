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

use crate::api::extract::{AppQuery, CurrentUser};
use crate::domain::benchmark::{BenchmarkContext, Segment};
use crate::domain::hero::{HeroFit, HeroMeta, HeroMetaContext, HeroPoolEntry, RankBracket};
use crate::domain::player::DotaPlayer;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::services::benchmarks::{self, percentile, PlayerValues};
use crate::services::heroes::{self, FitInput, PlayerBaseline, PoolSummary, RECENT_WINDOW};
use crate::services::training;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct HeroQuery {
    /// Overrides the default taken from `HeroConfig::recommendation_limit`.
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct HeroPoolResponse {
    pub pool: Vec<HeroPoolEntry>,
    pub summary: PoolSummary,
    /// How many recent matches per hero "recent form" is measured over.
    pub recent_window: i64,
    pub note: Option<String>,
}

#[derive(Serialize)]
pub struct RecommendationsResponse {
    pub recommendations: Vec<HeroFit>,
    pub meta: MetaContext,
}

#[derive(Serialize)]
pub struct HeroIntelligenceResponse {
    pub pool: Vec<HeroPoolEntry>,
    pub summary: PoolSummary,
    pub recommendations: Vec<HeroFit>,
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
#[derive(Serialize)]
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
pub async fn pool(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<HeroPoolResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let (pool, _) = load_pool(&state, &player).await?;
    let summary = heroes::summarize(&pool);

    Ok(Json(HeroPoolResponse {
        note: empty_pool_note(&pool),
        pool,
        summary,
        recent_window: RECENT_WINDOW,
    }))
}

/// `GET /api/heroes/recommendations`
pub async fn recommendations(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppQuery(query): AppQuery<HeroQuery>,
) -> AppResult<Json<RecommendationsResponse>> {
    let built = build(&state, &user, query.limit).await?;

    Ok(Json(RecommendationsResponse {
        recommendations: built.recommendations,
        meta: built.meta,
    }))
}

/// `GET /api/hero-intelligence`
pub async fn intelligence(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppQuery(query): AppQuery<HeroQuery>,
) -> AppResult<Json<HeroIntelligenceResponse>> {
    let built = build(&state, &user, query.limit).await?;

    Ok(Json(HeroIntelligenceResponse {
        note: empty_pool_note(&built.pool),
        summary: heroes::summarize(&built.pool),
        pool: built.pool,
        recommendations: built.recommendations,
        meta_leaders: built.meta_leaders,
        meta: built.meta,
        recent_window: RECENT_WINDOW,
    }))
}

pub(crate) struct Built {
    pub pool: Vec<HeroPoolEntry>,
    pub recommendations: Vec<HeroFit>,
    pub meta_leaders: Vec<HeroMeta>,
    pub meta: MetaContext,
}

/// The one pipeline behind the recommendation surfaces.
///
/// Also read by the coaching layer, which needs the same pool and the same
/// scored candidates as evidence — recomputing them there would be a second
/// place for the two answers to drift apart.
pub(crate) async fn build(state: &AppState, user: &User, limit: Option<usize>) -> AppResult<Built> {
    let config = &state.config.heroes;
    let limit = limit.unwrap_or(config.recommendation_limit).clamp(1, 50);

    let player = load_linked_player(state, user).await?;
    let (pool, baseline) = load_pool(state, &player).await?;

    // A provider outage degrades the page rather than failing it: the player's
    // own history is local and still worth scoring on.
    let bracket = player.rank_tier.and_then(RankBracket::from_rank_tier);
    let context = HeroMetaContext::for_bracket(bracket);
    let (meta_set, outage_note) = match state.hero_meta.get_hero_meta(&context).await {
        Ok(set) => (Some(set), None),
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

    let benchmarks = benchmark_percentiles(state, &player, &pool, config.benchmark_lookups).await;

    // Training-focus compatibility, as a modifier on the finished score. Read
    // only: the heroes page reflects whatever focus is set, it never sets one.
    let (alignment, focus_title) = focus_alignment(state, &player).await;

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

    let mut recommendations = heroes::rank(fits);
    recommendations.truncate(limit);

    Ok(Built {
        pool,
        recommendations,
        meta_leaders: top_meta(&meta_set, limit).into_iter().cloned().collect(),
        meta: meta_context,
    })
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
) -> (HashMap<i32, f32>, Option<String>) {
    let focus = match repositories::training::active(&state.db, player.id).await {
        Ok(Some(focus)) => focus,
        Ok(None) => return (HashMap::new(), None),
        Err(e) => {
            tracing::warn!(error = %e, "training focus lookup failed");
            return (HashMap::new(), None);
        }
    };

    let history = match repositories::player_model::history(&state.db, player.id).await {
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
) -> HashMap<i32, f32> {
    let candidates: Vec<&HeroPoolEntry> = pool
        .iter()
        .filter(|entry| entry.matches >= percentile::MIN_SAMPLE)
        .take(lookups)
        .collect();

    let fetches = candidates.iter().map(|entry| async move {
        let averages =
            repositories::metrics::hero_averages(&state.db, player.id, entry.hero_id).await;
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
) -> AppResult<(Vec<HeroPoolEntry>, PlayerBaseline)> {
    let rows = repositories::hero_pool::all(&state.db, player.id, RECENT_WINDOW).await?;
    let stats = repositories::metrics::player_stats(&state.db, player.id).await?;

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
