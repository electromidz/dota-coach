/** Shapes returned by the Rust backend. Kept in one place so components never
 *  guess at the API contract. */

export type DependencyStatus = "up" | "down";

export interface HealthResponse {
  status: string;
  version: string;
  database: DependencyStatus;
  llm_configured: boolean;
}

/** The signed-in Steam account. */
export interface User {
  id: string;
  /** A string, not a number: SteamID64 exceeds JavaScript's safe integer range. */
  steam_id: string;
  persona_name: string | null;
  avatar_url: string | null;
  profile_url: string | null;
  last_login_at: string | null;
  created_at: string;
  updated_at: string;
}

/** The Dota identity linked to that account. */
export interface DotaPlayer {
  id: string;
  user_id: string;
  steam_id: string;
  dota_account_id: number;
  rank_tier: number | null;
  last_synced_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface MeResponse {
  user: User;
  dota_player: DotaPlayer;
  matches_stored: number;
}

export interface SyncReport {
  matches_seen: number;
  new_matches: number;
  duplicates_skipped: number;
  details_enriched: number;
  details_failed: number;
}

export interface SyncResponse {
  dota_player: DotaPlayer;
  sync: SyncReport;
}

export interface Match {
  id: string;
  dota_player_id: string;
  match_id: number;
  hero_id: number;
  hero_name: string;
  /** Estimated position; see the README on why this is not authoritative. */
  role: string;
  lane_role: number | null;
  won: boolean;
  duration_seconds: number;
  kills: number;
  deaths: number;
  assists: number;
  gpm: number;
  xpm: number;
  last_hits: number;
  denies: number | null;
  net_worth: number | null;
  hero_damage: number | null;
  tower_damage: number | null;
  hero_healing: number | null;
  game_mode: number | null;
  lobby_type: number | null;
  party_size: number | null;
  started_at: string;
  detail_synced: boolean;
  replay_parsed: boolean;
  /** Derived by the backend. Never recomputed here. */
  kda: number | null;
  created_at: string;
  updated_at: string;
}

/** Aggregates from `GET /api/stats`. Every value is computed in Rust. */
export interface PlayerStats {
  matches: number;
  wins: number;
  losses: number;
  win_rate: number | null;
  avg_kda: number | null;
  avg_gpm: number | null;
  avg_xpm: number | null;
  avg_last_hits: number | null;
  avg_deaths_per_10: number | null;
  avg_kills_per_10: number | null;
  avg_hero_damage: number | null;
  avg_kill_participation: number | null;
  /** How many matches actually carried the kill-participation input. */
  kill_participation_sample: number;
  /** Time-sliced metrics are gated on a parsed replay. */
  parsed_matches: number;
}

export interface HeroStats {
  hero_id: number;
  hero_name: string;
  matches: number;
  wins: number;
  win_rate: number;
  avg_kda: number;
  avg_gpm: number;
  last_played_at: string;
}

export interface RoleStats {
  role: string;
  matches: number;
  wins: number;
  win_rate: number;
  avg_kda: number;
  avg_gpm: number;
}

export interface StatsResponse {
  overall: PlayerStats;
  heroes: HeroStats[];
  roles: RoleStats[];
  /** Which formula set produced these numbers. */
  metrics_version: number;
}

export interface MatchListResponse {
  matches: Match[];
  page: number;
  limit: number;
  total: number;
  total_pages: number;
}

export interface MatchResponse {
  match: Match;
}

/** Uniform error envelope produced by the backend's `AppError`. */
export interface ApiErrorBody {
  error: {
    code: string;
    message: string;
  };
}

/** How much weight a player's own figure can bear. */
export type Confidence = "insufficient" | "low" | "adequate";

/** Dimensions a benchmark was genuinely segmented on. */
export type Segment = "hero" | "role" | "rank_bracket" | "patch";

export interface BenchmarkResult {
  metric: string;
  label: string;
  higher_is_better: boolean;
  player_value: number;
  player_sample: number;
  /** The provider's 50th percentile — a median, not a mean. */
  peer_median: number | null;
  top_20_value: number | null;
  /** 0-100, direction-corrected. Null when the sample is too thin to rank. */
  percentile: number | null;
  /** Positive always means "work to do". */
  gap_to_top_20: number | null;
  peer_sample_size: number | null;
  confidence: Confidence;
  segmented_by: Segment[];
  note: string | null;
}

export interface BenchmarkResponse {
  hero_id: number;
  hero_name: string;
  sample: number;
  results: BenchmarkResult[];
  segmented_by: Segment[];
  note: string | null;
}
