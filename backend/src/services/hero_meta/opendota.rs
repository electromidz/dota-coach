//! OpenDota implementation of [`HeroMetaProvider`].
//!
//! Scope, verified against the live endpoint before this was written:
//! `GET /heroStats` returns one object per hero carrying
//!
//!   - `id`, `localized_name`, `roles`
//!   - `{1..8}_pick` / `{1..8}_win` — picks and wins **per rank bracket**,
//!     where the leading digit is the medal (1 = Herald … 8 = Immortal)
//!   - `pub_pick` / `pub_win` — the same across all brackets
//!   - `pub_pick_trend` / `pub_win_trend` — seven buckets of recent public
//!     matches, all brackets
//!   - `pro_pick` / `pro_win` / `pro_ban` — professional matches only
//!
//! Three consequences are honoured rather than hidden:
//!
//!   1. Rank segmentation is real, so [`Segment::RankBracket`] is reported.
//!   2. There is no patch or role segmentation, so neither is claimed. The
//!      `roles` array is a hero's *tags*, not a per-role win rate.
//!   3. `pro_ban` is the only ban figure, and it describes a population this
//!      product does not coach. It is not carried into the domain model.

use std::sync::Arc;

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use sqlx::PgPool;

use super::strength::{self, MetaWeights};
use super::{HeroMetaError, HeroMetaProvider, HeroMetaSet};
use crate::domain::benchmark::Segment;
use crate::domain::hero::{HeroMeta, HeroMetaContext, RankBracket};
use crate::repositories;

/// Cache key for the one document OpenDota publishes. Every bracket is carried
/// in the same payload, so there is nothing else to vary on.
const CONTEXT_KEY: &str = "hero_stats";
const PROVIDER: &str = "opendota";

pub struct OpenDotaHeroMetaProvider {
    http: Client,
    base_url: String,
    api_key: Option<String>,
    /// Postgres rather than memory, for the same reasons as the benchmark
    /// cache: the document is identical for every user and should survive a
    /// restart instead of costing each deploy a fresh fetch.
    db: PgPool,
    ttl_hours: i64,
    weights: MetaWeights,
}

impl OpenDotaHeroMetaProvider {
    pub fn new(
        http: Client,
        base_url: &str,
        api_key: Option<String>,
        db: PgPool,
        ttl_hours: i64,
        weights: MetaWeights,
    ) -> Arc<Self> {
        Arc::new(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            db,
            ttl_hours,
            weights,
        })
    }

    async fn fetch(&self) -> Result<serde_json::Value, HeroMetaError> {
        let url = format!("{}/heroStats", self.base_url);
        let mut request = self.http.get(&url);
        if let Some(key) = &self.api_key {
            request = request.query(&[("api_key", key)]);
        }

        let response = request
            .send()
            .await
            .map_err(|e| HeroMetaError::Unavailable(e.to_string()))?;

        let status = response.status();
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(HeroMetaError::RateLimited);
        }
        if status == StatusCode::NOT_FOUND {
            return Err(HeroMetaError::NotFound);
        }
        if !status.is_success() {
            return Err(HeroMetaError::Unavailable(format!("HTTP {status}")));
        }

        response
            .json()
            .await
            .map_err(|e| HeroMetaError::InvalidResponse(e.to_string()))
    }
}

#[async_trait]
impl HeroMetaProvider for OpenDotaHeroMetaProvider {
    async fn get_hero_meta(&self, context: &HeroMetaContext) -> Result<HeroMetaSet, HeroMetaError> {
        let cached = repositories::hero_meta::fresh_snapshot(
            &self.db,
            PROVIDER,
            CONTEXT_KEY,
            self.ttl_hours,
        )
        .await
        .unwrap_or_else(|e| {
            // A cache miss must never be fatal; fall through to the network.
            tracing::warn!(error = %e, "hero meta cache read failed");
            None
        });

        let payload = match cached {
            Some(payload) => payload,
            None => {
                let fetched = self.fetch().await?;
                if let Err(e) = repositories::hero_meta::store_snapshot(
                    &self.db,
                    PROVIDER,
                    CONTEXT_KEY,
                    &fetched,
                )
                .await
                {
                    tracing::warn!(error = %e, "hero meta cache write failed");
                }
                fetched
            }
        };

        let mut set = parse_hero_meta(&payload, context.bracket)?;
        strength::score_all(&mut set.heroes, self.weights);
        Ok(set)
    }
}

