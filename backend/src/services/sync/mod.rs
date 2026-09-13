//! Match synchronization: fetch, deduplicate, enrich, store.
//!
//! Phase 3 hangs metric computation off the end of [`sync_player`]; Phase 4/5
//! add analysis and profile updates. The shape stays: one pass, idempotent.

use std::collections::{HashMap, HashSet};

use futures::stream::{self, StreamExt};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::player::DotaPlayer;
use crate::domain::r#match::{NewMatch, NormalizedMatch};
use crate::domain::user::SteamProfileUpdate;
use crate::repositories;
use crate::services::dota::{fallback_hero_name, DotaDataProvider, ProviderError};

/// Concurrent match-detail requests. OpenDota's anonymous tier allows 60
/// calls/minute; four in flight stays well inside that while keeping a
/// twenty-match sync to a few seconds.
const DETAIL_CONCURRENCY: usize = 4;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SyncReport {
    /// Matches the provider returned.
    pub matches_seen: usize,
    /// Rows actually written.
    pub new_matches: usize,
    /// Already stored, so never re-fetched.
    pub duplicates_skipped: usize,
    /// New matches whose full detail (denies, net worth) was retrieved.
    pub details_enriched: usize,
    /// New matches stored from the summary alone because the detail call failed.
    pub details_failed: usize,
}

/// Run one synchronization pass for `player`.
///
/// Failures of individual match-detail calls are tolerated: the summary data is
/// still stored and the match is flagged `detail_synced = false`, so a later
/// sync can complete it. Only a failure to list matches at all aborts the pass.
pub async fn sync_player(
    pool: &PgPool,
    dota: &dyn DotaDataProvider,
    user_id: Uuid,
    player: &DotaPlayer,
    limit: u32,
) -> Result<(SyncReport, DotaPlayer), SyncError> {
    // Refresh the display profile first; it is cheap and makes the response
    // useful even when there are no new matches. A failure here is not fatal:
    // stale display data is better than a failed sync.
    match dota.get_player(player.dota_account_id).await {
        Ok(profile) => {
            // Steam display fields belong to the account, the rank to the
            // Dota identity.
            repositories::user::update_profile(
                pool,
                user_id,
                &SteamProfileUpdate {
                    persona_name: profile.persona_name,
                    avatar_url: profile.avatar_url,
                    profile_url: profile.profile_url,
                },
            )
            .await?;
            repositories::dota_player::update_rank(pool, player.id, profile.rank_tier).await?;
        }
        Err(e) => {
            tracing::warn!(
                dota_account_id = player.dota_account_id,
                error = %e,
                "profile refresh failed"
            );
        }
    }

    let fetched = dota
        .get_player_matches(player.dota_account_id, limit)
        .await?;
    let matches_seen = fetched.len();

    let existing = repositories::r#match::existing_match_ids(pool, player.id).await?;
    let mut pending = plan_new_matches(fetched, &existing);
    let duplicates_skipped = matches_seen - pending.len();

    let details_enriched = enrich_with_details(dota, player.dota_account_id, &mut pending).await;
    let details_failed = pending.len() - details_enriched;

    let heroes = match dota.heroes().await {
        Ok(map) => map,
        Err(e) => {
            // Cosmetic only: a sync must not fail because hero names are down.
            tracing::warn!(error = %e, "hero catalogue unavailable, using fallback names");
            HashMap::new()
        }
    };

    let rows: Vec<NewMatch> = pending
        .into_iter()
        .map(|m| {
            let hero_name = heroes
                .get(&m.hero_id)
                .cloned()
                .unwrap_or_else(|| fallback_hero_name(m.hero_id));
            NewMatch::new(player.id, m, hero_name)
        })
        .collect();

    let new_matches = repositories::r#match::insert_new(pool, &rows).await? as usize;
    // Returns the row as it now stands, including the rank refresh above.
    let player = repositories::dota_player::mark_synced(pool, player.id).await?;

    let report = SyncReport {
        matches_seen,
        new_matches,
        duplicates_skipped,
        details_enriched,
        details_failed,
    };

    tracing::info!(
        dota_account_id = player.dota_account_id,
        seen = report.matches_seen,
        new = report.new_matches,
        enriched = report.details_enriched,
        "sync complete"
    );

    Ok((report, player))
}

