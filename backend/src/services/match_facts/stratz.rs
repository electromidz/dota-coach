//! STRATZ implementation of [`MatchFactsProvider`].
//!
//! Everything STRATZ-specific — the GraphQL document, its field names, its
//! enum spellings, its item and hero catalogues — is confined to this file.
//! Nothing below the normalizers escapes the module.
//!
//! # Why the match id is inlined rather than sent as a variable
//!
//! A GraphQL variable has to declare its scalar type by name (`$id: Long!`),
//! and a wrong name is rejected by the server with a message that looks like an
//! outage. An integer literal needs no declaration, and the value is an `i64`
//! formatted by Rust — there is no string, so there is nothing to inject. The
//! only thing given up is server-side query caching, which is worth less than
//! not guessing a scalar's spelling.
//!
//! # Rate limits, and why the snapshot cache is in here
//!
//! STRATZ publishes 20/second, 250/minute, 2,000/hour and 10,000/day for a
//! default token — per token, not per user, so the whole deployment shares one
//! budget. A finished match never changes, so a normalized reading is stored in
//! `match_fact_snapshots` and this provider is asked once per match and player.
//! The one exception is a match whose replay has not been parsed yet: that
//! answer can improve on its own, so it is re-asked on a much shorter TTL.
//!
//! The cache lives behind the trait rather than in front of it, which is the
//! arrangement `OpenDotaHeroMetaProvider` already uses: callers ask for a match
//! and get one, and whether that cost a network round trip is not their
//! business. It also means the guarantee cannot be forgotten at a call site —
//! and the call site that matters is `GET /api/matches/:id/analysis`, a read
//! that is required to cost nothing.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use sqlx::PgPool;
use tokio::sync::RwLock;

use super::MatchFactsProvider;
use crate::config::StratzConfig;
use crate::domain::match_facts::{
    DeathEvent, MatchFacts, MatchFactsPlayer, PurchaseEvent, TowerEvent,
};
use crate::repositories;
use crate::services::dota::ProviderError;

/// Snapshot rows this implementation owns. A second provider's rows sit beside
/// them rather than overwriting them.
const PROVIDER: &str = "stratz";

/// Everything this phase reads, and nothing else.
///
/// Deliberately narrow. STRATZ will happily return ward placements, rune
/// pickups, ability casts and per-minute action counts for all ten players; a
/// document that asked for them would cost the provider more, cost us the
/// parsing, and give the analysis layer nothing it uses. Fields are added here
/// when a finding needs them, not in case one might.
const MATCH_FIELDS: &str = "
    id
    didRadiantWin
    durationSeconds
    startDateTime
    parsedDateTime
    towerDeaths { time isRadiant }
    playbackData { roshanEvents { time } }
    players {
      steamAccountId
      isRadiant
      isVictory
      heroId
      lane
      position
      kills
      deaths
      assists
      goldPerMinute
      experiencePerMinute
      numLastHits
      numDenies
      networth
      level
      heroDamage
      towerDamage
      heroHealing
      stats {
        networthPerMinute
        lastHitsPerMinute
        deathEvents {
          time
          attacker
          goldLost
          goldFed
          timeDead
          isBurst
          hasHealAvailable
          isEngagedOnDeath
          isAttemptTpOut
        }
        itemPurchases { time itemId }
      }
    }
";

/// Hero and item display names, so a timestamp can name what happened.
const CATALOGUE_QUERY: &str = "{ constants { \
     heroes { id displayName } \
     items { id displayName } \
   } }";

pub struct StratzMatchFactsProvider {
    http: Client,
    url: String,
    /// Postgres rather than memory, for the same reason the hero-meta cache uses
    /// it: a match reading is identical for every reader of that match and
    /// should survive a restart instead of costing each deploy a fresh pull
    /// against a shared hourly budget.
    db: PgPool,
    parsed_ttl_hours: i64,
    unparsed_ttl_hours: i64,
    /// Fetched once per process. Hero and item names change a few times a year,
    /// and a restart is a perfectly good refresh interval for a display name.
    catalogue: RwLock<Option<Arc<Catalogue>>>,
}

/// Id-to-display-name lookups, as the provider publishes them.
#[derive(Debug, Default)]
struct Catalogue {
    heroes: HashMap<i32, String>,
    items: HashMap<i32, String>,
}

