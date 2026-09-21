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
  /** Gates the admin panel. Set only by hand in the database. */
  is_admin: boolean;
  status: "active" | "disabled";
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

/** The five roles a player can ask to be coached for. */
export type CoachableRole =
  | "carry"
  | "mid"
  | "offlane"
  | "soft_support"
  | "hard_support";

/**
 * How much weight an analysis built on this many matches can bear.
 *
 * About the size of the analysis window, not about whether a percentile may be
 * claimed — that is `Confidence`, which the benchmark engine owns.
 */
export type SampleConfidence = "limited" | "moderate" | "strong";

export type ExclusionReason =
  | "turbo"
  | "other_game_mode"
  | "non_public_lobby"
  | "mode_unknown";

export interface ExcludedGroup {
  reason: ExclusionReason;
  label: string;
  description: string;
  matches: number;
}

/** Every stored match accounted for: what was read, and what the rest were. */
export interface EligibilitySummary {
  total_matches: number;
  eligible_matches: number;
  excluded: ExcludedGroup[];
}

/** One measure's contribution to a role score, so the number can be explained. */
export interface ScoreComponent {
  key: "win_rate" | "kill_participation" | "kda" | "deaths_per_10";
  label: string;
  value: number;
  normalized: number;
  weight: number;
  sample: number;
}

export interface RolePerformance {
  role: CoachableRole;
  role_label: string;
  position: number;
  matches: number;
  wins: number;
  losses: number;
  win_rate: number;
  avg_kda: number | null;
  avg_gpm: number | null;
  avg_xpm: number | null;
  avg_last_hits_per_min: number | null;
  avg_deaths_per_10: number | null;
  avg_kill_participation: number | null;
  kill_participation_sample: number;
  /** 0-100, after the sample-size adjustment. Compare roles on this. */
  performance: number;
  /** Before that adjustment, so a thin sample is visibly thin. */
  raw_performance: number;
  confidence: SampleConfidence;
  components: ScoreComponent[];
}

/** Advisory only. Nothing downstream reads it to decide what to coach. */
export interface RoleRecommendation {
  role: CoachableRole;
  role_label: string;
  why: string;
  runner_up: CoachableRole | null;
  confidence: SampleConfidence;
}

export interface RoleAnalysis {
  analyzed_matches: number;
  confidence: SampleConfidence;
  confidence_label: string;
  confidence_caveat: string;
  /** Strongest first. */
  roles: RolePerformance[];
  /** Eligible matches no single role could be attributed to. */
  unclassified_matches: number;
  /** Eligible games a role needs before it can be recommended. */
  min_recommendable_matches: number;
  recommendation: RoleRecommendation | null;
  note: string | null;
}

/** The player's choice of role, and the circumstances it was made in. */
export interface CoachingProfile {
  id: string;
  /** The role every piece of coaching is scoped to. Always the player's. */
  selected_role: CoachableRole;
  selected_role_label: string;
  /** What was advised at the time, or null when nothing cleared the bar. */
  recommended_role: CoachableRole | null;
  recommended_role_label: string | null;
  /** True when the player was advised one role and picked another. */
  overrode_recommendation: boolean;
  analyzed_matches: number;
  selected_at: string;
  updated_at: string;
}

export interface SelectableRole {
  role: CoachableRole;
  label: string;
  position: number;
}

export interface RoleSelectionResponse {
  scope: AnalysisScopeInfo;
  analysis: RoleAnalysis;
  profile: CoachingProfile | null;
  /** All five, whether or not the player has matches in them. */
  selectable_roles: SelectableRole[];
}

/** What the numbers in a response were computed over. */
export interface AnalysisScopeInfo {
  population: string;
  description: string;
  analyzed_matches: number;
  window_limit: number;
  confidence: SampleConfidence;
  confidence_label: string;
  confidence_caveat: string;
}

/**
 * The overall player analysis.
 *
 * Competitive population only — Ranked and public All Pick. `eligibility`
 * accounts for every stored match that did not make it in, so this and the
 * match list can never appear to contradict each other without saying why.
 */
export interface StatsResponse {
  scope: AnalysisScopeInfo;
  eligibility: EligibilitySummary;
  overall: PlayerStats;
  heroes: HeroStats[];
  role_analysis: RoleAnalysis;
  /** Which formula set produced these numbers. */
  metrics_version: number;
}

/**
 * A match as the list serves it: the stored row plus the two facts a row cannot
 * work out for itself.
 *
 * Eligibility is decided in one place on the server. The client renders the
 * verdict rather than recomputing it, so the match list and the analysis
 * screens cannot drift apart.
 */
