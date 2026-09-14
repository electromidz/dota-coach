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

/* --- Hero Intelligence ---------------------------------------------------- */

export type RankBracket =
  | "herald"
  | "guardian"
  | "crusader"
  | "archon"
  | "legend"
  | "ancient"
  | "divine"
  | "immortal";

/** Where a hero sits in the player's repertoire. Derived from their history. */
export type HeroTier = "signature" | "comfort" | "stretch" | "risk";

export type RecommendationLevel = "recommended" | "consider" | "avoid_for_now";

export type FitComponentName =
  | "player_performance"
  | "meta_strength"
  | "experience"
  | "benchmark"
  | "recent_form";

export interface HeroPoolEntry {
  hero_id: number;
  hero_name: string;
  /** The player's most frequent role on this hero. */
  role: string;
  matches: number;
  wins: number;
  losses: number;
  win_rate: number;
  /** Matches inside the recent window, and the win rate across them. */
  recent_matches: number;
  recent_win_rate: number | null;
  avg_kda: number;
  avg_gpm: number;
  last_played_at: string;
  tier: HeroTier;
  tier_label: string;
  confidence: Confidence;
}

export interface PoolSummary {
  heroes: number;
  signature: number;
  comfort: number;
  stretch: number;
  risk: number;
  /** Heroes with enough matches for their figures to bear weight. */
  established: number;
}

/** One weighted input to a fit score, with the reason behind it. */
export interface FitPart {
  component: FitComponentName;
  label: string;
  /** 0-100. */
  score: number;
  /** Share of the final score, after absent components were renormalized. */
  weight: number;
  detail: string;
}

export interface HeroFit {
  hero_id: number;
  hero_name: string;
  fit_score: number;
  level: RecommendationLevel;
  level_label: string;
  parts: FitPart[];
  reasons: string[];
  caveats: string[];
  matches: number;
  tier: HeroTier | null;
  meta_strength: number | null;
  /** Points added or removed for training-focus compatibility. */
  focus_adjustment: number;
}

export interface HeroMeta {
  hero_id: number;
  hero_name: string;
  roles: string[];
  /** Picks in the cohort. This is the sample behind `win_rate`. */
  picks: number;
  wins: number;
  win_rate: number;
  pick_rate: number;
  trend: number | null;
  meta_strength: number;
  bracket: RankBracket | null;
}

/** What the recommendations were actually scored against. */
export interface MetaContext {
  available: boolean;
  source: string | null;
  bracket: RankBracket | null;
  bracket_label: string | null;
  segmented_by: Segment[];
  note: string | null;
}

export interface HeroPoolResponse {
  pool: HeroPoolEntry[];
  summary: PoolSummary;
  recent_window: number;
  note: string | null;
}

export interface HeroIntelligenceResponse {
  pool: HeroPoolEntry[];
  summary: PoolSummary;
  recommendations: HeroFit[];
  meta_leaders: HeroMeta[];
  meta: MetaContext;
  recent_window: number;
  note: string | null;
}

/* --- AI coaching ---------------------------------------------------------- */

export type EvidenceKind =
  | "overall"
  | "form"
  | "benchmark"
  | "hero"
  | "pattern"
  | "focus"
  | "match";

export type InsightKind =
  | "strength"
  | "weakness"
  | "recurring_pattern"
  | "recommendation"
  | "warning"
  | "improvement";

export type AnalysisScope = "player" | "match";

/**
 * One measured fact. `statement` is composed by the backend from its own
 * numbers — the model never writes a figure the client renders.
 */
export interface Evidence {
  id: string;
  kind: EvidenceKind;
  label: string;
  statement: string;
  sample: number;
  /** How far the figure generalizes, not whether it is accurate. */
  confidence: Confidence;
}

export interface Insight {
  kind: InsightKind;
  kind_label: string;
  title: string;
  explanation: string;
  /** Evidence ids, every one guaranteed to exist in the analysis. */
  evidence: string[];
}

export interface CoachingAnalysis {
  id: string;
  scope: AnalysisScope;
  match_id: string | null;
  model: string;
  summary: string;
  insights: Insight[];
  /** The evidence the model was shown, kept with the answer. */
  evidence: Evidence[];
  generated_at: string;
}

export interface CoachResponse {
  analysis: CoachingAnalysis | null;
  evidence: Evidence[];
  llm_available: boolean;
  /** The stored analysis predates the evidence above. */
  stale: boolean;
  cached: boolean;
  /** Deterministic, and present whether or not a model has ever run. */
  patterns: RecurringPattern[];
  note: string | null;
}

