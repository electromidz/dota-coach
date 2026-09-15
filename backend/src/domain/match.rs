use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;
use utoipa::ToSchema;

/// Creep score per minute above which a laner is treated as a core.
/// Deliberately blunt: the provider does not report roles, so this is an
/// estimate and the UI labels it as one.
const CORE_LAST_HITS_PER_MINUTE: f64 = 3.0;

/// A match as the provider described it, already stripped of provider-specific
/// shapes but not yet tied to a player row.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedMatch {
    pub match_id: i64,
    pub hero_id: i32,
    pub won: bool,
    pub duration_seconds: i32,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub gpm: i32,
    pub xpm: i32,
    pub last_hits: i32,
    pub denies: Option<i32>,
    pub net_worth: Option<i32>,
    pub hero_damage: Option<i32>,
    pub tower_damage: Option<i32>,
    pub hero_healing: Option<i32>,
    pub lane_role: Option<i16>,
    pub is_roaming: Option<bool>,
    /// Position in the team's net-worth order, 0 = highest. Only available from
    /// full match details.
    pub farm_rank: Option<u8>,
    pub game_mode: Option<i32>,
    pub lobby_type: Option<i32>,
    pub party_size: Option<i32>,
    pub started_at: DateTime<Utc>,
    /// True once the full match detail (denies, net worth, damage) was fetched.
    pub from_details: bool,

    /// Team totals, for participation rates. Any match detail carries these.
    pub team_kills: Option<i32>,
    pub team_deaths: Option<i32>,

    /// True when the provider had a parsed replay. Everything below is gated
    /// on it, and stays `None` for the majority of public matches.
    pub replay_parsed: bool,
    pub last_hits_at_10: Option<i32>,
    pub last_hits_at_15: Option<i32>,
    pub gold_at_10: Option<i32>,
    pub gold_at_15: Option<i32>,
    pub xp_at_10: Option<i32>,
    pub xp_at_15: Option<i32>,
    /// Seconds from the horn.
    pub bkb_seconds: Option<i32>,
    pub blink_seconds: Option<i32>,
    pub midas_seconds: Option<i32>,
    /// 0-1, as the provider reports it.
    pub teamfight_participation: Option<f32>,
}

impl NormalizedMatch {
    /// Fold a full match detail into a recent-matches summary.
    ///
    /// The detail is the complete per-player record, so it wins for every
    /// performance figure. The summary keeps only what the detail endpoint does
    /// not carry (party size).
    pub fn enrich_with(&mut self, detail: &NormalizedMatch) {
        self.kills = detail.kills;
        self.deaths = detail.deaths;
        self.assists = detail.assists;
        self.gpm = detail.gpm;
        self.xpm = detail.xpm;
        self.last_hits = detail.last_hits;

        self.denies = detail.denies.or(self.denies);
        self.net_worth = detail.net_worth.or(self.net_worth);
        self.hero_damage = detail.hero_damage.or(self.hero_damage);
        self.tower_damage = detail.tower_damage.or(self.tower_damage);
        self.hero_healing = detail.hero_healing.or(self.hero_healing);
        self.lane_role = detail.lane_role.or(self.lane_role);
        self.is_roaming = detail.is_roaming.or(self.is_roaming);
        self.farm_rank = detail.farm_rank.or(self.farm_rank);

        self.team_kills = detail.team_kills.or(self.team_kills);
        self.team_deaths = detail.team_deaths.or(self.team_deaths);

        // Parsed-replay facts only ever arrive with the detail.
        self.replay_parsed = detail.replay_parsed;
        self.last_hits_at_10 = detail.last_hits_at_10.or(self.last_hits_at_10);
        self.last_hits_at_15 = detail.last_hits_at_15.or(self.last_hits_at_15);
        self.gold_at_10 = detail.gold_at_10.or(self.gold_at_10);
        self.gold_at_15 = detail.gold_at_15.or(self.gold_at_15);
        self.xp_at_10 = detail.xp_at_10.or(self.xp_at_10);
        self.xp_at_15 = detail.xp_at_15.or(self.xp_at_15);
        self.bkb_seconds = detail.bkb_seconds.or(self.bkb_seconds);
        self.blink_seconds = detail.blink_seconds.or(self.blink_seconds);
        self.midas_seconds = detail.midas_seconds.or(self.midas_seconds);
        self.teamfight_participation = detail
            .teamfight_participation
            .or(self.teamfight_participation);

        self.from_details = true;
    }

