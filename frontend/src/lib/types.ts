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

export interface MatchListResponse {
  matches: MatchView[];
  page: number;
  limit: number;
  total: number;
  total_pages: number;
  /** Which population this page was drawn from. */
  scope: "all" | "competitive";
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
  /** The four dimensions the product asks to compare on. */
  requested: Segment[];
  /** The ones the peer distribution genuinely covers. */
  segmented_by: Segment[];
  unavailable: UnavailableSegment[];
  population: PopulationScope;
}

export interface BenchmarkResponse {
  hero_id: number;
  hero_name: string;
  sample: number;
  results: BenchmarkResult[];
  segmented_by: Segment[];
  context: BenchmarkContextInfo;
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

export interface TrainingFocusResponse {
  focus: TrainingFocus | null;
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

export interface Subscription {
  id: string;
  status: SubscriptionStatus;
  status_label: string;
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