/* --- Player model --------------------------------------------------------- */

export type PatternStatus = "active" | "improving" | "resolved";

export type TraitKind = "strength" | "weakness";

export type TraitSource = "benchmark" | "hero_pool" | "form";

export type ModelConfidence = "sparse" | "developing" | "established";

/**
 * Something the player does repeatedly.
 *
 * `occurrences` and `measured` are different denominators on purpose: most
 * laning signals only exist on a parsed replay, so "3 of 4 parsed games" must
 * never be rendered as if it were "3 of 40".
 */
export interface RecurringPattern {
  id: string;
  label: string;
  description: string;
  occurrences: number;
  /** Matches the condition could be checked in. Never the career total. */
  measured: number;
  rate: number;
  recent_rate: number | null;
  recent_measured: number;
  status: PatternStatus;
  status_label: string;
  confidence: Confidence;
  statement: string;
  /** Match ids where it happened, newest first. */
  examples: string[];
  first_seen_at: string | null;
  last_seen_at: string | null;
  /** When the backend first recorded it — survives recomputation. */
  first_detected_at: string | null;
}

export interface PlayerTrait {
  kind: TraitKind;
  source: TraitSource;
  key: string;
  label: string;
  statement: string;
  sample: number;
  confidence: Confidence;
}

export interface RoleAffinity {
  role: string;
  matches: number;
  /** Share of the player's matches, 0-1. */
  share: number;
  win_rate: number;
}

export interface RecentForm {
  matches: number;
  wins: number;
  win_rate: number | null;
  /** Positive for a winning streak, negative for a losing one. */
  streak: number;
}

export interface PlayerModel {
  model_version: number;
  matches_analyzed: number;
  confidence: ModelConfidence;
  confidence_label: string;
  confidence_caveat: string;
  strengths: PlayerTrait[];
  weaknesses: PlayerTrait[];
  preferred_roles: RoleAffinity[];
  patterns: RecurringPattern[];
  /** Detected before, no longer meeting the threshold. */
  resolved_patterns: RecurringPattern[];
  recent_form: RecentForm;
  computed_at: string;
}

/** A detector that could not report, and how far short it fell. */
export interface UnmeasuredDetector {
  id: string;
  label: string;
  description: string;
  measured: number;
  required: number;
}

export interface PatternThresholds {
  min_measured: number;
  min_occurrences: number;
  min_rate: number;
}

export interface PlayerModelResponse {
  model: PlayerModel;
  unmeasurable: UnmeasuredDetector[];
  thresholds: PatternThresholds;
}

/* --- Training focus ------------------------------------------------------- */

export type FocusMeasure =
  | "deaths_per_10"
  | "kill_participation"
  | "gold_per_min"
  | "last_hits_at_10"
  | "pattern_rate";

export type FocusSource = "benchmark" | "pattern";

export type FocusStatus = "active" | "achieved" | "retired";

/** One weighted input to the selection score. */
export interface FocusScorePart {
  key: string;
  label: string;
  /** 0-100. */
  score: number;
  weight: number;
  detail: string;
}

/**
 * The one thing to work on.
 *
 * `baseline_value` is where the player stood when the focus was set and does
 * not drift; everything else is recomputed against the current history.
 */
export interface TrainingFocus {
  id: string | null;
  key: string;
  title: string;
  why: string;
  source: FocusSource;
  measure: FocusMeasure;
  measure_label: string;
  pattern_id: string | null;
  higher_is_better: boolean;
  baseline_value: number;
  target_value: number;
  current_value: number | null;
  /** 0-1 from baseline to target, clamped. */
  progress: number | null;
  target_met: boolean;
  status: FocusStatus;
  status_label: string;
  score: number;
  score_parts: FocusScorePart[];
  confidence: Confidence;
  sample: number;
  started_at: string | null;
  ended_at: string | null;
}

export interface ProgressPoint {
  matches: number;
  value: number;
  at: string;
}

export interface ProgressSeries {
  measure: FocusMeasure;
  label: string;
  higher_is_better: boolean;
  /** Oldest bucket first, so it reads left to right. */
  points: ProgressPoint[];
  /** Matches per bucket. */
  window: number;
  target_value: number | null;
}

export interface TrainingFocusResponse {
  focus: TrainingFocus | null;
  progress: ProgressSeries | null;
  /** What would be next, so "why this one" has a comparison. */
  next_up: TrainingFocus[];
  history: TrainingFocus[];
  note: string | null;
}
