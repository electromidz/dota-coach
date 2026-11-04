//! Core item completion times against peers on the hero.
//!
//! Pure: the provider that supplies peer buckets lives in
//! `services::item_timing`.

use serde::Serialize;
use utoipa::ToSchema;

/// The core items the match record stores completion times for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoreItem {
    BlackKingBar,
    BlinkDagger,
    HandOfMidas,
}

impl CoreItem {
    pub const ALL: [CoreItem; 3] = [CoreItem::BlackKingBar, CoreItem::BlinkDagger, CoreItem::HandOfMidas];

    /// The provider's item key.
    pub fn key(self) -> &'static str {
        match self {
            CoreItem::BlackKingBar => "black_king_bar",
            CoreItem::BlinkDagger => "blink",
            CoreItem::HandOfMidas => "hand_of_midas",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CoreItem::BlackKingBar => "Black King Bar",
            CoreItem::BlinkDagger => "Blink Dagger",
            CoreItem::HandOfMidas => "Hand of Midas",
        }
    }
}

/// Completions by time bucket, as the provider counts them.
#[derive(Debug, Clone, PartialEq)]
pub struct TimingBucket {
    /// Upper edge of the bucket, in seconds.
    pub seconds: i32,
    pub games: i64,
}

/// Below this many recorded completions there is no peer median.
pub const MIN_GAMES: i64 = 20;

/// The bucket holding the median completion, or `None` on a thin sample.
pub fn median_seconds(buckets: &[TimingBucket]) -> Option<(i32, i64)> {
    let total: i64 = buckets.iter().map(|b| b.games).sum();
    if total < MIN_GAMES {
        return None;
    }
    let mut sorted = buckets.to_vec();
    sorted.sort_by_key(|b| b.seconds);
    let mut seen = 0;
    for bucket in &sorted {
        seen += bucket.games;
        if seen * 2 >= total {
            return Some((bucket.seconds, total));
        }
    }
    None
}

/// How a match's completion compares with peers on the hero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimingVerdict {
    Early,
    OnTime,
    Late,
}

/// One core item in one match, against peers on the hero.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct ItemTiming {
    pub item: CoreItem,
    pub label: &'static str,
    /// When this match completed it.
    pub seconds: i32,
    /// Middle of the bucket holding the peer median. `None` on a thin sample
    /// or a provider outage: then there is no comparison, not a guess.
    pub peer_median_seconds: Option<i32>,
    /// Completions behind that median, all ranks.
    pub peer_games: i64,
    pub verdict: Option<TimingVerdict>,
}

/// Within this many seconds of the peer median counts as on time. The
/// provider buckets in five-minute steps, so finer would be false precision.
pub const ON_TIME_WINDOW: i32 = 300;

/// Compare a completion with the peer buckets.
pub fn compare(item: CoreItem, seconds: i32, buckets: &[TimingBucket]) -> ItemTiming {
    let median = median_seconds(buckets);
    // The provider's `time` is a bucket's upper edge; its middle is fairer.
    let mid = median.map(|(edge, _)| edge - 150);
    let verdict = mid.map(|m| {
        if seconds > m + ON_TIME_WINDOW {
            TimingVerdict::Late
        } else if seconds < m - ON_TIME_WINDOW {
            TimingVerdict::Early
        } else {
            TimingVerdict::OnTime
        }
    });
    ItemTiming {
        item,
        label: item.label(),
        seconds,
        peer_median_seconds: mid,
        peer_games: median.map_or(0, |(_, games)| games),
        verdict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(seconds: i32, games: i64) -> TimingBucket {
        TimingBucket { seconds, games }
    }

    #[test]
    fn the_median_is_the_bucket_holding_the_middle_completion() {
        // Mirana BKB as the provider reported it: 1, 8, 10, 10 games.
        let buckets = [b(1800, 10), b(900, 1), b(1500, 10), b(1200, 8)];
        assert_eq!(median_seconds(&buckets), Some((1500, 29)));
    }

    #[test]
    fn a_52_minute_bkb_is_late_against_a_25_minute_median() {
        let buckets = [b(900, 1), b(1200, 8), b(1500, 10), b(1800, 10)];
        let t = compare(CoreItem::BlackKingBar, 52 * 60 + 2, &buckets);
        assert_eq!(t.peer_median_seconds, Some(1350));
        assert_eq!(t.peer_games, 29);
        assert_eq!(t.verdict, Some(TimingVerdict::Late));
        assert_eq!(compare(CoreItem::BlackKingBar, 1400, &buckets).verdict, Some(TimingVerdict::OnTime));
        assert_eq!(compare(CoreItem::BlackKingBar, 900, &buckets).verdict, Some(TimingVerdict::Early));
        // No sample, no verdict.
        assert_eq!(compare(CoreItem::BlackKingBar, 3000, &[]).verdict, None);
    }

    #[test]
    fn a_thin_sample_has_no_median() {
        assert_eq!(median_seconds(&[b(900, 5), b(1200, 10)]), None);
        assert_eq!(median_seconds(&[]), None);
    }
}
