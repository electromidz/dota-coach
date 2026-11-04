//! When peers on a hero complete a core item, for comparing one match's
//! timing against.
//!
//! Behind a trait like every provider. A timing comparison is an extra: an
//! outage or a thin sample means no comparison, never an estimated one.

pub mod opendota;

use async_trait::async_trait;
pub use crate::domain::item_timing::*;

#[derive(Debug, thiserror::Error)]
pub enum ItemTimingError {
    #[error("item timing provider unavailable: {0}")]
    Unavailable(String),
    #[error("unexpected item timing response: {0}")]
    InvalidResponse(String),
}

#[async_trait]
pub trait ItemTimingProvider: Send + Sync {
    async fn timings(&self, hero_id: i32, item: CoreItem) -> Result<Vec<TimingBucket>, ItemTimingError>;
}

/// No provider: no comparisons.
pub struct NullItemTimingProvider;

#[async_trait]
impl ItemTimingProvider for NullItemTimingProvider {
    async fn timings(&self, _: i32, _: CoreItem) -> Result<Vec<TimingBucket>, ItemTimingError> {
        Ok(Vec::new())
    }
}

/// Every core item this match completed, against peers on the hero.
///
/// Best effort per item: an outage or a thin sample leaves that item
/// uncompared (`peer_median_seconds: None`), never estimated, and never
/// fails the caller.
pub async fn for_match(
    provider: &dyn ItemTimingProvider,
    hero_id: i32,
    bkb: Option<i32>,
    blink: Option<i32>,
    midas: Option<i32>,
) -> Vec<ItemTiming> {
    let wanted: Vec<(CoreItem, i32)> = [
        (CoreItem::BlackKingBar, bkb),
        (CoreItem::BlinkDagger, blink),
        (CoreItem::HandOfMidas, midas),
    ]
    .into_iter()
    // Pre-horn or missing times are not completions worth comparing.
    .filter_map(|(item, seconds)| seconds.filter(|s| *s > 0).map(|s| (item, s)))
    .collect();

    let fetched = futures::future::join_all(
        wanted.iter().map(|(item, _)| provider.timings(hero_id, *item)),
    )
    .await;

    let mut out: Vec<ItemTiming> = wanted
        .into_iter()
        .zip(fetched)
        .map(|((item, seconds), buckets)| {
            if let Err(error) = &buckets {
                tracing::warn!(%error, hero_id, item = item.key(), "item timings unavailable");
            }
            compare(item, seconds, &buckets.unwrap_or_default())
        })
        .collect();
    out.sort_by_key(|t| t.seconds);
    out
}
