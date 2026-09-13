//! OpenDota implementation of [`BenchmarkProvider`].
//!
//! Scope, verified against the live endpoint before writing this:
//! `GET /benchmarks?hero_id=N` returns 11 percentile buckets (p0.1 … p0.99)
//! across 10 metrics, **segmented by hero only**. It carries no rank bracket,
//! no role, no patch, and no sample size.
//!
//! That is a real limitation, not a temporary gap, so it is reported rather
//! than papered over: [`Distribution::segmented_by`] says `[Hero]` and
//! `sample_size` stays `None`. A STRATZ provider can later fill both in
//! without the engine above changing.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use sqlx::PgPool;

use super::{BenchmarkError, BenchmarkProvider, Distribution};
use crate::domain::benchmark::{BenchmarkContext, BenchmarkMetric, Bucket, Segment};
use crate::repositories;

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

    async fn fetch(&self, hero_id: i32) -> Result<serde_json::Value, BenchmarkError> {
        let url = format!("{}/benchmarks", self.base_url);
        let mut request = self.http.get(&url).query(&[("hero_id", hero_id)]);
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
}

#[async_trait]
impl BenchmarkProvider for OpenDotaBenchmarkProvider {
    async fn get_distribution(
        &self,
        context: &BenchmarkContext,
    ) -> Result<Distribution, BenchmarkError> {
        let cached = repositories::benchmark::fresh_snapshot(
            &self.db,
            "opendota",
            context.hero_id,
            self.ttl_hours,
        )
        .await
        .unwrap_or_else(|e| {
            // A cache miss must never be fatal; fall through to the network.
            tracing::warn!(error = %e, "benchmark cache read failed");
            None
        });

        let payload = match cached {
            Some(payload) => payload,
            None => {
                let fetched = self.fetch(context.hero_id).await?;
                if let Err(e) = repositories::benchmark::store_snapshot(
                    &self.db,
                    "opendota",
                    context.hero_id,
                    &fetched,
                )
                .await
                {
                    tracing::warn!(error = %e, "benchmark cache write failed");
                }
                fetched
            }
        };

        parse_distribution(&payload)
    }
}

#[derive(Debug, Deserialize)]
struct RawBucket {
    percentile: f32,
    /// Numeric for most metrics, but the provider has been seen to return
    /// integers and floats interchangeably; `serde_json::Value` would be
    /// stricter than useful here, so f32 covers both.
    value: f32,
}

/// Turn the provider payload into buckets per metric.
///
/// A metric that is missing, empty or unparseable is skipped rather than
/// defaulted — an absent distribution must reach the engine as absent.
fn parse_distribution(payload: &serde_json::Value) -> Result<Distribution, BenchmarkError> {
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
            // A zero-valued bucket means the provider had nothing there; it is
            // not a real "0 gold per minute" cohort.
            .filter(|b| b.value > 0.0)
            .map(|b| Bucket {
                percentile: b.percentile,
                value: b.value,
            })
            .collect();

        if !list.is_empty() {
            buckets.insert(metric, list);
        }
    }

    if buckets.is_empty() {
        return Err(BenchmarkError::NotFound);
    }

    Ok(Distribution {
        buckets,
        // Hero, and only hero. See the module docs.
        segmented_by: vec![Segment::Hero],
        sample_size: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = include_str!("../../../tests/fixtures/opendota_benchmarks.json");

    #[test]
    fn a_real_payload_parses_into_per_metric_buckets() {
        let value: serde_json::Value = serde_json::from_str(PAYLOAD).unwrap();
        let dist = parse_distribution(&value).unwrap();

        let gpm = dist.buckets.get(&BenchmarkMetric::GoldPerMin).unwrap();
        assert_eq!(gpm.len(), 11);
        assert!((gpm[0].percentile - 0.1).abs() < f32::EPSILON);
        assert_eq!(gpm[0].value, 361.0);
    }

    #[test]
    fn the_provider_reports_hero_segmentation_only() {
        let value: serde_json::Value = serde_json::from_str(PAYLOAD).unwrap();
        let dist = parse_distribution(&value).unwrap();

        // It cannot segment by rank, so it must not claim to.
        assert_eq!(dist.segmented_by, vec![Segment::Hero]);
        // And it reports no cohort size, so none is invented.
        assert_eq!(dist.sample_size, None);
    }

    #[test]
    fn a_payload_without_a_result_object_is_an_invalid_response() {
        let value: serde_json::Value = serde_json::from_str(r#"{"hero_id":35}"#).unwrap();
        assert!(matches!(
            parse_distribution(&value),
            Err(BenchmarkError::InvalidResponse(_))
        ));
    }

    #[test]
    fn an_empty_result_is_not_found_rather_than_an_empty_distribution() {
        let value: serde_json::Value = serde_json::from_str(r#"{"result":{}}"#).unwrap();
        assert!(matches!(
            parse_distribution(&value),
            Err(BenchmarkError::NotFound)
        ));
    }

    #[test]
    fn zero_valued_buckets_are_dropped_rather_than_read_as_a_real_cohort() {
        let value: serde_json::Value = serde_json::from_str(
            r#"{"result":{"gold_per_min":[
                 {"percentile":0.1,"value":0},
                 {"percentile":0.5,"value":500}
               ]}}"#,
        )
        .unwrap();

        let dist = parse_distribution(&value).unwrap();
        let gpm = dist.buckets.get(&BenchmarkMetric::GoldPerMin).unwrap();

        assert_eq!(gpm.len(), 1);
        assert_eq!(gpm[0].value, 500.0);
    }

    #[test]
    fn an_unparseable_metric_is_skipped_without_losing_the_others() {
        let value: serde_json::Value = serde_json::from_str(
            r#"{"result":{
                 "gold_per_min":[{"percentile":0.5,"value":500}],
                 "xp_per_min":"not a list"
               }}"#,
        )
        .unwrap();

        let dist = parse_distribution(&value).unwrap();
        assert!(dist.buckets.contains_key(&BenchmarkMetric::GoldPerMin));
        assert!(!dist.buckets.contains_key(&BenchmarkMetric::XpPerMin));
    }
}
