import type {
  ApiErrorBody,
  HealthResponse,
  MatchListResponse,
  MatchResponse,
  MeResponse,
  SyncResponse,
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
}

export function baseUrl(): string {
  return (process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080").replace(
    /\/+$/,
    "",
  );
}

/** Where the browser goes to start a Steam login. A full navigation, not fetch. */
export function steamLoginUrl(): string {
  return `${baseUrl()}/auth/steam/login`;
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

export function getMatches(page = 1, limit = 20): Promise<MatchListResponse> {
  return apiFetch<MatchListResponse>(`/api/matches?page=${page}&limit=${limit}`);
}

export function getMatch(id: string): Promise<MatchResponse> {
  return apiFetch<MatchResponse>(`/api/matches/${id}`);
}

export function logout(): Promise<unknown> {
  return apiFetch("/api/auth/logout", { method: "POST" });
}