    /// Best-effort role label. Not authoritative: Dota does not publish roles,
    /// so this combines the provider's lane assignment with creep score, and
    /// falls back to farm priority when the match was never parsed.
    pub fn derive_role(&self) -> Role {
        derive_role(
            self.lane_role,
            self.is_roaming,
            self.farm_rank,
            self.last_hits,
            self.duration_seconds,
        )
    }
}

/// Estimated position. Stored as text so new labels do not need a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Carry,
    Mid,
    Offlane,
    Support,
    HardSupport,
    Roamer,
    Jungle,
    /// Farm priority says core, but without lane data we cannot say which one.
    Core,
    Unknown,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Carry => "Carry",
            Role::Mid => "Mid",
            Role::Offlane => "Offlane",
            Role::Support => "Support",
            Role::HardSupport => "Hard Support",
            Role::Roamer => "Roamer",
            Role::Jungle => "Jungle",
            Role::Core => "Core",
            Role::Unknown => "Unknown",
        }
    }
}

/// Estimate the played position.
///
/// Precision depends on what the provider knows. `lane_role` (1 safe, 2 mid,
/// 3 off, 4 jungle) only exists for parsed replays, which most public matches
/// are not; farm priority within the team is the fallback, and it can only
/// separate cores from supports.
pub fn derive_role(
    lane_role: Option<i16>,
    is_roaming: Option<bool>,
    farm_rank: Option<u8>,
    last_hits: i32,
    duration_seconds: i32,
) -> Role {
    if is_roaming == Some(true) {
        return Role::Roamer;
    }

    let minutes = (duration_seconds as f64 / 60.0).max(1.0);
    let is_core = (last_hits as f64 / minutes) >= CORE_LAST_HITS_PER_MINUTE;

    match lane_role {
        Some(1) if is_core => return Role::Carry,
        Some(1) => return Role::HardSupport,
        Some(2) => return Role::Mid,
        Some(3) if is_core => return Role::Offlane,
        Some(3) => return Role::Support,
        Some(4) => return Role::Jungle,
        _ => {}
    }

    match farm_rank {
        Some(0..=2) => Role::Core,
        Some(3) => Role::Support,
        Some(_) => Role::HardSupport,
        None => Role::Unknown,
    }
}

/// Insert payload: a normalized match bound to a player, with the display
/// fields resolved.
#[derive(Debug, Clone)]
pub struct NewMatch {
    pub dota_player_id: Uuid,
    pub hero_name: String,
    pub role: String,
    pub data: NormalizedMatch,
}

impl NewMatch {
    pub fn new(dota_player_id: Uuid, data: NormalizedMatch, hero_name: String) -> Self {
        let role = data.derive_role().as_str().to_string();
        Self {
            dota_player_id,
            hero_name,
            role,
            data,
        }
    }
}

