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
  created_at: string;
  updated_at: string;
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