export interface MatchView extends Match {
  /** Whether coaching reads this match. */
  eligible: boolean;
  /** `Ranked All Pick`, `Turbo`, `Other mode`, … */
  mode_label: string;
}

/** How a listed page is ordered. Slugs the backend accepts verbatim. */
export type MatchSort =
  | "newest"
  | "oldest"
  | "gpm_desc"
  | "gpm_asc"
  | "kda_desc";

/** `all` keeps every result; the other two narrow it. */
export type MatchResultFilter = "all" | "win" | "loss";

/**
 * One value a filter can take, with how many matches it would leave.
 *
 * Counted by the backend over the population being browsed, never over the
 * filters already applied — so the options do not collapse to whatever is
 * currently selected.
 */
export interface FilterOption {
  /** Sent straight back as the query value: a hero id, or a role slug. */
  value: string;
  label: string;
  matches: number;
}

export interface FilterOptions {
  heroes: FilterOption[];
  roles: FilterOption[];
}

export interface MatchListResponse {
  matches: MatchView[];
  page: number;
  limit: number;
  total: number;
  total_pages: number;
  /** Which population this page was drawn from. */
  scope: "all" | "competitive";
  /** True when a hero, role or result filter narrowed this page. */
  filtered: boolean;
  sort: MatchSort;
  /** Drawn from the player's own matches — never a hardcoded hero list. */
  filters: FilterOptions;
}