impl StratzMatchFactsProvider {
    /// `None` when no token is configured, so the caller can choose the null
    /// provider rather than building one that can only fail.
    pub fn new(config: &StratzConfig, db: PgPool) -> Result<Option<Arc<Self>>, reqwest::Error> {
        let Some(token) = config.api_token.clone() else {
            return Ok(None);
        };

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // A malformed token would otherwise fail on every request with a header
        // error rather than an authentication one. Non-ASCII means the value was
        // mis-copied, and saying so at startup is cheaper than at 3am.
        match HeaderValue::from_str(&format!("Bearer {token}")) {
            Ok(value) => {
                let mut value = value;
                value.set_sensitive(true);
                headers.insert(AUTHORIZATION, value);
            }
            Err(_) => {
                tracing::error!(
                    "STRATZ_API_TOKEN contains characters that cannot be sent in a header - \
                     treating the provider as unconfigured"
                );
                return Ok(None);
            }
        }

        // STRATZ identifies API traffic by user agent and rejects some clients
        // without it. Configurable because it is the provider's requirement to
        // change, not ours.
        if let Ok(agent) = HeaderValue::from_str(&config.user_agent) {
            headers.insert(USER_AGENT, agent);
        }

        let http = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_seconds))
            .default_headers(headers)
            .build()?;

        Ok(Some(Arc::new(Self {
            http,
            url: config.base_url.clone(),
            db,
            parsed_ttl_hours: config.cache_ttl_hours,
            unparsed_ttl_hours: config.unparsed_cache_ttl_hours,
            catalogue: RwLock::new(None),
        })))
    }

    async fn query<T: DeserializeOwned>(&self, document: &str) -> Result<T, ProviderError> {
        let response = self
            .http
            .post(&self.url)
            .json(&serde_json::json!({ "query": document }))
            .send()
            .await
            .map_err(|e| ProviderError::Unavailable(e.to_string()))?;

        let status = response.status();

        // Authentication is an operator's problem, not a user's, and it will not
        // fix itself — so it is logged at error and reported as an outage rather
        // than retried forever behind a rate-limit backoff.
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            tracing::error!(
                %status,
                "STRATZ rejected our credentials - check STRATZ_API_TOKEN"
            );
            return Err(ProviderError::Unavailable(format!(
                "authentication failed (HTTP {status})"
            )));
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited);
        }
        if !status.is_success() {
            return Err(ProviderError::Unavailable(format!("HTTP {status}")));
        }

        let body = response
            .text()
            .await
            .map_err(|e| ProviderError::Unavailable(e.to_string()))?;

        decode_envelope(&body)
    }

    /// Hero and item names, fetched once.
    ///
    /// Failure yields an empty catalogue rather than an error: a match whose
    /// deaths are labelled "a hero" is still a coachable match, and a cosmetic
    /// lookup must never be the reason an analysis cannot be produced.
    async fn catalogue(&self) -> Arc<Catalogue> {
        if let Some(cached) = self.catalogue.read().await.as_ref() {
            return Arc::clone(cached);
        }

        let loaded = match self.query::<RawConstants>(CATALOGUE_QUERY).await {
            Ok(raw) => Arc::new(raw.into_catalogue()),
            Err(e) => {
                tracing::warn!(error = %e, "STRATZ catalogue unavailable; timeline events will be unnamed");
                // Not stored: an empty catalogue cached for the life of the
                // process would make one transient failure permanent.
                return Arc::new(Catalogue::default());
            }
        };

        *self.catalogue.write().await = Some(Arc::clone(&loaded));
        loaded
    }
}

#[async_trait]
impl MatchFactsProvider for StratzMatchFactsProvider {
    fn is_configured(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "STRATZ"
    }

    async fn get_match_facts(
        &self,
        match_id: i64,
        account_id: i64,
    ) -> Result<MatchFacts, ProviderError> {
        if let Some(cached) = self.cached(match_id, account_id).await {
            return Ok(cached);
        }

        let document = format!("{{ match(id: {match_id}) {{{MATCH_FIELDS}}} }}");
        let raw: RawMatchEnvelope = self.query(&document).await?;

        // A match id nobody has is `data.match: null`, not an HTTP 404.
        let raw = raw.r#match.ok_or(ProviderError::NotFound)?;

        let catalogue = self.catalogue().await;
        let facts = normalize(raw, account_id, catalogue.as_ref())?;
        self.store(&facts).await;
        Ok(facts)
    }
}

