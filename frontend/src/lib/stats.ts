import type { Match } from "./types";

/**
 * Presentation helpers for backend-computed analytics.
 *
 * There is deliberately **no arithmetic over match data** here. Every average,
 * rate and rollup comes from `GET /api/stats`, computed in Rust and stamped
 * with a formula version, so the client and the coaching layer can never
 * disagree about what a number means. What remains is formatting, and
 * reshaping values the backend already derived into the order a chart wants.
 */

/** Backend-derived KDA per match, oldest first, for a trend line. */
export function kdaSeries(matches: Match[], limit = 20): number[] {
  return matches
    .slice(0, limit)
    .map((m) => m.kda)
    .filter((kda): kda is number => kda !== null)
    .reverse();
}

/** Recent results, oldest first, for the form strip. */
export function recentForm(matches: Match[], limit = 10): boolean[] {
  return matches
    .slice(0, limit)
    .map((m) => m.won)
    .reverse();
}

/** `0.5432` -> `"54%"`. Renders an em dash when there is nothing to show. */
export function formatPercent(value: number | null): string {
  return value === null ? "—" : `${Math.round(value * 100)}%`;
}

/** Rounds to one decimal, or an em dash. */
export function formatFixed(value: number | null, digits = 1): string {
  return value === null ? "—" : value.toFixed(digits);
}

/** Rounds to a whole number with thousands separators, or an em dash. */
export function formatWhole(value: number | null): string {
  return value === null ? "—" : Math.round(value).toLocaleString();
}