export interface MatchResponse {
  match: MatchView;
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

/** A dimension the comparison asked for and could not honour. */
export interface UnavailableSegment {
  segment: Segment;
  label: string;
  reason: string;
}

/**
 * Which bracket a comparison asked for, and which one it got.
 *
 * These differ more often than is comfortable: the provider publishes nothing
 * for some hero/bracket pairs, and an unranked player has no bracket to ask
 * for. Both fall back to all ranks, which is a different peer group — so the
 * UI reads `fell_back` rather than assuming the request was honoured.
 */
export interface ResolvedBracket {
  requested: RankBracket | null;
  /** Null means the distribution covers every rank. */
  used: RankBracket | null;
  label: string;
  fell_back: boolean;
}

/**
 * Which matches sit on each side of a comparison.
 *
 * `comparable` is false against the current provider and that is not a defect
 * to hide: it does not publish which modes or ranks its distribution covers, so
 * the two populations cannot be declared equal.
 */
export interface PopulationScope {
  player: string;
  peers: string;
  comparable: boolean;
  note: string;
}

export interface BenchmarkContextInfo {
  hero_id: number;
  hero_name: string;
  role: CoachableRole | null;
  role_label: string | null;
  rank_tier: number | null;
  /** Which bracket the peer distribution actually covers. */
  bracket: ResolvedBracket;
  /** The four dimensions the product asks to compare on. */
  requested: Segment[];
  /** The ones the peer distribution genuinely covers. */
  segmented_by: Segment[];
  unavailable: UnavailableSegment[];
  population: PopulationScope;
}

/**
 * A rank bracket the peer distribution can be asked for.
 *
 * Served by the backend rather than listed here: `domain::hero::RankBracket` is
 * the one definition of Dota's ranks in this system, and a second copy in
 * TypeScript would be a second thing to keep in step.
 */
export interface BracketOption {
  value: RankBracket;
  label: string;
  /** True for the bracket the player's own rank falls in. */
  is_player_rank: boolean;
}

export interface BenchmarkResponse {
  hero_id: number;
  hero_name: string;
  sample: number;
  results: BenchmarkResult[];
  segmented_by: Segment[];
  context: BenchmarkContextInfo;
  /** Every bracket that can be compared against, in rank order. */
  brackets: BracketOption[];
  note: string | null;
}

/* --- Match comparison ----------------------------------------------------- */

/**
 * One figure and where it sits in the peer distribution.
 *
 * `percentile` is direction-corrected server-side, so 90 means "better than
 * 90% of peers" for deaths exactly as it does for gold. Nothing on the client
 * inverts anything.
 */
export interface Reading {
  value: number;
  percentile: number | null;
}

/** The same, for an average, which carries the sample it rests on. */
export interface AverageReading {
  value: number;
  percentile: number | null;
  sample: number;
  /** Applies to this reading only — a single match is not an estimate. */
  confidence: Confidence;
}

export interface MetricComparison {
  metric: string;
  label: string;
  higher_is_better: boolean;
  this_match: Reading | null;
  hero_average: AverageReading | null;
  peer_median: number | null;
  top_20_value: number | null;
}

/** The one-number summary, and what it is a summary of. */
export interface Standing {
  /** Median of this match's per-metric percentiles. */
  this_match: number | null;
  hero_average: number | null;
  metrics_counted: number;
  peer_sample_size: number | null;
}

/** A metric worth naming, good or bad. */
export interface Highlight {
  metric: string;
  label: string;
  value: number;
  percentile: number;
  /** Already carries its numbers; the client never recomputes one. */
  detail: string;
}

/** One past game on this hero, reduced to its standing. */
export interface TrendPoint {
  match_id: string;
  dota_match_id: number;
  started_at: string;
  won: boolean;
  standing: number;
  is_current: boolean;
}

export interface Suggestion {
  metric: string;
  label: string;
  percentile: number;
  player_value: number;
  peer_median: number;
  /** Null for gold, XP and damage, which stay rates. */
  whole_game_delta: number | null;
  whole_game_unit: string | null;
  text: string;
}

export interface MatchComparisonResponse {
  hero_id: number;
  hero_name: string;
  bracket: ResolvedBracket;
  /**
   * False when this match's figures must not be compared at all — a Turbo
   * game against a distribution drawn from ranked pubs, or a provider outage.
   * The raw values are still present; the percentiles are not.
   */
  comparable: boolean;
  standing: Standing;
  metrics: MetricComparison[];
  /** Newest first. */
  trend: TrendPoint[];
  delta_vs_previous: number | null;
  pros: Highlight[];
  cons: Highlight[];
  suggestion: Suggestion | null;
  context: BenchmarkContextInfo;
  note: string | null;
}

/* --- Coaching sessions, progress and conversation --------------------------- */

export type MetricUnit =
  | "count"
  | "per_minute"
  | "per10"
  | "percentile"
  /** A bounded 0-1 share: win rate, kill participation, a pattern's rate. */
  | "proportion"
  /** An unbounded ratio such as KDA. */
  | "ratio"
  /** A 0-100 composite the backend defines, such as the role score. */
  | "score";

/** One measured number, in a form a later session can be compared against. */
export interface MetricSnapshot {
  key: string;
  label: string;
  value: number;
  sample: number;
  unit: MetricUnit;
  higher_is_better: boolean;
}

export interface BenchmarkSnapshot {
  metric: string;
  label: string;
  player_value: number;
  peer_median: number | null;
  percentile: number | null;
  higher_is_better: boolean;
}

export interface HeroSnapshot {
  hero_id: number;
  hero_name: string;
  matches: number;
  wins: number;
  win_rate: number;
  avg_kda: number | null;
}

/** A session summary, as the history list serves it. */
export interface SessionSummary {
  id: string;
  role: CoachableRole;
  role_label: string;
  sequence: number;
  analyzed_match_count: number;
  newest_match_at: string | null;
  performance: number | null;
  has_analysis: boolean;
  created_at: string;
}

/** One immutable snapshot, served verbatim. */
export interface CoachingSession {
  id: string;
  role: CoachableRole;
  role_label: string;
  sequence: number;
  analyzed_match_count: number;
  analyzed_match_ids: string[];
  newest_match_at: string | null;
  performance: number | null;
  metrics: MetricSnapshot[];
  strengths: PlayerTrait[];
  weaknesses: PlayerTrait[];
  benchmarks: BenchmarkSnapshot[];
  heroes: HeroSnapshot[];
  training_focus_id: string | null;
  analysis_id: string | null;
  created_at: string;
}

export interface SessionHistoryResponse {
  sessions: SessionSummary[];
  page: number;
  limit: number;
  total: number;
  total_pages: number;
  role: CoachableRole;
  role_label: string;
}

export interface SessionResponse {
  session: CoachingSession;
}

/**
 * What happened to one metric between two sessions.
 *
 * Every one of these is the backend's judgement. The client renders it and
 * never recomputes it — `improved` is a decision, not a subtraction.
 */
export type ProgressStatus =
  | "improved"
  | "declined"
  | "stable"
  | "new_issue"
  | "resolved_issue"
  | "insufficient_data";

export interface MetricProgress {
  key: string;
  label: string;
  unit: MetricUnit;
  higher_is_better: boolean;
  previous: number | null;
  current: number | null;
  delta: number | null;
  /** Signed so positive always means better, including for deaths. */
  direction_delta: number | null;
  percent_change: number | null;
  previous_sample: number | null;
  current_sample: number | null;
  status: ProgressStatus;
  status_label: string;
  note: string | null;
}

export interface SessionProgress {
  role: CoachableRole;
  role_label: string;
  previous_session_id: string;
  previous_sequence: number;
  previous_at: string;
  current_session_id: string;
  current_sequence: number;
  current_at: string;
  performance: MetricProgress | null;
  metrics: MetricProgress[];
  headline: string | null;
}

export interface SeriesPoint {
  session_id: string;
  sequence: number;
  at: string;
  value: number;
}

export interface MetricSeries {
  key: string;
  label: string;
  unit: MetricUnit;
  higher_is_better: boolean;
  /** Oldest first. */
  points: SeriesPoint[];
}

export interface ProgressResponse {
  role: CoachableRole;
  role_label: string;
  /** Null until there are two sessions to compare. */
  comparison: SessionProgress | null;
  series: MetricSeries[];
  sessions: number;
  note: string | null;
}

export type ConversationSpeaker = "player" | "coach";

export interface ConversationMessage {
  id: string;
  speaker: ConversationSpeaker;
  content: string;
  /** Evidence ids the reply's figures came from. Empty for a player turn. */
  evidence: string[];
  model: string | null;
  created_at: string;
}

export interface ConversationResponse {
  role: CoachableRole;
  role_label: string;
  /** Oldest first. */
  messages: ConversationMessage[];
  llm_available: boolean;
  note: string | null;
}

export interface AskResponse {
  role: CoachableRole;
  role_label: string;
  message: ConversationMessage;
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
  | "match"
  /** What changed since the previous coaching session. */
  | "progress";

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

/**
 * One step of a generated training plan.
 *
 * Reaches the client only after its citations were checked against the evidence
 * the model was shown, and after every figure in its text was found in that
 * evidence.
 */
export interface PlanStep {
  /** 1-based, renumbered after validation so there are never holes. */
  position: number;
  title: string;
  action: string;
  evidence: string[];
}

export interface CoachingAnalysis {
  id: string;
  scope: AnalysisScope;
  match_id: string | null;
  model: string;
  summary: string;
  insights: Insight[];
  /** Empty when nothing the model proposed survived validation. */
  plan: PlanStep[];
  /** The evidence the model was shown, kept with the answer. */
  evidence: Evidence[];
  generated_at: string;
}

export interface CoachResponse {
  /** The role everything in this response is about. */
  role: CoachableRole | null;
  role_label: string | null;
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

/**
 * The weakest thing that was measured, when nothing clears the bar for a real
 * focus.
 *
 * Deliberately not a `TrainingFocus`: there is no target, no baseline and no
 * progress, because the evidence cannot support them. `percentile` is `null`
 * whenever the backend declined to claim one, and the UI must not fill that in
 * from `peer_median` — an early signal that reads as a conclusion is the one
 * failure mode this whole shape exists to prevent.
 */
export interface PreliminaryFocus {
  metric: string;
  label: string;
  higher_is_better: boolean;
  player_value: number;
  /** Matches behind the figure. The number the caveat quotes. */
  player_sample: number;
  peer_median: number | null;
  /** Direction-corrected, 0-100. `null` when the sample was too thin to rank. */
  percentile: number | null;
  confidence: Confidence;
  why: string;
  /** What would turn this reading into a conclusion. */
  to_confirm: string;
}

export interface TrainingFocusResponse {
  focus: TrainingFocus | null;
  /** Only ever set alongside `focus: null`. */
  preliminary: PreliminaryFocus | null;
  progress: ProgressSeries | null;
  /** What would be next, so "why this one" has a comparison. */
  next_up: TrainingFocus[];
  history: TrainingFocus[];
  note: string | null;
}

// ---------------------------------------------------------------------------
// Billing
// ---------------------------------------------------------------------------

/** What the account may do right now. Decided by the backend, never here. */
export type Entitlement = "free" | "trial" | "pro";

export type SubscriptionStatus =
  | "trialing"
  | "active"
  | "expired"
  | "cancelled"
  | "past_due";

export type PaymentStatus =
  | "pending"
  | "confirming"
  | "paid"
  | "failed"
  | "expired"
  | "refunded";

/** Where the account's current access actually came from. Independent of
 *  `status`: whichever of paying, redeeming a voucher, or an admin grant
 *  happened most recently is what this says. */
export type SubscriptionSource = "trial" | "payment" | "voucher" | "admin";

export interface Subscription {
  id: string;
  status: SubscriptionStatus;
  status_label: string;
  source: SubscriptionSource;
  plan: string;
  trial_started_at: string;
  trial_ends_at: string;
  current_period_start: string | null;
  current_period_end: string | null;
  provider: string | null;
  created_at: string;
}

export interface Payment {
  id: string;
  provider: string;
  status: PaymentStatus;
  status_label: string;
  /** Minor units of `currency`. Formatted for display, never recomputed. */
  amount_cents: number;
  currency: string;
  /** The coin the charge was actually paid in, once one is known. */
  pay_currency: string | null;
  payment_url: string | null;
  created_at: string;
  completed_at: string | null;
}

/** The offer, as the backend is configured. The price is never hard-coded here. */
export interface Plan {
  name: string;
  amount_cents: number;
  currency: string;
  period_days: number;
  trial_days: number;
}

export interface BillingResponse {
  entitlement: Entitlement;
  subscription: Subscription;
  plan: Plan;
  /** Whole days of access left, or null once it has run out. */
  days_remaining: number | null;
  access_ends_at: string | null;
  payments: Payment[];
  /** False when the deployment has no payment provider configured. */
  checkout_available: boolean;
}

export interface CheckoutResponse {
  payment: Payment;
}

/** The offer, readable without a session so the landing page can quote it. */
export interface PlanResponse {
  plan: Plan;
  checkout_available: boolean;
}

// ---------------------------------------------------------------------------
// Admin
// ---------------------------------------------------------------------------

/** One row of the admin user list, and the base of the detail view. */
export interface AdminUserSummary {
  id: string;
  steam_id: string;
  persona_name: string | null;
  avatar_url: string | null;
  status: "active" | "disabled";
  is_admin: boolean;
  created_at: string;
  last_login_at: string | null;
  /** `null` for an account that has never had a subscription materialised —
   *  in practice, one that has never logged in since trials started at
   *  signup. */
  subscription_status: SubscriptionStatus | null;
  subscription_plan: string | null;
  trial_ends_at: string | null;
  current_period_end: string | null;
}

/** One row of an account's activity timeline. `type` is not narrowed to a
 *  known set here: a future event type this build doesn't know about should
 *  still render rather than break the page. */
export interface AdminEventRecord {
  id: string;
  type: string;
  metadata: Record<string, unknown>;
  created_at: string;
}

export interface AdminUserDetail extends AdminUserSummary {
  /** Newest first. */
  events: AdminEventRecord[];
}

export interface AdminUserListResponse {
  users: AdminUserSummary[];
  page: number;
  limit: number;
  total: number;
  total_pages: number;
}

export interface AdminDailyStat {
  date: string;
  signups: number;
  /** Distinct accounts, not raw login events. */
  logins: number;
  /** Raw purchase events — two charges from one account the same day are
   *  two purchases, unlike `logins`. */
  purchases: number;
}

/** Everything `GET /admin/stats` answers. */
export interface AdminStats {
  from: string;
  to: string;
  total_users: number;
  /** Fixed-width windows ending at `to`, independent of `from`. */
  dau: number;
  wau: number;
  mau: number;
  active_trials: number;
  /** `trial_expired` events inside `[from, to]`. */
  trials_expired: number;
  /** All-time, the top of the funnel. */
  trials_started: number;
  /** All-time — "how many have ever bought". */
  paid_users: number;
  /** Right now — smaller than `paid_users` once anyone has lapsed. */
  currently_paid: number;
  /** Of the cohort that started a trial inside `[from, to]`, the percentage
   *  that has purchased by now. `null` when no trial started in the window. */
  trial_to_paid_conversion_pct: number | null;
  /** `voucher_redeemed` events inside `[from, to]` — kept separate from the
   *  payment-based conversion above; a voucher redemption is not a purchase. */
  voucher_redemptions: number;
  revenue_cents: number;
  currency: string;
  /** Oldest first, `from`..=`to` with no gaps. */
  daily: AdminDailyStat[];
}

// ---------------------------------------------------------------------------
// Vouchers
// ---------------------------------------------------------------------------

export interface Voucher {
  id: string;
  code: string;
  duration_days: number;
  max_uses: number;
  used_count: number;
  expires_at: string | null;
  active: boolean;
  /** The admin's own label. Never shown to the redeeming user. */
  note: string | null;
  /** `null` once the admin who made it no longer has an account. */
  created_by: string | null;
  created_at: string;
}

/** One redemption, with who redeemed it — the admin voucher-detail page. */
export interface VoucherRedemptionSummary {
  id: string;
  user_id: string;
  steam_id: string;
  persona_name: string | null;
  redeemed_at: string;
}

export interface AdminVoucherDetail extends Voucher {
  /** Newest first. */
  redemptions: VoucherRedemptionSummary[];
}

export interface VoucherListResponse {
  vouchers: Voucher[];
  page: number;
  limit: number;
  total: number;
  total_pages: number;
}

export interface RedeemResponse {
  subscription: Subscription;
}
