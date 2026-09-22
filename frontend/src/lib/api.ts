import type {
  AdminStats,
  AdminUserDetail,
  AdminUserListResponse,
  AdminUserSummary,
  AdminVoucherDetail,
  ApiErrorBody,
  AskResponse,
  BenchmarkResponse,
  BillingResponse,
  CalibrationResponse,
  CheckoutResponse,
  CoachableRole,
  CoachResponse,
  ConversationResponse,
  HealthResponse,
  HeroIntelligenceResponse,
  HeroPoolResponse,
  MatchComparisonResponse,
  MatchListResponse,
  MatchResponse,
  MatchResultFilter,
  MatchSort,
  MeResponse,
  PlanResponse,
  PlayerModelResponse,
  ProgressResponse,
  RankBracket,
  RankDistribution,
  RedeemResponse,
  RoleSelectionResponse,
  SessionHistoryResponse,
  SessionResponse,
  StatsResponse,
  SyncResponse,
  TrainingFocusResponse,
  Voucher,
  VoucherListResponse,
} from "./types";

/**
 * Error carrying the backend's machine-readable code alongside a message that
 * is always safe to render.
 */
export class ApiError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = "ApiError";
  }

  /** The caller has no session; the UI should show the signed-out state. */
  get isUnauthenticated(): boolean {
    return this.code === "UNAUTHENTICATED";
  }

  /**
   * The trial has ended and nothing is paid for. A state, not a failure: the
   * UI should offer the subscription rather than report an error.
   */
  get isPaymentRequired(): boolean {
    return this.code === "PAYMENT_REQUIRED";
  }
}

export function baseUrl(): string {
  return (process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080").replace(
    /\/+$/,
    "",
  );
}

/** Where the browser goes to start a Steam login. A full navigation, not fetch. */
export function steamLoginUrl(): string {
  return `${baseUrl()}/api/auth/steam`;
}

/**
 * Single entry point for backend calls. Normalizes transport failures and
 * error envelopes into `ApiError`, so callers only handle one error type.
 *
 * `credentials: "include"` is what carries the session cookie cross-origin;
 * the backend's CORS layer allows exactly this frontend's origin.
 */
export async function apiFetch<T>(
  path: string,
  init?: RequestInit,
): Promise<T> {
  let response: Response;

  try {
    response = await fetch(`${baseUrl()}${path}`, {
      ...init,
      credentials: "include",
      headers: { "Content-Type": "application/json", ...init?.headers },
      cache: "no-store",
    });
  } catch {
    throw new ApiError(
      "NETWORK_ERROR",
      "Could not reach the coaching service. Is the backend running?",
      0,
    );
  }

  if (!response.ok) {
    const body = (await response
      .json()
      .catch(() => null)) as ApiErrorBody | null;
    throw new ApiError(
      body?.error?.code ?? "UNKNOWN_ERROR",
      body?.error?.message ?? `Request failed with status ${response.status}.`,
      response.status,
    );
  }

  return (await response.json()) as T;
}

export function getHealth(): Promise<HealthResponse> {
  return apiFetch<HealthResponse>("/health");
}

export function getMe(): Promise<MeResponse> {
  return apiFetch<MeResponse>("/api/players/me");
}

export function syncMatches(): Promise<SyncResponse> {
  return apiFetch<SyncResponse>("/api/players/me/sync", { method: "POST" });
}

/** Aggregated analytics. All arithmetic happens in the backend. */
export function getStats(): Promise<StatsResponse> {
  return apiFetch<StatsResponse>("/api/stats");
}

/**
 * Established rank, rank confidence, trajectory, streak and role split.
 *
 * Ranked lobbies only, and every figure is computed server-side — including
 * which trajectory points are measured and which are modeled. Nothing here is
 * derived in the browser.
 */
export function getCalibration(): Promise<CalibrationResponse> {
  return apiFetch<CalibrationResponse>("/api/calibration");
}

/**
 * Where the player's numbers sit against every rank bracket.
 *
 * A separate call from `getCalibration` on purpose: this one needs the
 * benchmark provider, so an outage empties this panel without taking the rank
 * and trajectory down with it.
 */
export function getRankDistribution(): Promise<RankDistribution> {
  return apiFetch<RankDistribution>("/api/calibration/brackets");
}

