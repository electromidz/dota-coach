//! OpenDota implementation of [`ItemTimingProvider`].
//!
//! Verified against the live endpoint before this was written (no key; the
//! usual OpenDota rate limit): `GET /scenarios/itemTimings?item=<key>&hero_id=<id>`
//! returns `[{ hero_id, item, time, games, wins }]`, where `time` is the
//! upper edge of a bucket in seconds and `games` / `wins` are strings. All
//! ranks combined; no bracket segmentation, which the UI says.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::RwLock;

use super::{CoreItem, ItemTimingError, ItemTimingProvider, TimingBucket};

const TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Deserialize)]
struct Row {
    time: i32,
    /// Sent as a string.
    games: serde_json::Value,
}

fn count(value: &serde_json::Value) -> i64 {
    match value {
        serde_json::Value::String(s) => s.parse().unwrap_or(0),
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(0),
        _ => 0,
    }
}

pub struct OpenDotaItemTimingProvider {
    http: Client,
    base_url: String,
    api_key: Option<String>,
    cache: RwLock<HashMap<(i32, CoreItem), (Instant, Vec<TimingBucket>)>>,
}

impl OpenDotaItemTimingProvider {
    pub fn new(http: Client, base_url: &str, api_key: Option<String>) -> Arc<Self> {
        Arc::new(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            cache: RwLock::new(HashMap::new()),
        })
    }
}

#[async_trait]
impl ItemTimingProvider for OpenDotaItemTimingProvider {
    async fn timings(&self, hero_id: i32, item: CoreItem) -> Result<Vec<TimingBucket>, ItemTimingError> {
        if let Some((at, buckets)) = self.cache.read().await.get(&(hero_id, item)) {
            if at.elapsed() < TTL {
                return Ok(buckets.clone());
            }
        }
        let mut request = self
            .http
            .get(format!("{}/scenarios/itemTimings", self.base_url))
            .query(&[("item", item.key()), ("hero_id", &hero_id.to_string())])
            .timeout(Duration::from_secs(10));
        if let Some(key) = &self.api_key {
            request = request.query(&[("api_key", key)]);
        }
        let response = request
            .send()
            .await
            .map_err(|e| ItemTimingError::Unavailable(e.to_string()))?;
        if !response.status().is_success() {
            return Err(ItemTimingError::Unavailable(format!("status {}", response.status())));
        }
        let body = response
            .bytes()
            .await
            .map_err(|e| ItemTimingError::Unavailable(e.to_string()))?;
        let rows: Vec<Row> = serde_json::from_slice(&body)
            .map_err(|e| ItemTimingError::InvalidResponse(e.to_string()))?;
        let buckets: Vec<TimingBucket> = rows
            .into_iter()
            .map(|r| TimingBucket { seconds: r.time, games: count(&r.games) })
            .collect();
        self.cache
            .write()
            .await
            .insert((hero_id, item), (Instant::now(), buckets.clone()));
        Ok(buckets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_live_shape_parses_with_string_counts() {
        let rows: Vec<Row> = serde_json::from_str(
            r#"[{"hero_id":9,"item":"black_king_bar","time":1500,"games":"10","wins":"6"}]"#,
        )
        .unwrap();
        assert_eq!(rows[0].time, 1500);
        assert_eq!(count(&rows[0].games), 10);
    }

    /// Hits the real API. `cargo test -- --ignored live_item_timings` to run.
    #[tokio::test]
    #[ignore]
    async fn live_item_timings() {
        let p = OpenDotaItemTimingProvider::new(Client::new(), "https://api.opendota.com/api", None);
        let buckets = p.timings(9, CoreItem::BlackKingBar).await.expect("timings");
        println!("{buckets:?}");
        assert!(!buckets.is_empty());
    }
}