impl StratzMatchFactsProvider {
    /// A stored reading, if one is still reusable.
    ///
    /// Every failure here is a miss. A cache that can break a match page is
    /// worse than no cache, and everything in it is re-derivable by asking the
    /// provider again.
    async fn cached(&self, match_id: i64, account_id: i64) -> Option<MatchFacts> {
        let snapshot = match repositories::match_facts::fresh_snapshot(
            &self.db,
            PROVIDER,
            match_id,
            account_id,
            self.parsed_ttl_hours,
            self.unparsed_ttl_hours,
        )
        .await
        {
            Ok(hit) => hit?,
            Err(e) => {
                tracing::warn!(error = %e, "match facts cache read failed");
                return None;
            }
        };

        match serde_json::from_value::<MatchFacts>(snapshot.payload) {
            Ok(facts) => Some(facts),
            // A shape that no longer parses is a miss, not an error: the row was
            // only ever a copy of something the provider can restate.
            Err(e) => {
                tracing::debug!(error = %e, "stored match facts did not deserialize; refetching");
                None
            }
        }
    }

    async fn store(&self, facts: &MatchFacts) {
        let Ok(payload) = serde_json::to_value(facts) else {
            tracing::warn!("match facts did not serialize; not stored");
            return;
        };

        if let Err(e) = repositories::match_facts::store_snapshot(
            &self.db,
            PROVIDER,
            facts.match_id,
            facts.player.account_id,
            facts.parsed,
            &payload,
        )
        .await
        {
            tracing::warn!(error = %e, "match facts cache write failed");
        }
    }
}

// ---------------------------------------------------------------------------
// Provider response shapes. Nothing below this line escapes the module.
// ---------------------------------------------------------------------------

/// The GraphQL envelope.
///
/// `errors` may arrive *alongside* usable data — GraphQL reports a failure per
/// field, not per request — so it is only fatal when `data` is absent.
#[derive(Debug, Deserialize)]
struct Envelope<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GraphQlError>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    #[serde(default)]
    message: String,
}