/// A stored match, as returned by the API.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct Match {
    pub id: Uuid,
    pub dota_player_id: Uuid,
    pub match_id: i64,
    pub hero_id: i32,
    pub hero_name: String,
    pub role: String,
    pub lane_role: Option<i16>,
    pub won: bool,
    pub duration_seconds: i32,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub gpm: i32,
    pub xpm: i32,
    pub last_hits: i32,
    pub denies: Option<i32>,
    pub net_worth: Option<i32>,
    pub hero_damage: Option<i32>,
    pub tower_damage: Option<i32>,
    pub hero_healing: Option<i32>,
    pub game_mode: Option<i32>,
    pub lobby_type: Option<i32>,
    pub party_size: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub detail_synced: bool,

    pub team_kills: Option<i32>,
    pub team_deaths: Option<i32>,
    pub replay_parsed: bool,
    pub last_hits_at_10: Option<i32>,
    pub last_hits_at_15: Option<i32>,
    pub gold_at_10: Option<i32>,
    pub gold_at_15: Option<i32>,
    pub xp_at_10: Option<i32>,
    pub xp_at_15: Option<i32>,
    pub bkb_seconds: Option<i32>,
    pub blink_seconds: Option<i32>,
    pub midas_seconds: Option<i32>,
    pub teamfight_participation: Option<f32>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    /// Derived KDA, joined from `match_metrics`. Present so the client renders
    /// a number the backend computed rather than recomputing it.
    #[sqlx(default)]
    #[serde(rename = "kda")]
    pub metrics_kda: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(last_hits: i32, lane_role: Option<i16>) -> NormalizedMatch {
        NormalizedMatch {
            match_id: 1,
            hero_id: 1,
            won: true,
            duration_seconds: 2400, // 40 minutes
            kills: 5,
            deaths: 2,
            assists: 10,
            gpm: 500,
            xpm: 600,
            last_hits,
            denies: None,
            net_worth: None,
            hero_damage: None,
            tower_damage: None,
            hero_healing: None,
            lane_role,
            is_roaming: Some(false),
            farm_rank: None,
            game_mode: Some(22),
            lobby_type: Some(7),
            party_size: Some(1),
            started_at: Utc::now(),
            from_details: false,
            team_kills: None,
            team_deaths: None,
            replay_parsed: false,
            last_hits_at_10: None,
            last_hits_at_15: None,
            gold_at_10: None,
            gold_at_15: None,
            xp_at_10: None,
            xp_at_15: None,
            bkb_seconds: None,
            blink_seconds: None,
            midas_seconds: None,
            teamfight_participation: None,
        }
    }

    #[test]
    fn safe_lane_splits_on_creep_score() {
        // 300 last hits over 40 minutes = 7.5/min -> core.
        assert_eq!(sample(300, Some(1)).derive_role(), Role::Carry);
        // 40 last hits over 40 minutes = 1.0/min -> support.
        assert_eq!(sample(40, Some(1)).derive_role(), Role::HardSupport);
    }

    #[test]
    fn offlane_splits_on_creep_score() {
        assert_eq!(sample(300, Some(3)).derive_role(), Role::Offlane);
        assert_eq!(sample(40, Some(3)).derive_role(), Role::Support);
    }

    #[test]
    fn mid_and_jungle_ignore_creep_score() {
        assert_eq!(sample(40, Some(2)).derive_role(), Role::Mid);
        assert_eq!(sample(400, Some(4)).derive_role(), Role::Jungle);
    }

    #[test]
    fn roaming_wins_over_lane_assignment() {
        let mut m = sample(300, Some(1));
        m.is_roaming = Some(true);
        assert_eq!(m.derive_role(), Role::Roamer);
    }

    #[test]
    fn missing_lane_role_falls_back_to_farm_priority() {
        // Unparsed match, no lane data at all.
        assert_eq!(sample(300, None).derive_role(), Role::Unknown);

        let mut core = sample(300, None);
        core.farm_rank = Some(1);
        assert_eq!(core.derive_role(), Role::Core);

        let mut support = sample(40, None);
        support.farm_rank = Some(3);
        assert_eq!(support.derive_role(), Role::Support);

        let mut hard_support = sample(40, None);
        hard_support.farm_rank = Some(4);
        assert_eq!(hard_support.derive_role(), Role::HardSupport);
    }

    #[test]
    fn lane_data_beats_farm_priority_when_both_exist() {
        let mut m = sample(40, Some(1));
        m.farm_rank = Some(0);
        assert_eq!(m.derive_role(), Role::HardSupport);
    }

    #[test]
    fn very_short_matches_do_not_divide_by_zero() {
        let mut m = sample(2, Some(1));
        m.duration_seconds = 0;
        assert_eq!(m.derive_role(), Role::HardSupport);
    }

    #[test]
    fn enrichment_fills_gaps_without_erasing_summary_fields() {
        let mut summary = sample(300, Some(1));
        summary.party_size = Some(3);

        let mut detail = sample(300, Some(1));
        detail.denies = Some(12);
        detail.net_worth = Some(24_000);
        detail.farm_rank = Some(0);
        detail.party_size = None; // detail endpoint does not report it

        summary.enrich_with(&detail);

        assert_eq!(summary.denies, Some(12));
        assert_eq!(summary.net_worth, Some(24_000));
        assert_eq!(summary.farm_rank, Some(0));
        assert_eq!(summary.party_size, Some(3));
        assert!(summary.from_details);
    }

    #[test]
    fn enrichment_repairs_performance_fields_the_summary_left_empty() {
        // The provider's summary projection can come back without them.
        let mut summary = sample(0, None);
        summary.gpm = 0;
        summary.xpm = 0;

        let mut detail = sample(112, None);
        detail.gpm = 379;
        detail.xpm = 516;

        summary.enrich_with(&detail);

        assert_eq!(summary.gpm, 379);
        assert_eq!(summary.xpm, 516);
        assert_eq!(summary.last_hits, 112);
    }
}
