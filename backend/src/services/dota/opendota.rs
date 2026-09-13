//! OpenDota implementation of [`DotaDataProvider`].
//!
//! Everything provider-specific — URL shapes, field names, the radiant/dire
//! player-slot convention — is confined to this file.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use tokio::sync::RwLock;

use super::{DotaDataProvider, ProviderError, ProviderPlayer};
use crate::config::DotaConfig;
use crate::domain::r#match::NormalizedMatch;

/// Slots 0-127 are Radiant, 128-255 are Dire.
const DIRE_SLOT_THRESHOLD: i32 = 128;

/// Fields to request from `players/{id}/matches`.
///
/// `match_id`, `player_slot`, `radiant_win`, `duration`, `game_mode` and
/// `lobby_type` always come back; everything else must be asked for.
const SUMMARY_PROJECTION: &[&str] = &[
    "hero_id",
    "start_time",
    "kills",
    "deaths",
    "assists",
    "gold_per_min",
    "xp_per_min",
    "last_hits",
    "hero_damage",
    "tower_damage",
    "hero_healing",
    "lane_role",
    "is_roaming",
    "party_size",
];

pub struct OpenDotaProvider {
    http: Client,
    base_url: String,
    api_key: Option<String>,
    /// The hero catalogue changes a few times a year; fetch once per process.
    heroes: RwLock<Option<Arc<HashMap<i32, String>>>>,
}

impl OpenDotaProvider {
    pub fn new(config: &DotaConfig) -> Result<Self, reqwest::Error> {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_seconds))
            .user_agent(concat!("dota-coach/", env!("CARGO_PKG_VERSION")))
            .build()?;

        Ok(Self {
            http,
            base_url: config.base_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            heroes: RwLock::new(None),
        })
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, ProviderError> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));

        let mut request = self.http.get(&url).query(query);
        if let Some(key) = &self.api_key {
            request = request.query(&[("api_key", key)]);
        }

        let response = request
            .send()
            .await
            .map_err(|e| ProviderError::Unavailable(e.to_string()))?;

        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return Err(ProviderError::NotFound);
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited);
        }
        if !status.is_success() {
            return Err(ProviderError::Unavailable(format!("HTTP {status}")));
        }

        // Read as text first so a decode failure can report what broke without
        // leaking the whole payload into the error.
        let body = response
            .text()
            .await
            .map_err(|e| ProviderError::Unavailable(e.to_string()))?;

        serde_json::from_str(&body).map_err(|e| ProviderError::Decode(e.to_string()))
    }
}

#[async_trait]
impl DotaDataProvider for OpenDotaProvider {
    async fn get_player(&self, account_id: i64) -> Result<ProviderPlayer, ProviderError> {
        let raw: RawPlayerEnvelope = self.get(&format!("players/{account_id}"), &[]).await?;
        Ok(normalize_player(account_id, raw))
    }

    async fn get_player_matches(
        &self,
        account_id: i64,
        limit: u32,
    ) -> Result<Vec<NormalizedMatch>, ProviderError> {
        // `recentMatches` is capped at 20; `matches?limit=` is the parameterized
        // equivalent, but `project` *replaces* its default field set rather than
        // extending it — so every field we read has to be listed.
        let mut query = vec![("limit", limit.to_string())];
        query.extend(
            SUMMARY_PROJECTION
                .iter()
                .map(|field| ("project", (*field).to_string())),
        );

        let raw: Vec<RawRecentMatch> = self
            .get(&format!("players/{account_id}/matches"), &query)
            .await?;

        // Matches whose result the provider does not know yet are dropped
        // rather than guessed at.
        Ok(raw.into_iter().filter_map(normalize_recent_match).collect())
    }

    async fn get_match_details(
        &self,
        match_id: i64,
        account_id: i64,
    ) -> Result<NormalizedMatch, ProviderError> {
        let raw: RawMatchDetail = self.get(&format!("matches/{match_id}"), &[]).await?;
        normalize_match_detail(raw, account_id)
    }

    async fn heroes(&self) -> Result<HashMap<i32, String>, ProviderError> {
        if let Some(cached) = self.heroes.read().await.as_ref() {
            return Ok((**cached).clone());
        }

        let raw: Vec<RawHero> = self.get("heroes", &[]).await?;
        let map: HashMap<i32, String> = raw.into_iter().map(|h| (h.id, h.localized_name)).collect();

        *self.heroes.write().await = Some(Arc::new(map.clone()));
        Ok(map)
    }
}