#[derive(Debug, Deserialize)]
struct RawMatchEnvelope {
    r#match: Option<RawMatch>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMatch {
    id: Option<i64>,
    did_radiant_win: Option<bool>,
    duration_seconds: Option<i32>,
    start_date_time: Option<i64>,
    /// Non-null only once the replay has been parsed. This is what gates every
    /// event list, exactly as OpenDota's `version` does.
    parsed_date_time: Option<i64>,
    #[serde(default)]
    tower_deaths: Vec<RawTowerDeath>,
    playback_data: Option<RawPlaybackData>,
    #[serde(default)]
    players: Vec<RawPlayer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTowerDeath {
    time: Option<i32>,
    is_radiant: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPlaybackData {
    #[serde(default)]
    roshan_events: Vec<RawTimed>,
}

#[derive(Debug, Deserialize)]
struct RawTimed {
    time: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPlayer {
    steam_account_id: Option<i64>,
    is_radiant: Option<bool>,
    is_victory: Option<bool>,
    hero_id: Option<i32>,
    lane: Option<String>,
    position: Option<String>,
    kills: Option<i32>,
    deaths: Option<i32>,
    assists: Option<i32>,
    gold_per_minute: Option<i32>,
    experience_per_minute: Option<i32>,
    num_last_hits: Option<i32>,
    num_denies: Option<i32>,
    networth: Option<i32>,
    level: Option<i32>,
    hero_damage: Option<i32>,
    tower_damage: Option<i32>,
    hero_healing: Option<i32>,
    stats: Option<RawPlayerStats>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPlayerStats {
    #[serde(default)]
    networth_per_minute: Vec<i32>,
    #[serde(default)]
    last_hits_per_minute: Vec<i32>,
    #[serde(default)]
    death_events: Vec<RawDeathEvent>,
    #[serde(default)]
    item_purchases: Vec<RawPurchase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDeathEvent {
    time: Option<i32>,
    /// The killer's hero id.
    attacker: Option<i32>,
    gold_lost: Option<i32>,
    gold_fed: Option<i32>,
    time_dead: Option<i32>,
    is_burst: Option<bool>,
    has_heal_available: Option<bool>,
    is_engaged_on_death: Option<bool>,
    is_attempt_tp_out: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPurchase {
    time: Option<i32>,
    item_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct RawConstants {
    constants: Option<RawConstantSets>,
}

#[derive(Debug, Deserialize)]
struct RawConstantSets {
    #[serde(default)]
    heroes: Vec<RawNamed>,
    #[serde(default)]
    items: Vec<RawNamed>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNamed {
    id: Option<i32>,
    display_name: Option<String>,
}

impl RawConstants {
    fn into_catalogue(self) -> Catalogue {
        let Some(sets) = self.constants else {
            return Catalogue::default();
        };

        let collect = |entries: Vec<RawNamed>| {
            entries
                .into_iter()
                .filter_map(|e| Some((e.id?, e.display_name?)))
                .collect()
        };

        Catalogue {
            heroes: collect(sets.heroes),
            items: collect(sets.items),
        }
    }
}

/// Unwrap a GraphQL envelope, treating a data-less response as the failure it is.
fn decode_envelope<T: DeserializeOwned>(body: &str) -> Result<T, ProviderError> {
    let envelope: Envelope<T> =
        serde_json::from_str(body).map_err(|e| ProviderError::Decode(e.to_string()))?;

    match envelope.data {
        Some(data) => {
            // Partial failures are worth an operator's attention — a field we
            // asked for was refused — but not worth discarding the rest of a
            // usable answer over.
            if !envelope.errors.is_empty() {
                let messages: Vec<&str> = envelope
                    .errors
                    .iter()
                    .map(|e| e.message.as_str())
                    .take(3)
                    .collect();
                tracing::warn!(errors = ?messages, "STRATZ returned data with field errors");
            }
            Ok(data)
        }
        None => {
            let detail = envelope
                .errors
                .first()
                .map(|e| e.message.clone())
                .unwrap_or_else(|| "no data and no error in the response".to_string());
            Err(ProviderError::Decode(detail))
        }
    }
}

/// Turn one STRATZ match into [`MatchFacts`] for one player in it.
fn normalize(
    raw: RawMatch,
    account_id: i64,
    catalogue: &Catalogue,
) -> Result<MatchFacts, ProviderError> {
    let match_id = raw
        .id
        .ok_or_else(|| ProviderError::Decode("match has no id".into()))?;
    let started_at = raw
        .start_date_time
        .and_then(|s| Utc.timestamp_opt(s, 0).single())
        .ok_or_else(|| ProviderError::Decode("match has no usable start time".into()))?;

    // Anonymous players come back with a null account id, so a private profile
    // inside an otherwise public match reads as "not in this match" — the same
    // answer OpenDota gives, and the same one an unknown match id gives.
    let player = raw
        .players
        .into_iter()
        .find(|p| p.steam_account_id == Some(account_id))
        .ok_or(ProviderError::NotFound)?;

    let is_radiant = player.is_radiant.unwrap_or(false);

    // `isVictory` is the player's own result and is what we prefer. Falling back
    // to the match result plus the player's side covers a response that omits
    // it; if neither is there, the match has no interpretable result.
    let won = match (player.is_victory, raw.did_radiant_win) {
        (Some(won), _) => won,
        (None, Some(radiant_won)) => radiant_won == is_radiant,
        (None, None) => return Err(ProviderError::Decode("match has no result".into())),
    };

    // Everything below is gated on this, and nothing fabricates a default for
    // it: an unparsed match has no events, and saying so is the point.
    let parsed = raw.parsed_date_time.is_some();
    let stats = player.stats.unwrap_or_default();

    let deaths = if parsed {
        stats
            .death_events
            .into_iter()
            .filter_map(|e| normalize_death(e, catalogue))
            .collect()
    } else {
        Vec::new()
    };

    let purchases = if parsed {
        stats
            .item_purchases
            .into_iter()
            .filter_map(|p| {
                let item_id = p.item_id?;
                Some(PurchaseEvent {
                    time_seconds: p.time?,
                    item_id,
                    item_name: catalogue.items.get(&item_id).cloned(),
                })
            })
            .collect()
    } else {
        Vec::new()
    };

    let towers = raw
        .tower_deaths
        .into_iter()
        .filter_map(|t| {
            Some(TowerEvent {
                time_seconds: t.time?,
                was_radiant_tower: t.is_radiant?,
            })
        })
        .collect();

    let roshan_kills = raw
        .playback_data
        .map(|p| p.roshan_events.into_iter().filter_map(|e| e.time).collect())
        .unwrap_or_default();

    let mut facts = MatchFacts {
        match_id,
        duration_seconds: raw.duration_seconds.unwrap_or(0).max(0),
        started_at,
        won,
        parsed,
        player: MatchFactsPlayer {
            account_id,
            hero_id: player.hero_id.unwrap_or(0),
            hero_name: player
                .hero_id
                .and_then(|id| catalogue.heroes.get(&id).cloned()),
            is_radiant,
            lane: player.lane.map(|l| label(&l)),
            position: player.position,
            kills: player.kills.unwrap_or(0),
            deaths: player.deaths.unwrap_or(0),
            assists: player.assists.unwrap_or(0),
            gpm: player.gold_per_minute.unwrap_or(0),
            xpm: player.experience_per_minute.unwrap_or(0),
            last_hits: player.num_last_hits.unwrap_or(0),
            denies: player.num_denies,
            net_worth: player.networth,
            level: player.level,
            hero_damage: player.hero_damage,
            tower_damage: player.tower_damage,
            hero_healing: player.hero_healing,
            net_worth_per_minute: if parsed {
                stats.networth_per_minute
            } else {
                Vec::new()
            },
            last_hits_per_minute: if parsed {
                stats.last_hits_per_minute
            } else {
                Vec::new()
            },
        },
        deaths,
        purchases,
        towers,
        roshan_kills,
    };

    // Time order is what every downstream statement assumes; the provider
    // usually supplies it, but "usually" is not a guarantee worth relying on for
    // a list of timestamps a player reads top to bottom.
    facts.deaths.sort_by_key(|d| d.time_seconds);
    facts.purchases.sort_by_key(|p| p.time_seconds);
    facts.towers.sort_by_key(|t| t.time_seconds);
    facts.roshan_kills.sort_unstable();

    Ok(facts)
}

/// `None` for a death with no timestamp: an untimed death is exactly the thing
/// this module exists not to report.
fn normalize_death(raw: RawDeathEvent, catalogue: &Catalogue) -> Option<DeathEvent> {
    let time_seconds = raw.time?;

    Some(DeathEvent {
        time_seconds,
        killer_hero_id: raw.attacker,
        killer_hero_name: raw
            .attacker
            .and_then(|id| catalogue.heroes.get(&id).cloned()),
        gold_lost: raw.gold_lost,
        gold_fed: raw.gold_fed,
        time_dead_seconds: raw.time_dead,
        was_burst: raw.is_burst,
        had_heal_available: raw.has_heal_available,
        was_in_a_fight: raw.is_engaged_on_death,
        attempted_to_escape: raw.is_attempt_tp_out,
    })
}

/// `SAFE_LANE` -> `Safe lane`.
///
/// The provider's enum spelling is a wire format, not something to show a
/// player, and the evidence sentences that quote it are read by both a user and
/// a model.
fn label(raw: &str) -> String {
    let mut words = raw.split('_').filter(|w| !w.is_empty());
    let Some(first) = words.next() else {
        return String::new();
    };

    let mut out = String::with_capacity(raw.len());
    let mut chars = first.chars();
    if let Some(initial) = chars.next() {
        out.push(initial.to_ascii_uppercase());
        out.extend(chars.map(|c| c.to_ascii_lowercase()));
    }
    for word in words {
        out.push(' ');
        out.extend(word.chars().map(|c| c.to_ascii_lowercase()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MATCH: &str = include_str!("../../../tests/fixtures/stratz_match.json");
    const MATCH_UNPARSED: &str = include_str!("../../../tests/fixtures/stratz_match_unparsed.json");

    const ACCOUNT_ID: i64 = 86_745_912;
    /// The Dire player in the same fixture.
    const OTHER_ACCOUNT_ID: i64 = 99_999_999;

    fn catalogue() -> Catalogue {
        Catalogue {
            heroes: HashMap::from([
                (35, "Sniper".to_string()),
                (26, "Lion".to_string()),
                (2, "Axe".to_string()),
            ]),
            items: HashMap::from([
                (1, "Blink Dagger".to_string()),
                (116, "Black King Bar".to_string()),
            ]),
        }
    }

    fn facts(body: &str, account_id: i64) -> MatchFacts {
        let envelope: RawMatchEnvelope = decode_envelope(body).unwrap();
        normalize(envelope.r#match.unwrap(), account_id, &catalogue()).unwrap()
    }

    #[test]
    fn a_parsed_match_yields_the_requested_players_timeline() {
        let m = facts(MATCH, ACCOUNT_ID);

        assert_eq!(m.match_id, 7_500_000_001);
        assert_eq!(m.duration_seconds, 2520);
        assert_eq!(m.started_at.timestamp(), 1_700_000_000);
        assert!(m.parsed);
        assert!(m.won);
        assert!(m.has_timeline());

        assert_eq!(m.player.hero_id, 35);
        assert_eq!(m.player.hero_name.as_deref(), Some("Sniper"));
        assert_eq!(m.player.lane.as_deref(), Some("Safe lane"));
        assert_eq!(m.player.position.as_deref(), Some("POSITION_1"));
        assert_eq!(
            (m.player.kills, m.player.deaths, m.player.assists),
            (8, 3, 12)
        );
        assert_eq!(m.player.gpm, 612);
        assert_eq!(m.player.net_worth, Some(24_500));
    }

    #[test]
    fn deaths_carry_their_timestamp_killer_and_circumstances() {
        let m = facts(MATCH, ACCOUNT_ID);

        assert_eq!(m.deaths.len(), 3);

        let first = &m.deaths[0];
        assert_eq!(first.time_seconds, 612);
        assert_eq!(first.killer_hero_name.as_deref(), Some("Lion"));
        assert_eq!(first.gold_lost, Some(320));
        assert_eq!(first.time_dead_seconds, Some(18));
        assert_eq!(first.was_burst, Some(true));
        assert_eq!(first.had_heal_available, Some(false));

        // An unnamed killer stays unnamed rather than being guessed at.
        let unknown = &m.deaths[2];
        assert_eq!(unknown.killer_hero_id, Some(999));
        assert_eq!(unknown.killer_hero_name, None);
    }

    #[test]
    fn purchases_are_named_from_the_catalogue_and_kept_in_time_order() {
        let m = facts(MATCH, ACCOUNT_ID);

        // The fixture lists them out of order on purpose.
        let times: Vec<i32> = m.purchases.iter().map(|p| p.time_seconds).collect();
        assert_eq!(times, vec![-90, 862, 1304]);

        assert_eq!(m.purchases[1].item_name.as_deref(), Some("Blink Dagger"));
        // An id outside the catalogue is carried without a name.
        assert_eq!(m.purchases[0].item_id, 44);
        assert_eq!(m.purchases[0].item_name, None);
    }

    #[test]
    fn towers_and_roshan_kills_come_from_the_match_not_the_player() {
        let m = facts(MATCH, ACCOUNT_ID);

        assert_eq!(m.towers.len(), 3);
        // Radiant player: their own towers are the Radiant ones.
        assert_eq!(m.towers_lost().count(), 1);
        assert_eq!(m.towers_taken().count(), 2);
        assert_eq!(m.roshan_kills, vec![1_500, 2_100]);
    }

    #[test]
    fn the_other_side_reads_the_same_match_from_its_own_perspective() {
        let m = facts(MATCH, OTHER_ACCOUNT_ID);

        assert!(!m.player.is_radiant);
        assert!(!m.won, "the fixture's Dire player lost");
        assert_eq!(m.towers_lost().count(), 2);
        assert_eq!(m.towers_taken().count(), 1);
    }

    /// The honesty rule, enforced rather than documented: no parse, no events.
    /// Without this an unparsed match would look like a game in which nobody
    /// ever died.
    #[test]
    fn an_unparsed_match_reports_no_events_at_all() {
        let m = facts(MATCH_UNPARSED, ACCOUNT_ID);

        assert!(!m.parsed);
        assert!(!m.has_timeline());
        assert!(
            m.deaths.is_empty(),
            "the payload carried events; a parse did not happen"
        );
        assert!(m.purchases.is_empty());
        assert!(m.player.net_worth_per_minute.is_empty());

        // The aggregate is still real and still usable.
        assert_eq!(m.player.deaths, 7);
        assert_eq!(m.player.gpm, 388);
    }

    #[test]
    fn a_player_who_is_not_in_the_match_reads_as_not_found() {
        let envelope: RawMatchEnvelope = decode_envelope(MATCH).unwrap();
        let error = normalize(envelope.r#match.unwrap(), 1_234_567, &catalogue()).unwrap_err();

        assert!(matches!(error, ProviderError::NotFound));
    }

    /// STRATZ reports an unknown match id inside a 200 response, so this is the
    /// shape a wrong id actually arrives in.
    #[test]
    fn a_null_match_is_not_found_rather_than_a_decode_failure() {
        let envelope: RawMatchEnvelope = decode_envelope(r#"{"data":{"match":null}}"#).unwrap();
        assert!(envelope.r#match.is_none());
    }

    #[test]
    fn a_response_with_only_errors_is_a_provider_failure() {
        let body = r#"{"errors":[{"message":"Rate limit exceeded"}]}"#;
        let error = decode_envelope::<RawMatchEnvelope>(body).unwrap_err();

        match error {
            ProviderError::Decode(detail) => assert!(detail.contains("Rate limit")),
            other => panic!("expected a decode failure, got {other:?}"),
        }
    }

    /// GraphQL reports failures per field. Discarding a usable match because one
    /// optional field was refused would turn a partial outage into a total one.
    #[test]
    fn data_alongside_field_errors_is_still_used() {
        let body = r#"{"data":{"match":null},"errors":[{"message":"playbackData unavailable"}]}"#;
        assert!(decode_envelope::<RawMatchEnvelope>(body).is_ok());
    }

    #[test]
    fn malformed_json_is_a_decode_failure_rather_than_a_panic() {
        let error = decode_envelope::<RawMatchEnvelope>("not json").unwrap_err();
        assert!(matches!(error, ProviderError::Decode(_)));
    }

    #[test]
    fn provider_enum_spellings_become_readable_labels() {
        assert_eq!(label("SAFE_LANE"), "Safe lane");
        assert_eq!(label("MID_LANE"), "Mid lane");
        assert_eq!(label("ROAMING"), "Roaming");
        assert_eq!(label("UNKNOWN"), "Unknown");
        assert_eq!(label(""), "");
    }

    /// The document is the one place a typo is invisible until production: a
    /// field we parse but never asked for arrives as `None` forever.
    #[test]
    fn the_document_asks_for_every_field_the_normalizer_reads() {
        for field in [
            "didRadiantWin",
            "durationSeconds",
            "startDateTime",
            "parsedDateTime",
            "towerDeaths",
            "isRadiant",
            "roshanEvents",
            "steamAccountId",
            "isVictory",
            "heroId",
            "lane",
            "position",
            "goldPerMinute",
            "experiencePerMinute",
            "numLastHits",
            "numDenies",
            "networth",
            "heroDamage",
            "towerDamage",
            "heroHealing",
            "networthPerMinute",
            "lastHitsPerMinute",
            "deathEvents",
            "attacker",
            "goldLost",
            "goldFed",
            "timeDead",
            "isBurst",
            "hasHealAvailable",
            "isEngagedOnDeath",
            "isAttemptTpOut",
            "itemPurchases",
            "itemId",
        ] {
            assert!(
                MATCH_FIELDS.contains(field),
                "`{field}` is read by the normalizer but not requested"
            );
        }
    }

    #[test]
    fn the_match_id_reaches_the_document_as_an_integer_literal() {
        // No quotes: a quoted id is a GraphQL String where a Long is expected,
        // and the server rejects the whole document.
        let document = format!("{{ match(id: {}) {{{MATCH_FIELDS}}} }}", 7_500_000_001i64);
        assert!(document.contains("match(id: 7500000001)"));
    }
}
