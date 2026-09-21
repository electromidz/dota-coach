//! OpenDota implementation of [`BenchmarkProvider`].
//!
//! Scope, re-verified against the live endpoint:
//! `GET /benchmarks?hero_id=N` returns 11 percentile buckets (p0.1 … p0.99)
//! across 10 metrics, and **does** accept a `bracket` parameter — 1 Herald to
//! 8 Immortal, omitted for all ranks — documented in OpenDota's own OpenAPI
//! response and confirmed to return materially different buckets per bracket
//! (Anti-Mage gold-per-minute p50: 546 Herald, 645 all ranks, 716 Divine).
//!
//! An earlier reading of this API concluded rank segmentation was impossible.
//! It was testing the wrong parameter names: `rank` and `lane_role` are
//! ignored and do return byte-identical buckets. `bracket` is the one that
//! works. Role and patch remain genuinely unavailable.
//!
//! Two consequences shape the code below:
//!
//!   - Thin brackets answer with the full bucket list and `null` in every
//!     value — bracket 8 for most heroes. That is "no data", not "zero", and
//!     it must not deserialize into a distribution.
//!   - When a bracket has nothing, the all-ranks distribution is a usable
//!     substitute but a *different* peer group, so the fallback is recorded in
//!     [`Distribution::bracket`] rather than passed off as what was asked for.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use sqlx::PgPool;

use super::{BenchmarkError, BenchmarkProvider, Distribution};
use crate::domain::benchmark::{
    BenchmarkContext, BenchmarkMetric, Bucket, ResolvedBracket, Segment,
};
use crate::domain::hero::RankBracket;
use crate::repositories;
use crate::repositories::benchmark::ALL_RANKS;

pub struct OpenDotaBenchmarkProvider {
    http: Client,
    base_url: String,
    api_key: Option<String>,
    /// Snapshots are cached in Postgres rather than in memory: the
    /// distribution for a hero is identical for every user, and it should
    /// survive a restart rather than costing each deploy a fresh stampede.
    db: PgPool,
    ttl_hours: i64,
}

impl OpenDotaBenchmarkProvider {
    pub fn new(
        http: Client,
        base_url: &str,
        api_key: Option<String>,
        db: PgPool,
        ttl_hours: i64,
    ) -> Arc<Self> {
        Arc::new(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            db,
            ttl_hours,
        })
    }

    async fn fetch(
        &self,
        hero_id: i32,
        bracket: Option<RankBracket>,
    ) -> Result<serde_json::Value, BenchmarkError> {
        let url = format!("{}/benchmarks", self.base_url);
        let mut request = self.http.get(&url).query(&[("hero_id", hero_id)]);
        // Omitted entirely for all ranks — the endpoint documents omission as
        // the way to ask for every bracket, and there is no sentinel for it.
        if let Some(bracket) = bracket {
            request = request.query(&[("bracket", bracket.index())]);
        }
        if let Some(key) = &self.api_key {
            request = request.query(&[("api_key", key)]);
        }

        let response = request
            .send()
            .await
            .map_err(|e| BenchmarkError::Unavailable(e.to_string()))?;

        let status = response.status();
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(BenchmarkError::RateLimited);
        }
        if status == StatusCode::NOT_FOUND {
            return Err(BenchmarkError::NotFound);
        }
        if !status.is_success() {
            return Err(BenchmarkError::Unavailable(format!("HTTP {status}")));
        }

        response
            .json()
            .await
            .map_err(|e| BenchmarkError::InvalidResponse(e.to_string()))
    }

    /// Read-through cache for one hero in one bracket.
    ///
    /// A cache failure is never fatal — it degrades to a network call, which
    /// is slower but correct. A *fetch* failure is fatal to this attempt and
    /// propagates, so the caller can decide whether a fallback is appropriate.
    async fn buckets_for(
        &self,
        hero_id: i32,
        bracket: Option<RankBracket>,
    ) -> Result<HashMap<BenchmarkMetric, Vec<Bucket>>, BenchmarkError> {
        let key = bracket.map_or(ALL_RANKS, |b| b.index() as i16);

        let cached = repositories::benchmark::fresh_snapshot(
            &self.db,
            "opendota",
            hero_id,
            key,
            self.ttl_hours,
        )
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "benchmark cache read failed");
            None
        });

        let payload = match cached {
            Some(payload) => payload,
            None => {
                let fetched = self.fetch(hero_id, bracket).await?;
                // Cached even when it turns out to be empty: a bracket the
                // provider has no data for is a stable fact, and re-asking on
                // every page view would spend the rate limit learning it again.
                if let Err(e) = repositories::benchmark::store_snapshot(
                    &self.db, "opendota", hero_id, key, &fetched,
                )
                .await
                {
                    tracing::warn!(error = %e, "benchmark cache write failed");
                }
                fetched
            }
        };

        parse_buckets(&payload)
    }
}