/// One hero as OpenDota reports it. Everything is optional: the endpoint has
/// added and removed fields before, and a missing figure must read as absent
/// rather than as a zero.
#[derive(Debug, Deserialize)]
struct RawHero {
    id: i32,
    localized_name: Option<String>,
    #[serde(default)]
    roles: Vec<String>,
    pub_pick: Option<i64>,
    pub_win: Option<i64>,
    #[serde(default)]
    pub_pick_trend: Vec<i64>,
    #[serde(default)]
    pub_win_trend: Vec<i64>,
    /// `1_pick`, `1_win`, … are not valid Rust identifiers, so the bracket
    /// columns are read from the untyped rest of the object.
    #[serde(flatten)]
    rest: serde_json::Map<String, serde_json::Value>,
}

impl RawHero {
    fn bracket_counts(&self, bracket: RankBracket) -> Option<(i64, i64)> {
        let index = bracket.index();
        let picks = self.rest.get(&format!("{index}_pick"))?.as_i64()?;
        let wins = self.rest.get(&format!("{index}_win"))?.as_i64()?;
        Some((picks, wins))
    }

    /// Change in win rate between the most recent trend buckets and the whole
    /// trend window.
    ///
    /// The buckets are counts, not rates, so they are re-divided rather than
    /// averaged — a light week would otherwise weigh as much as a heavy one.
    /// The final bucket is usually partial, so the recent side spans two.
    fn trend(&self) -> Option<f32> {
        let picks = &self.pub_pick_trend;
        let wins = &self.pub_win_trend;
        if picks.len() != wins.len() || picks.len() < 3 {
            return None;
        }

        let rate = |p: i64, w: i64| (p > 0).then(|| w as f32 / p as f32);

        let overall = rate(picks.iter().sum(), wins.iter().sum())?;
        let tail = picks.len() - 2;
        let recent = rate(picks[tail..].iter().sum(), wins[tail..].iter().sum())?;

        Some(recent - overall)
    }
}

