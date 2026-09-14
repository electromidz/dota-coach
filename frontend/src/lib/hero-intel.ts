import type { HeroFit, HeroPoolEntry, HeroTier, RecommendationLevel } from "./types";

/**
 * Presentation helpers for Hero Intelligence.
 *
 * As with `stats.ts`, there is deliberately **no scoring here**. Fit scores,
 * tiers, levels and every explanation come from the backend; this file decides
 * what colour they wear and which order they sit in.
 */

/** Tier order, strongest repertoire first. Used for grouping the pool. */
export const TIER_ORDER: HeroTier[] = ["signature", "comfort", "stretch", "risk"];

/** What each tier means, in the player's terms rather than the spec's. */
export const TIER_BLURB: Record<HeroTier, string> = {
  signature: "Deep history, results above your own average.",
  comfort: "Played enough to be reliable, results around your average.",
  stretch: "Too few games to judge yet.",
  risk: "Real history, results below your own average.",
};

/**
 * Token colours per tier and level.
 *
 * Ordinal, not categorical: these run from "strongest" to "weakest" in both
 * scales, so they deliberately reuse the same good/warn/bad ramp rather than
 * inventing a second palette that would have to be learned separately.
 */
export const TIER_CLASS: Record<HeroTier, string> = {
  signature: "border-keyword/50 bg-keyword/10 text-keyword",
  comfort: "border-function/50 bg-function/10 text-function",
  stretch: "border-border bg-surface-2 text-ink-muted",
  risk: "border-error/50 bg-error/10 text-error",
};

export const LEVEL_CLASS: Record<RecommendationLevel, string> = {
  recommended: "border-string/50 bg-string/10 text-string",
  consider: "border-number/50 bg-number/10 text-number",
  avoid_for_now: "border-error/50 bg-error/10 text-error",
};

/** Group a pool into tiers, dropping the ones nobody is in. */
export function byTier(
  pool: HeroPoolEntry[],
): Array<{ tier: HeroTier; heroes: HeroPoolEntry[] }> {
  return TIER_ORDER.map((tier) => ({
    tier,
    heroes: pool.filter((entry) => entry.tier === tier),
  })).filter((group) => group.heroes.length > 0);
}

/**
 * Split recommendations into the ones being put forward and the rest.
 *
 * The spec asks for one clear answer rather than a wall of heroes, so the
 * "avoid for now" tail is collapsed behind its own heading instead of padding
 * the top of the page.
 */
export function splitByLevel(recommendations: HeroFit[]): {
  leading: HeroFit[];
  rest: HeroFit[];
} {
  const promoted = recommendations.filter((r) => r.level !== "avoid_for_now");
  // Never show an empty page: if nothing clears the bar, the closest hero is
  // still shown, with its own "avoid for now" label intact.
  const leading = promoted.length > 0 ? promoted : recommendations.slice(0, 1);

  return {
    leading,
    rest: recommendations.filter((r) => !leading.includes(r)),
  };
}

/** `84.62` -> `"85"`. Fit scores are never shown with false precision. */
export function formatScore(value: number | null): string {
  return value === null ? "—" : Math.round(value).toString();
}