#[async_trait]
impl BenchmarkProvider for OpenDotaBenchmarkProvider {
    async fn get_distribution(
        &self,
        context: &BenchmarkContext,
    ) -> Result<Distribution, BenchmarkError> {
        // An explicit request wins over the player's own rank. It is still a
        // *request*: if the provider publishes nothing there, the fallback
        // below records that it fell back, exactly as it does for a rank-derived
        // bracket. A bracket the caller named is never quietly served from
        // another one's numbers.
        let requested = match context.bracket {
            Some(bracket) => ResolvedBracket::exact(bracket),
            None => ResolvedBracket::requested_for(context.rank_tier),
        };

        // The optimistic attempt. `used` is `None` for an unranked player, in
        // which case this is already the all-ranks call and there is nothing
        // to fall back to.
        let buckets = self.buckets_for(context.hero_id, requested.used).await?;

        let (buckets, bracket) = match (buckets.is_empty(), requested.used) {
            // The provider publishes nothing for this hero in this bracket —
            // routine above Divine. All ranks is a real answer to a slightly
            // different question, which is better than no comparison, and the
            // difference travels with it.
            (true, Some(asked)) => (
                self.buckets_for(context.hero_id, None).await?,
                ResolvedBracket::fell_back_from(asked),
            ),
            _ => (buckets, requested),
        };

        if buckets.is_empty() {
            return Err(BenchmarkError::NotFound);
        }

        Ok(Distribution {
            buckets,
            // Rank only counts as segmented when a bracket genuinely backed the
            // numbers. On the fallback path this is hero alone, and the UI's
            // "not segmented by rank" caveat correctly reappears.
            segmented_by: if bracket.is_rank_segmented() {
                vec![Segment::Hero, Segment::RankBracket]
            } else {
                vec![Segment::Hero]
            },
            sample_size: None,
            bracket,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawBucket {
    percentile: f32,
    /// `Option`, and it matters: a bracket the provider has no data for still
    /// answers with all eleven buckets and `null` in every value. Typed as
    /// `f32` this fails to deserialize and silently drops the whole metric,
    /// which reads downstream as "this hero has no gold-per-minute benchmark"
    /// rather than "this bracket is empty".
    ///
    /// Integers and floats both appear across metrics; `f32` covers both.
    value: Option<f32>,
}

/// Turn the provider payload into buckets per metric.
///
/// A metric that is missing, empty or unparseable is skipped rather than
/// defaulted — an absent distribution must reach the engine as absent. An
/// empty map is a legitimate result here, not an error: it is how a bracket
/// with no data is reported, and the caller decides what to do about it.
fn parse_buckets(
    payload: &serde_json::Value,
) -> Result<HashMap<BenchmarkMetric, Vec<Bucket>>, BenchmarkError> {
    let result = payload
        .get("result")
        .ok_or_else(|| BenchmarkError::InvalidResponse("no `result` object".into()))?;

    let mut buckets = HashMap::new();

    for metric in BenchmarkMetric::ALL {
        let Some(raw) = result.get(metric.provider_key()) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_value::<Vec<RawBucket>>(raw.clone()) else {
            tracing::debug!(
                metric = metric.provider_key(),
                "unparseable benchmark bucket list"
            );
            continue;
        };

        let list: Vec<Bucket> = parsed
            .into_iter()
            .filter_map(|b| {
                // A null bucket is missing data; a zero-valued one means the
                // provider had nothing there either. Neither is a real
                // "0 gold per minute" cohort.
                let value = b.value.filter(|v| *v > 0.0)?;
                Some(Bucket {
                    percentile: b.percentile,
                    value,
                })
            })
            .collect();

        if !list.is_empty() {
            buckets.insert(metric, list);
        }
    }

    Ok(buckets)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = include_str!("../../../tests/fixtures/opendota_benchmarks.json");

    /// Captured live from `/benchmarks?hero_id=1&bracket=8`. Every bucket is
    /// present and every value is `null` — the shape a thin bracket answers
    /// with, and the one that used to take the whole metric down with it.
    const EMPTY_BRACKET: &str =
        include_str!("../../../tests/fixtures/opendota_benchmarks_bracket_8.json");

    fn parse(json: &str) -> Result<HashMap<BenchmarkMetric, Vec<Bucket>>, BenchmarkError> {
        parse_buckets(&serde_json::from_str::<serde_json::Value>(json).unwrap())
    }

    #[test]
    fn a_real_payload_parses_into_per_metric_buckets() {
        let buckets = parse(PAYLOAD).unwrap();

        let gpm = buckets.get(&BenchmarkMetric::GoldPerMin).unwrap();
        assert_eq!(gpm.len(), 11);
        assert!((gpm[0].percentile - 0.1).abs() < f32::EPSILON);
        assert_eq!(gpm[0].value, 361.0);
    }

    #[test]
    fn a_bracket_with_no_data_parses_to_nothing_rather_than_failing() {
        // The caller retries at all ranks on an empty map. It cannot do that
        // if a payload of nulls surfaces as a decode error, and it must not
        // read the nulls as zeroes.
        let buckets = parse(EMPTY_BRACKET).unwrap();
        assert!(buckets.is_empty());
    }

    #[test]
    fn a_null_bucket_does_not_take_the_rest_of_the_metric_with_it() {
        let buckets = parse(
            r#"{"result":{"gold_per_min":[
                 {"percentile":0.1,"value":null},
                 {"percentile":0.5,"value":500}
               ]}}"#,
        )
        .unwrap();

        let gpm = buckets.get(&BenchmarkMetric::GoldPerMin).unwrap();
        assert_eq!(gpm.len(), 1);
        assert_eq!(gpm[0].value, 500.0);
    }

    #[test]
    fn a_payload_without_a_result_object_is_an_invalid_response() {
        assert!(matches!(
            parse(r#"{"hero_id":35}"#),
            Err(BenchmarkError::InvalidResponse(_))
        ));
    }

    #[test]
    fn an_empty_result_yields_no_buckets() {
        assert!(parse(r#"{"result":{}}"#).unwrap().is_empty());
    }

    #[test]
    fn zero_valued_buckets_are_dropped_rather_than_read_as_a_real_cohort() {
        let buckets = parse(
            r#"{"result":{"gold_per_min":[
                 {"percentile":0.1,"value":0},
                 {"percentile":0.5,"value":500}
               ]}}"#,
        )
        .unwrap();

        let gpm = buckets.get(&BenchmarkMetric::GoldPerMin).unwrap();
        assert_eq!(gpm.len(), 1);
        assert_eq!(gpm[0].value, 500.0);
    }

    #[test]
    fn an_unparseable_metric_is_skipped_without_losing_the_others() {
        let buckets = parse(
            r#"{"result":{
                 "gold_per_min":[{"percentile":0.5,"value":500}],
                 "xp_per_min":"not a list"
               }}"#,
        )
        .unwrap();