/**
 * Peer comparison for one hero.
 *
 * Defaults to the most-played hero in scope, and the scope defaults to the role
 * being coached — pass `"all"` to compare across every role. Game-mode
 * eligibility is never a preference: Turbo is excluded either way.
 *
 * `bracket` names the rank to hold up *beside* the player's own — it never
 * replaces it, so `results` and their percentiles always describe the peers the
 * player actually plays against. Omit it for the next bracket up, which is what
 * the page shows by default; pass `"none"` to drop the target entirely.
 *
 * Where the provider publishes nothing for that hero in that bracket, `target`
 * comes back `null` rather than all-ranks numbers wearing a bracket's name.
 */
export function getBenchmark(
  heroId?: number,
  role?: CoachableRole | "all",
  bracket?: RankBracket | "none",
): Promise<BenchmarkResponse> {
  const params = new URLSearchParams();
  if (heroId !== undefined) params.set("hero_id", String(heroId));
  if (role !== undefined) params.set("role", role);
  // Absent is meaningful — it asks the server for the next bracket up — so
  // "none" has to be sent explicitly to mean the opposite.
  if (bracket !== undefined) params.set("bracket", bracket);

  const query = params.toString();
  return apiFetch<BenchmarkResponse>(
    `/api/benchmark${query ? `?${query}` : ""}`,
  );
}

/**
 * The player's own hero pool.
 *
 * Reads no provider, so it keeps answering when the meta is unavailable.
 */
export function getHeroPool(): Promise<HeroPoolResponse> {
  return apiFetch<HeroPoolResponse>("/api/heroes");
}

/** Pool, meta and scored recommendations in one payload. */
export function getHeroIntelligence(
  limit?: number,
): Promise<HeroIntelligenceResponse> {
  const query = limit === undefined ? "" : `?limit=${limit}`;
  return apiFetch<HeroIntelligenceResponse>(`/api/hero-intelligence${query}`);
}

/** Measured evidence plus the last analysis. Never spends a model call. */
export function getCoach(): Promise<CoachResponse> {
  return apiFetch<CoachResponse>("/api/coach");
}

/** The only call that asks the model. Rate limited server-side. */
export function analyzeCoach(): Promise<CoachResponse> {
  return apiFetch<CoachResponse>("/api/coach/analyze", { method: "POST" });
}

/**
 * The long-term model: traits, role affinity and recurring patterns.
 *
 * Deterministic throughout — reading it never calls a model, and it is
 * meaningful on a deployment with no LLM configured at all.
 */
export function getPlayerModel(): Promise<PlayerModelResponse> {
  return apiFetch<PlayerModelResponse>("/api/coach/player-model");
}

/**
 * The one thing to work on, its progress, and the runners-up.
 *
 * Reading this is what selects a focus when none is set — deliberately, so
 * that opening the coach does not commit a player to a goal by accident.
 */
export function getTrainingFocus(): Promise<TrainingFocusResponse> {
  return apiFetch<TrainingFocusResponse>("/api/coach/training-focus");
}

/**
 * Role performance, the advisory pick, and whatever the player has chosen.
 *
 * Deterministic and free — choosing what to work on is not a model call.
 */
export function getRoleSelection(): Promise<RoleSelectionResponse> {
  return apiFetch<RoleSelectionResponse>("/api/coach/roles");
}

/**
 * Choose the role to be coached on.
 *
 * Any of the five is accepted, including one the backend did not recommend.
 * The recommendation is advice; this call is the decision.
 */
export function selectCoachingRole(
  role: CoachableRole,
): Promise<RoleSelectionResponse> {
  return apiFetch<RoleSelectionResponse>("/api/coach/role", {
    method: "POST",
    body: JSON.stringify({ role }),
  });
}

export function getMatchAnalysis(id: string): Promise<CoachResponse> {
  return apiFetch<CoachResponse>(`/api/matches/${id}/analysis`);
}

export function analyzeMatch(id: string): Promise<CoachResponse> {
  return apiFetch<CoachResponse>(`/api/matches/${id}/analyze`, {
    method: "POST",
  });
}

/**
 * Which rows of a population to list, and in what order.
 *
 * Every field is optional and an absent one means "do not filter on this",
 * matching the backend. `heroId` is a hero id rather than a name because that
 * is what the match rows key on.
 */