// ---------------------------------------------------------------------------
// Provider response shapes. Nothing below this line escapes the module.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawPlayerEnvelope {
    profile: Option<RawProfile>,
    rank_tier: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct RawProfile {
    personaname: Option<String>,
    avatarfull: Option<String>,
    profileurl: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawRecentMatch {
    match_id: i64,
    player_slot: i32,
    radiant_win: Option<bool>,
    duration: Option<i32>,
    start_time: Option<i64>,
    hero_id: i32,
    game_mode: Option<i32>,
    lobby_type: Option<i32>,
    kills: Option<i32>,
    deaths: Option<i32>,
    assists: Option<i32>,
    gold_per_min: Option<i32>,
    xp_per_min: Option<i32>,
    last_hits: Option<i32>,
    hero_damage: Option<i32>,
    tower_damage: Option<i32>,
    hero_healing: Option<i32>,
    lane_role: Option<i16>,
    is_roaming: Option<bool>,
    party_size: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct RawMatchDetail {
    match_id: i64,
    radiant_win: Option<bool>,
    duration: Option<i32>,
    start_time: Option<i64>,
    game_mode: Option<i32>,
    lobby_type: Option<i32>,
    #[serde(default)]
    players: Vec<RawMatchPlayer>,
}

#[derive(Debug, Deserialize)]
struct RawMatchPlayer {
    account_id: Option<i64>,
    player_slot: i32,
    hero_id: i32,
    kills: Option<i32>,
    deaths: Option<i32>,
    assists: Option<i32>,
    last_hits: Option<i32>,
    denies: Option<i32>,
    gold_per_min: Option<i32>,
    xp_per_min: Option<i32>,
    net_worth: Option<i32>,
    total_gold: Option<i32>,
    hero_damage: Option<i32>,
    tower_damage: Option<i32>,
    hero_healing: Option<i32>,
    lane_role: Option<i16>,
    is_roaming: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawHero {
    id: i32,
    localized_name: String,
}

fn normalize_player(account_id: i64, raw: RawPlayerEnvelope) -> ProviderPlayer {
    let has_public_profile = raw.profile.is_some();
    let (persona_name, avatar_url, profile_url) = match raw.profile {
        Some(p) => (p.personaname, p.avatarfull, p.profileurl),
        None => (None, None, None),
    };

    ProviderPlayer {
        account_id,
        persona_name,
        avatar_url,
        profile_url,
        rank_tier: raw.rank_tier,
        has_public_profile,
    }
}

/// `None` when the match cannot be interpreted: no known winner, no start time,
/// or a timestamp outside the representable range.
fn normalize_recent_match(raw: RawRecentMatch) -> Option<NormalizedMatch> {
    let radiant_win = raw.radiant_win?;
    let started_at = timestamp(raw.start_time?)?;
    let won = radiant_win == is_radiant(raw.player_slot);

    Some(NormalizedMatch {
        match_id: raw.match_id,
        hero_id: raw.hero_id,
        won,
        duration_seconds: raw.duration.unwrap_or(0).max(0),
        kills: raw.kills.unwrap_or(0),
        deaths: raw.deaths.unwrap_or(0),
        assists: raw.assists.unwrap_or(0),
        gpm: raw.gold_per_min.unwrap_or(0),
        xpm: raw.xp_per_min.unwrap_or(0),
        last_hits: raw.last_hits.unwrap_or(0),
        // The summary endpoint carries neither.
        denies: None,
        net_worth: None,
        hero_damage: raw.hero_damage,
        tower_damage: raw.tower_damage,
        hero_healing: raw.hero_healing,
        lane_role: raw.lane_role,
        is_roaming: raw.is_roaming,
        // Needs the whole team, which the summary endpoint does not return.
        farm_rank: None,
        game_mode: raw.game_mode,
        lobby_type: raw.lobby_type,
        party_size: raw.party_size,
        started_at,
        from_details: false,
    })
}

fn normalize_match_detail(
    raw: RawMatchDetail,
    account_id: i64,
) -> Result<NormalizedMatch, ProviderError> {
    let radiant_win = raw
        .radiant_win
        .ok_or_else(|| ProviderError::Decode("match has no result".into()))?;
    let started_at = raw
        .start_time
        .and_then(timestamp)
        .ok_or_else(|| ProviderError::Decode("match has no usable start time".into()))?;

    // Anonymous players are reported with a null account id, so a private
    // profile inside an otherwise public match reads as "not found".
    let index = raw
        .players
        .iter()
        .position(|p| p.account_id == Some(account_id))
        .ok_or(ProviderError::NotFound)?;

    let farm_rank = farm_rank(&raw.players, index);
    let player = raw
        .players
        .into_iter()
        .nth(index)
        .expect("index just found");

    Ok(NormalizedMatch {
        match_id: raw.match_id,
        hero_id: player.hero_id,
        won: radiant_win == is_radiant(player.player_slot),
        duration_seconds: raw.duration.unwrap_or(0).max(0),
        kills: player.kills.unwrap_or(0),
        deaths: player.deaths.unwrap_or(0),
        assists: player.assists.unwrap_or(0),
        gpm: player.gold_per_min.unwrap_or(0),
        xpm: player.xp_per_min.unwrap_or(0),
        last_hits: player.last_hits.unwrap_or(0),
        denies: player.denies,
        // Older matches predate `net_worth`; `total_gold` is the closest stand-in.
        net_worth: net_worth_of(&player),
        hero_damage: player.hero_damage,
        tower_damage: player.tower_damage,
        hero_healing: player.hero_healing,
        lane_role: player.lane_role,
        is_roaming: player.is_roaming,
        farm_rank,
        game_mode: raw.game_mode,
        lobby_type: raw.lobby_type,
        party_size: None,
        started_at,
        from_details: true,
    })
}

/// Where the player sits in their own team's farm priority: 0 is the highest
/// net worth, 4 the lowest.
///
/// Only usable from full match details, and only a stand-in for the real lane
/// assignment, which unparsed matches do not carry.
fn farm_rank(players: &[RawMatchPlayer], index: usize) -> Option<u8> {
    let target = players.get(index)?;
    let target_side = is_radiant(target.player_slot);
    let target_worth = net_worth_of(target)?;

    let mut team: Vec<i32> = players
        .iter()
        .filter(|p| is_radiant(p.player_slot) == target_side)
        .filter_map(net_worth_of)
        .collect();

    // A partial team (anonymous players, truncated payloads) makes the rank
    // meaningless, so report nothing rather than something wrong.
    if team.len() < 5 {
        return None;
    }

    team.sort_unstable_by(|a, b| b.cmp(a));
    team.iter()
        .position(|w| *w == target_worth)
        .map(|rank| rank as u8)
}

fn net_worth_of(player: &RawMatchPlayer) -> Option<i32> {
    player.net_worth.or(player.total_gold)
}

fn is_radiant(player_slot: i32) -> bool {
    player_slot < DIRE_SLOT_THRESHOLD
}

fn timestamp(seconds: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(seconds, 0).single()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::r#match::Role;

    const RECENT_MATCHES: &str = include_str!("../../../tests/fixtures/opendota_matches.json");
    const MATCH_DETAIL: &str = include_str!("../../../tests/fixtures/opendota_match_detail.json");
    const MATCH_DETAIL_UNPARSED: &str =
        include_str!("../../../tests/fixtures/opendota_match_detail_unparsed.json");
    const PLAYER: &str = include_str!("../../../tests/fixtures/opendota_player.json");

    const ACCOUNT_ID: i64 = 86_745_912;

    fn recent() -> Vec<NormalizedMatch> {
        let raw: Vec<RawRecentMatch> = serde_json::from_str(RECENT_MATCHES).unwrap();
        raw.into_iter().filter_map(normalize_recent_match).collect()
    }

    #[test]
    fn player_profile_is_normalized() {
        let raw: RawPlayerEnvelope = serde_json::from_str(PLAYER).unwrap();
        let player = normalize_player(ACCOUNT_ID, raw);

        assert_eq!(player.account_id, ACCOUNT_ID);
        assert_eq!(player.persona_name.as_deref(), Some("Test Player"));
        assert_eq!(
            player.profile_url.as_deref(),
            Some("https://steamcommunity.com/id/testplayer/")
        );
        assert_eq!(player.rank_tier, Some(55));
        assert!(player.has_public_profile);
    }

    #[test]
    fn a_private_profile_still_yields_a_player() {
        let raw: RawPlayerEnvelope =
            serde_json::from_str(r#"{"profile":null,"rank_tier":null}"#).unwrap();
        let player = normalize_player(ACCOUNT_ID, raw);

        assert!(!player.has_public_profile);
        assert_eq!(player.persona_name, None);
    }

    #[test]
    fn radiant_and_dire_slots_decide_the_result() {
        let matches = recent();

        // Fixture 1: radiant slot 2, radiant_win true -> win.
        let win = &matches[0];
        assert_eq!(win.match_id, 7_500_000_001);
        assert!(win.won);

        // Fixture 2: dire slot 130, radiant_win true -> loss.
        let loss = &matches[1];
        assert_eq!(loss.match_id, 7_500_000_002);
        assert!(!loss.won);

        // Fixture 3: dire slot 129, radiant_win false -> win.
        let dire_win = &matches[2];
        assert_eq!(dire_win.match_id, 7_500_000_003);
        assert!(dire_win.won);
    }

    #[test]
    fn summary_fields_map_across_and_missing_numbers_become_zero() {
        let m = &recent()[0];

        assert_eq!(m.hero_id, 35);
        assert_eq!((m.kills, m.deaths, m.assists), (8, 7, 12));
        assert_eq!(m.gpm, 612);
        assert_eq!(m.xpm, 701);
        assert_eq!(m.last_hits, 380);
        assert_eq!(m.duration_seconds, 2520);
        assert_eq!(m.started_at.timestamp(), 1_700_000_000);
        assert!(!m.from_details);

        // Nulls in the fixture must not become garbage.
        let sparse = &recent()[2];
        assert_eq!(sparse.gpm, 0);
        assert_eq!(sparse.last_hits, 0);
        assert_eq!(sparse.hero_damage, None);
    }

    #[test]
    fn the_summary_endpoint_never_invents_detail_only_fields() {
        for m in recent() {
            assert_eq!(m.denies, None);
            assert_eq!(m.net_worth, None);
        }
    }

    #[test]
    fn matches_without_a_known_result_are_dropped() {
        let raw: Vec<RawRecentMatch> = serde_json::from_str(RECENT_MATCHES).unwrap();
        // The fixture has 4 entries; one has radiant_win: null.
        assert_eq!(raw.len(), 4);
        assert_eq!(recent().len(), 3);
    }

    #[test]
    fn match_detail_selects_the_requested_player() {
        let raw: RawMatchDetail = serde_json::from_str(MATCH_DETAIL).unwrap();
        let m = normalize_match_detail(raw, ACCOUNT_ID).unwrap();

        assert_eq!(m.match_id, 7_500_000_001);
        assert_eq!(m.hero_id, 35);
        assert_eq!(m.denies, Some(14));
        assert_eq!(m.net_worth, Some(24_500));
        assert_eq!(m.tower_damage, Some(6_200));
        assert!(m.won);
        assert!(m.from_details);
        assert_eq!(m.derive_role(), Role::Carry);
    }

    #[test]
    fn net_worth_falls_back_to_total_gold_on_older_matches() {
        let raw: RawMatchDetail = serde_json::from_str(MATCH_DETAIL).unwrap();
        // Second player in the fixture has net_worth null but total_gold set.
        let m = normalize_match_detail(raw, 99_999_999).unwrap();
        assert_eq!(m.net_worth, Some(18_300));
    }

    #[test]
    fn an_incomplete_team_yields_no_farm_rank() {
        // The three-player fixture cannot support a within-team ranking.
        let raw: RawMatchDetail = serde_json::from_str(MATCH_DETAIL).unwrap();
        let m = normalize_match_detail(raw, ACCOUNT_ID).unwrap();
        assert_eq!(m.farm_rank, None);
    }

    #[test]
    fn an_unparsed_match_is_ranked_by_farm_priority() {
        let raw: RawMatchDetail = serde_json::from_str(MATCH_DETAIL_UNPARSED).unwrap();
        let m = normalize_match_detail(raw, ACCOUNT_ID).unwrap();

        // No lane data at all in an unparsed match.
        assert_eq!(m.lane_role, None);
        // Highest net worth on Radiant.
        assert_eq!(m.farm_rank, Some(0));
        assert_eq!(m.derive_role(), Role::Core);
    }

    #[test]
    fn farm_rank_is_measured_within_the_players_own_team() {
        let raw: RawMatchDetail = serde_json::from_str(MATCH_DETAIL_UNPARSED).unwrap();
        // Dire's lowest earner, despite five Radiant players out-farming others.
        let m = normalize_match_detail(raw, 99_999_998).unwrap();

        assert_eq!(m.farm_rank, Some(4));
        assert_eq!(m.derive_role(), Role::HardSupport);
        assert!(!m.won, "dire lost this fixture");
    }

    #[test]
    fn an_anonymous_player_reads_as_not_found() {
        let raw: RawMatchDetail = serde_json::from_str(MATCH_DETAIL).unwrap();
        let err = normalize_match_detail(raw, 1_234_567).unwrap_err();
        assert!(matches!(err, ProviderError::NotFound));
    }
}