/// Drop matches already stored, and any the provider listed twice.
///
/// Pure, so the deduplication rule is testable without a database or a network.
pub fn plan_new_matches(
    fetched: Vec<NormalizedMatch>,
    existing: &HashSet<i64>,
) -> Vec<NormalizedMatch> {
    let mut seen = HashSet::new();

    fetched
        .into_iter()
        .filter(|m| !existing.contains(&m.match_id))
        .filter(|m| seen.insert(m.match_id))
        .collect()
}

/// Fetch full detail for each pending match, in place. Returns how many
/// succeeded; the rest keep their summary-only data.
async fn enrich_with_details(
    dota: &dyn DotaDataProvider,
    account_id: i64,
    pending: &mut [NormalizedMatch],
) -> usize {
    // Map over owned ids rather than borrowed matches: the borrow would have to
    // outlive each future, which the stream combinators cannot express.
    let ids: Vec<i64> = pending.iter().map(|m| m.match_id).collect();

    let details: Vec<(i64, Option<NormalizedMatch>)> = stream::iter(ids)
        .map(|id| async move {
            match dota.get_match_details(id, account_id).await {
                Ok(detail) => (id, Some(detail)),
                Err(e) => {
                    tracing::debug!(match_id = id, error = %e, "match detail unavailable");
                    (id, None)
                }
            }
        })
        .buffer_unordered(DETAIL_CONCURRENCY)
        .collect()
        .await;

    let mut by_id: HashMap<i64, NormalizedMatch> = details
        .into_iter()
        .filter_map(|(id, detail)| detail.map(|d| (id, d)))
        .collect();

    let mut enriched = 0;
    for m in pending.iter_mut() {
        if let Some(detail) = by_id.remove(&m.match_id) {
            m.enrich_with(&detail);
            enriched += 1;
        }
    }

    enriched
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn m(match_id: i64) -> NormalizedMatch {
        NormalizedMatch {
            match_id,
            hero_id: 1,
            won: true,
            duration_seconds: 2400,
            kills: 1,
            deaths: 1,
            assists: 1,
            gpm: 400,
            xpm: 400,
            last_hits: 100,
            denies: None,
            net_worth: None,
            hero_damage: None,
            tower_damage: None,
            hero_healing: None,
            lane_role: Some(1),
            is_roaming: Some(false),
            farm_rank: None,
            game_mode: Some(22),
            lobby_type: Some(7),
            party_size: Some(1),
            started_at: Utc::now(),
            from_details: false,
        }
    }

    #[test]
    fn already_stored_matches_are_skipped() {
        let existing: HashSet<i64> = [1, 2].into_iter().collect();
        let planned = plan_new_matches(vec![m(1), m(2), m(3)], &existing);

        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].match_id, 3);
    }

    #[test]
    fn a_duplicate_within_one_response_is_stored_once() {
        let planned = plan_new_matches(vec![m(7), m(7), m(8)], &HashSet::new());

        let ids: Vec<i64> = planned.iter().map(|p| p.match_id).collect();
        assert_eq!(ids, vec![7, 8]);
    }

    #[test]
    fn re_syncing_the_same_history_plans_nothing() {
        let fetched = vec![m(1), m(2), m(3)];
        let existing: HashSet<i64> = fetched.iter().map(|f| f.match_id).collect();

        assert!(plan_new_matches(fetched, &existing).is_empty());
    }

    #[test]
    fn an_empty_response_is_not_an_error() {
        assert!(plan_new_matches(vec![], &HashSet::new()).is_empty());
    }
}