export interface MatchListParams {
  heroId?: number;
  role?: CoachableRole;
  result?: MatchResultFilter;
  sort?: MatchSort;
}

/**
 * One page of matches.
 *
 * `scope` defaults to the player's whole history, which is what the match list
 * shows. Pass `"competitive"` for anything that is *analysing* — the dashboard's
 * trend line and form strip read the same games its statistics do, so a chart
 * and the number above it cannot disagree.
 *
 * Filtering and sorting happen in SQL, not here: the list is server-paginated,
 * so a client-side filter could only ever narrow the twenty rows it happens to
 * hold and would report a page count for a different list than the one on
 * screen.
 */
export function getMatches(
  page = 1,
  limit = 20,
  scope: "all" | "competitive" = "all",
  params: MatchListParams = {},
): Promise<MatchListResponse> {
  const query = new URLSearchParams({
    page: String(page),
    limit: String(limit),
  });
  if (scope !== "all") query.set("scope", scope);
  if (params.heroId !== undefined) query.set("hero_id", String(params.heroId));
  if (params.role) query.set("role", params.role);
  if (params.result && params.result !== "all") {
    query.set("result", params.result);
  }
  if (params.sort && params.sort !== "newest") query.set("sort", params.sort);

  return apiFetch<MatchListResponse>(`/api/matches?${query.toString()}`);
}

export function getMatch(id: string): Promise<MatchResponse> {
  return apiFetch<MatchResponse>(`/api/matches/${id}`);
}

/**
 * This match against players in the same rank bracket on the same hero.
 *
 * Separate from `getMatch` on purpose: it reaches an external benchmark
 * provider, so it is the slow half of the page and must not hold up the
 * match's own figures.
 */
export function getMatchComparison(
  id: string,
): Promise<MatchComparisonResponse> {
  return apiFetch<MatchComparisonResponse>(`/api/matches/${id}/comparison`);
}

/** One page of coaching history for a role. Newest first. */
export function getCoachingSessions(
  page = 1,
  limit = 20,
  role?: string,
): Promise<SessionHistoryResponse> {
  const query = `page=${page}&limit=${limit}${role ? `&role=${role}` : ""}`;
  return apiFetch<SessionHistoryResponse>(`/api/coach/sessions?${query}`);
}

/** One stored snapshot, exactly as it was recorded. */
export function getCoachingSession(id: string): Promise<SessionResponse> {
  return apiFetch<SessionResponse>(`/api/coach/sessions/${id}`);
}

/**
 * What changed since the previous session, plus the trend behind it.
 *
 * Every status is the backend's judgement. Nothing here is recomputed on the
 * client — see `lib/stats.ts` for why that rule exists.
 */
export function getCoachingProgress(role?: string): Promise<ProgressResponse> {
  return apiFetch<ProgressResponse>(
    `/api/coach/progress${role ? `?role=${role}` : ""}`,
  );
}

/** The coaching conversation for a role. Never calls a model. */
export function getConversation(role?: string): Promise<ConversationResponse> {
  return apiFetch<ConversationResponse>(
    `/api/coach/conversation${role ? `?role=${role}` : ""}`,
  );
}

/**
 * Ask the coach a question.
 *
 * Spends a model call, so it is rate limited and entitlement-gated server
 * side. A reply quoting a figure the player's data does not contain is
 * discarded by the backend and answers 502 rather than being shown.
 */
export function askCoach(question: string): Promise<AskResponse> {
  return apiFetch<AskResponse>("/api/coach/conversation", {
    method: "POST",
    body: JSON.stringify({ question }),
  });
}

export function logout(): Promise<unknown> {
  return apiFetch("/api/auth/logout", { method: "POST" });
}

/**
 * The offer alone — trial length and price — with no session required.
 *
 * The landing page has to state both, and the alternative is hard-coding them
 * where they would drift from what the backend actually charges.
 */
export function getPlan(): Promise<PlanResponse> {
  return apiFetch<PlanResponse>("/api/billing/plan");
}

/** Trial, subscription, price and charge history in one payload. */
export function getBilling(): Promise<BillingResponse> {
  return apiFetch<BillingResponse>("/api/billing");
}

/**
 * Redeem a voucher code for subscription time.
 *
 * Any signed-in account may call this — existing access is not a
 * precondition, since redeeming is how an account with none gets some.
 */
