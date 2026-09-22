import type { PlanResponse } from "./types";

/**
 * The public plan, fetched on the server so the landing page ships the trial
 * length and the price in its first byte.
 *
 * The client hook alone was enough while the page only existed after
 * hydration; now that the marketing page is server-rendered for crawlers, a
 * price that only appears after a second round trip is a price that is not in
 * the indexed HTML — and a visible layout jump for everyone else.
 *
 * `/api/billing/plan` is unauthenticated and the same for every visitor, so
 * the response is cached for an hour rather than refetched per request.
 * Failure returns `null`: as with `usePlan`, the honest fallback is to render
 * no figure, never a guessed one.
 */
export async function fetchPlan(): Promise<PlanResponse | null> {
  const base = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

  try {
    const response = await fetch(`${base}/api/billing/plan`, {
      next: { revalidate: 3600 },
    });
    if (!response.ok) return null;

    return (await response.json()) as PlanResponse;
  } catch {
    // The backend being unreachable must not take the marketing page down
    // with it — the pitch, the FAQ and the sign-in link do not depend on it.
    return null;
  }
}