        assert!(buckets.contains_key(&BenchmarkMetric::GoldPerMin));
        assert!(!buckets.contains_key(&BenchmarkMetric::XpPerMin));
    }

    #[test]
    fn a_rank_segmented_distribution_says_so_and_a_fallback_does_not() {
        // The two `segmented_by` values the trait impl chooses between. Stated
        // as a test because the UI's rank caveat hangs off exactly this, and
        // getting it backwards would claim a peer group that was never used.
        let divine = ResolvedBracket::exact(RankBracket::Divine);
        assert!(divine.is_rank_segmented());
        assert!(!divine.fell_back);

        let fallback = ResolvedBracket::fell_back_from(RankBracket::Immortal);
        assert!(!fallback.is_rank_segmented());
        assert!(fallback.fell_back);
        assert_eq!(fallback.requested, Some(RankBracket::Immortal));

        // An unranked player asked for nothing, so nothing was lost.
        let unranked = ResolvedBracket::requested_for(None);
        assert!(!unranked.is_rank_segmented());
        assert!(!unranked.fell_back);

        assert_eq!(
            ResolvedBracket::requested_for(Some(54)).used,
            Some(RankBracket::Legend),
            "rank_tier is medal * 10 + stars"
        );
    }

    #[test]
    fn an_explicitly_asked_for_bracket_outranks_the_players_own() {
        // The resolution `get_distribution` performs before any network call.
        // An Archon player looking at Ancient must be asking the provider for
        // Ancient, not for Archon with a different label on it.
        let context = BenchmarkContext {
            hero_id: 35,
            role: None,
            rank_tier: Some(41), // Archon 1
            bracket: Some(RankBracket::Ancient),
            patch: None,
        };

        let requested = match context.bracket {
            Some(bracket) => ResolvedBracket::exact(bracket),
            None => ResolvedBracket::requested_for(context.rank_tier),
        };

        assert_eq!(requested.used, Some(RankBracket::Ancient));
        assert_eq!(requested.label, "Ancient");
        // And the fallback still names what was asked for, not the medal.
        let fallback = ResolvedBracket::fell_back_from(requested.used.unwrap());
        assert_eq!(fallback.requested, Some(RankBracket::Ancient));
        assert!(!fallback.is_rank_segmented());
    }
}