export function redeemVoucher(code: string): Promise<RedeemResponse> {
  return apiFetch<RedeemResponse>("/api/subscribe/redeem", {
    method: "POST",
    body: JSON.stringify({ code }),
  });
}

// ---------------------------------------------------------------------------
// Admin
// ---------------------------------------------------------------------------

/** Usage, trial and revenue counts. `from`/`to` are RFC3339; both default to
 *  the last 30 days on the backend when omitted. */
export function getAdminStats(from?: string, to?: string): Promise<AdminStats> {
  const params = new URLSearchParams();
  if (from) params.set("from", from);
  if (to) params.set("to", to);

  const query = params.toString();
  return apiFetch<AdminStats>(`/api/admin/stats${query ? `?${query}` : ""}`);
}

export interface AdminUserListParams {
  page?: number;
  limit?: number;
  status?: string;
  plan?: string;
  search?: string;
}

/** One page of accounts, optionally filtered. Every param is optional; an
 *  absent one means "don't filter on this", matching the backend. */
export function getAdminUsers(
  params: AdminUserListParams = {},
): Promise<AdminUserListResponse> {
  const query = new URLSearchParams();
  if (params.page) query.set("page", String(params.page));
  if (params.limit) query.set("limit", String(params.limit));
  if (params.status) query.set("status", params.status);
  if (params.plan) query.set("plan", params.plan);
  if (params.search) query.set("search", params.search);

  const qs = query.toString();
  return apiFetch<AdminUserListResponse>(
    `/api/admin/users${qs ? `?${qs}` : ""}`,
  );
}

export function getAdminUser(id: string): Promise<AdminUserDetail> {
  return apiFetch<AdminUserDetail>(`/api/admin/users/${id}`);
}

/** Adds `days` to whichever window currently governs the account's access —
 *  the trial end for a trialing/expired account, the paid-through date for
 *  an active/past-due one — and pulls a lapsed row back to `trialing`. */
export function extendAccess(
  id: string,
  days: number,
): Promise<AdminUserSummary> {
  return apiFetch<AdminUserSummary>(`/api/admin/users/${id}/extend`, {
    method: "POST",
    body: JSON.stringify({ days }),
  });
}

/** Every future request from this account is refused, checked fresh each
 *  time — no separate session revocation needed. */
export function disableUser(id: string): Promise<AdminUserSummary> {
  return apiFetch<AdminUserSummary>(`/api/admin/users/${id}/disable`, {
    method: "POST",
  });
}

export function enableUser(id: string): Promise<AdminUserSummary> {
  return apiFetch<AdminUserSummary>(`/api/admin/users/${id}/enable`, {
    method: "POST",
  });
}

export interface CreateVoucherParams {
  duration_days: number;
  max_uses: number;
  expires_at?: string;
  note?: string;
  /** How many independent codes to generate. Defaults to 1 on the backend —
   *  a "bulk" creation is this same request with a bigger number. */
  count?: number;
}

/** Always returns a list, even for a single voucher, so the caller never
 *  branches on shape. */
export function createVouchers(
  params: CreateVoucherParams,
): Promise<VoucherListResponse> {
  return apiFetch<VoucherListResponse>("/api/admin/vouchers", {
    method: "POST",
    body: JSON.stringify(params),
  });
}

export function getAdminVouchers(
  page = 1,
  limit = 20,
): Promise<VoucherListResponse> {
  return apiFetch<VoucherListResponse>(
    `/api/admin/vouchers?page=${page}&limit=${limit}`,
  );
}

export function getAdminVoucher(id: string): Promise<AdminVoucherDetail> {
  return apiFetch<AdminVoucherDetail>(`/api/admin/vouchers/${id}`);
}

/** Existing redemptions are untouched — only future ones are refused. */
export function deactivateVoucher(id: string): Promise<Voucher> {
  return apiFetch<Voucher>(`/api/admin/vouchers/${id}/deactivate`, {
    method: "POST",
  });
}

/**
 * Open a charge, or get back the one that is still open.
 *
 * The backend decides the amount; nothing about the price travels from here.
 */
export function startCheckout(): Promise<CheckoutResponse> {
  return apiFetch<CheckoutResponse>("/api/billing/checkout", {
    method: "POST",
  });
}