/// Turn the provider payload into a cohort.
///
/// When a bracket is asked for but carries no picks at all — which is the live
/// state of the Immortal columns today — the all-bracket figures are used and
/// the drop in segmentation is reported, rather than returning a roster of
/// zeroes that would score as a real cohort.
fn parse_hero_meta(
    payload: &serde_json::Value,
    bracket: Option<RankBracket>,
) -> Result<HeroMetaSet, HeroMetaError> {
    let raw: Vec<RawHero> = serde_json::from_value(payload.clone())
        .map_err(|e| HeroMetaError::InvalidResponse(e.to_string()))?;

    if raw.is_empty() {
        return Err(HeroMetaError::NotFound);
    }

    let bracket_has_data = bracket.is_some_and(|b| {
        raw.iter()
            .filter_map(|h| h.bracket_counts(b))
            .any(|(picks, _)| picks > 0)
    });
    let used_bracket = bracket.filter(|_| bracket_has_data);

    let heroes: Vec<HeroMeta> = raw
        .iter()
        .filter_map(|hero| {
            let (picks, wins) = match used_bracket {
                Some(b) => hero.bracket_counts(b)?,
                None => (hero.pub_pick?, hero.pub_win?),
            };
            // A hero nobody picked in this cohort has no win rate, and a
            // zero would be read as "loses every game".
            if picks <= 0 {
                return None;
            }

            Some(HeroMeta {
                hero_id: hero.id,
                hero_name: hero
                    .localized_name
                    .clone()
                    .unwrap_or_else(|| crate::services::dota::fallback_hero_name(hero.id)),
                roles: hero.roles.clone(),
                picks,
                wins,
                win_rate: wins as f32 / picks as f32,
                // Filled by the scorer, which needs the whole cohort.
                pick_rate: 0.0,
                trend: hero.trend(),
                meta_strength: 0.0,
                bracket: used_bracket,
            })
        })
        .collect();

    if heroes.is_empty() {
        return Err(HeroMetaError::NotFound);
    }

    let (segmented_by, note) = match (bracket, used_bracket) {
        (_, Some(_)) => (
            vec![Segment::RankBracket],
            // The trend arrays are published across all brackets, so a
            // bracket-segmented cohort still carries an all-bracket trend.
            Some("Win and pick rates are for your rank bracket. The recent trend is published across all brackets.".to_string()),
        ),
        (Some(asked), None) => (
            Vec::new(),
            Some(format!(
                "OpenDota publishes no {} data yet, so this is every bracket combined.",
                asked.label()
            )),
        ),
        (None, None) => (Vec::new(), None),
    };

    Ok(HeroMetaSet {
        heroes,
        segmented_by,
        bracket: used_bracket,
        source: "OpenDota",
        note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = include_str!("../../../tests/fixtures/opendota_hero_stats.json");

    fn payload() -> serde_json::Value {
        serde_json::from_str(PAYLOAD).unwrap()
    }

    #[test]
    fn a_real_payload_parses_into_a_cohort() {
        let set = parse_hero_meta(&payload(), None).unwrap();

        assert_eq!(set.heroes.len(), 6);
        let am = set.heroes.iter().find(|h| h.hero_id == 1).unwrap();
        assert_eq!(am.hero_name, "Anti-Mage");
        assert!(am.roles.contains(&"Carry".to_string()));
        assert!((am.win_rate - 0.498).abs() < 0.01);
    }

    #[test]
    fn a_bracket_request_uses_that_brackets_columns() {
        let all = parse_hero_meta(&payload(), None).unwrap();
        let legend = parse_hero_meta(&payload(), Some(RankBracket::Legend)).unwrap();

        let all_am = all.heroes.iter().find(|h| h.hero_id == 1).unwrap();
        let legend_am = legend.heroes.iter().find(|h| h.hero_id == 1).unwrap();

        assert!(legend_am.picks < all_am.picks, "a bracket is a subset");
        assert_eq!(legend_am.bracket, Some(RankBracket::Legend));
        assert_eq!(legend.segmented_by, vec![Segment::RankBracket]);
    }

    #[test]
    fn an_all_bracket_cohort_does_not_claim_rank_segmentation() {
        let set = parse_hero_meta(&payload(), None).unwrap();
        assert!(set.segmented_by.is_empty());
        assert_eq!(set.bracket, None);
    }

    #[test]
    fn a_bracket_with_no_published_data_falls_back_and_says_so() {
        // Live state today: every `8_pick` is 0.
        let set = parse_hero_meta(&payload(), Some(RankBracket::Immortal)).unwrap();

        assert_eq!(
            set.bracket, None,
            "must not report a bracket it did not use"
        );
        assert!(set.segmented_by.is_empty());
        assert!(set.note.as_deref().unwrap().contains("Immortal"));
        // And the cohort is still usable.
        assert_eq!(set.heroes.len(), 6);
    }

    #[test]
    fn the_trend_compares_the_recent_buckets_against_the_window() {
        let set = parse_hero_meta(&payload(), None).unwrap();
        // Every hero in the fixture carries both trend arrays.
        assert!(set.heroes.iter().all(|h| h.trend.is_some()));
        assert!(set.heroes.iter().all(|h| h.trend.unwrap().abs() < 0.2));
    }

    #[test]
    fn a_hero_with_no_picks_in_the_cohort_is_omitted_not_scored_at_zero() {
        let value = serde_json::json!([
            {"id": 1, "localized_name": "Anti-Mage", "pub_pick": 100, "pub_win": 50},
            {"id": 2, "localized_name": "Axe", "pub_pick": 0, "pub_win": 0},
        ]);

        let set = parse_hero_meta(&value, None).unwrap();
        assert_eq!(set.heroes.len(), 1);
        assert_eq!(set.heroes[0].hero_id, 1);
    }

    #[test]
    fn a_hero_with_no_name_keeps_its_id_rather_than_going_blank() {
        let value = serde_json::json!([{"id": 42, "pub_pick": 100, "pub_win": 50}]);
        let set = parse_hero_meta(&value, None).unwrap();
        assert_eq!(set.heroes[0].hero_name, "Hero 42");
    }

    #[test]
    fn a_payload_that_is_not_a_hero_list_is_an_invalid_response() {
        let value = serde_json::json!({"error": "nope"});
        assert!(matches!(
            parse_hero_meta(&value, None),
            Err(HeroMetaError::InvalidResponse(_))
        ));
    }

    #[test]
    fn an_empty_roster_is_not_found_rather_than_an_empty_cohort() {
        let value = serde_json::json!([]);
        assert!(matches!(
            parse_hero_meta(&value, None),
            Err(HeroMetaError::NotFound)
        ));
    }

    #[test]
    fn a_short_trend_window_yields_no_trend_rather_than_a_guess() {
        let value = serde_json::json!([{
            "id": 1, "pub_pick": 100, "pub_win": 50,
            "pub_pick_trend": [10, 10], "pub_win_trend": [5, 5],
        }]);

        let set = parse_hero_meta(&value, None).unwrap();
        assert_eq!(set.heroes[0].trend, None);
    }

    #[test]
    fn mismatched_trend_arrays_yield_no_trend() {
        let value = serde_json::json!([{
            "id": 1, "pub_pick": 100, "pub_win": 50,
            "pub_pick_trend": [10, 10, 10], "pub_win_trend": [5, 5],
        }]);

        let set = parse_hero_meta(&value, None).unwrap();
        assert_eq!(set.heroes[0].trend, None);
    }
}
