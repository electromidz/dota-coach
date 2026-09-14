import type {
  ApiErrorBody,
  BenchmarkResponse,
  BillingResponse,
  CheckoutResponse,
  CoachResponse,
  HealthResponse,
  HeroIntelligenceResponse,
  HeroPoolResponse,
  MatchListResponse,
  MatchResponse,
  MeResponse,
  PlayerModelResponse,
  StatsResponse,
  SyncResponse,
  TrainingFocusResponse,
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
    const body = (await response.json().catch(() => null)) as ApiErrorBody | null;
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

/** Peer comparison for one hero. Defaults to the most-played. */
export function getBenchmark(heroId?: number): Promise<BenchmarkResponse> {
  const query = heroId === undefined ? "" : `?hero_id=${heroId}`;
  return apiFetch<BenchmarkResponse>(`/api/benchmark${query}`);
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
export function getHeroIntelligence(limit?: number): Promise<HeroIntelligenceResponse> {
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

export function getMatchAnalysis(id: string): Promise<CoachResponse> {
  return apiFetch<CoachResponse>(`/api/matches/${id}/analysis`);
}

export function analyzeMatch(id: string): Promise<CoachResponse> {
  return apiFetch<CoachResponse>(`/api/matches/${id}/analyze`, {
    method: "POST",
  });
}

export function getMatches(page = 1, limit = 20): Promise<MatchListResponse> {
  return apiFetch<MatchListResponse>(`/api/matches?page=${page}&limit=${limit}`);
}

export function getMatch(id: string): Promise<MatchResponse> {
  return apiFetch<MatchResponse>(`/api/matches/${id}`);
}

export function logout(): Promise<unknown> {
  return apiFetch("/api/auth/logout", { method: "POST" });
}

/** Trial, subscription, price and charge history in one payload. */
export function getBilling(): Promise<BillingResponse> {
  return apiFetch<BillingResponse>("/api/billing");
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
