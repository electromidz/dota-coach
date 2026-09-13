import type { Match } from "./types";

/**
 * Aggregates derived from the match list the client already holds.
 *
 * Pure and side-effect free so the charts stay presentational and every number
 * on the dashboard is unit-testable. These are display summaries, *not* the
 * deterministic coaching metrics — those are computed in Rust in Phase 4 and
 * will replace anything here that overlaps.
 */

export interface Summary {
  matches: number;
  wins: number;
  losses: number;
  /** 0-1, or `null` when there are no matches to divide by. */
  winRate: number | null;
  avgKda: number | null;
  avgGpm: number | null;
  avgXpm: number | null;
  avgDeaths: number | null;
}

export function summarize(matches: Match[]): Summary {
  const total = matches.length;

  if (total === 0) {
    return {
      matches: 0,
      wins: 0,
      losses: 0,
      winRate: null,
      avgKda: null,
      avgGpm: null,
      avgXpm: null,
      avgDeaths: null,
    };
  }

  const wins = matches.filter((m) => m.won).length;
  const mean = (pick: (m: Match) => number) =>
    matches.reduce((sum, m) => sum + pick(m), 0) / total;

  return {
    matches: total,
    wins,
    losses: total - wins,
    winRate: wins / total,
    // Per-match KDA averaged, not aggregate K+A over aggregate D: one 0-death
    // game should not dominate the number.
    avgKda: mean(matchKda),
    avgGpm: mean((m) => m.gpm),
    avgXpm: mean((m) => m.xpm),
    avgDeaths: mean((m) => m.deaths),
  };
}

/** `(kills + assists) / max(deaths, 1)` — the same rule the backend uses. */
export function matchKda(match: Match): number {
  return (match.kills + match.assists) / Math.max(match.deaths, 1);
}

/**
 * Oldest-first series for a trend line. The API returns newest-first, which
 * reads backwards on a time axis.
 */
export function kdaTrend(matches: Match[], limit = 20): number[] {
  return matches
    .slice(0, limit)
    .map(matchKda)
    .reverse();
}

/** Most recent results, oldest-first, for the form strip. */
export function recentForm(matches: Match[], limit = 10): boolean[] {
  return matches
    .slice(0, limit)
    .map((m) => m.won)
    .reverse();
}

export interface RoleShare {
  role: string;
  matches: number;
  wins: number;
  /** 0-1 share of all matches. */
  share: number;
}

/**
 * Role breakdown, most-played first.
 *
 * Anything past `keep` folds into "Other" rather than growing the category
 * count — a chart never solves crowding by inventing more colours.
 */
export function roleBreakdown(matches: Match[], keep = 4): RoleShare[] {
  if (matches.length === 0) return [];

  const counts = new Map<string, { matches: number; wins: number }>();
  for (const match of matches) {
    const entry = counts.get(match.role) ?? { matches: 0, wins: 0 };
    entry.matches += 1;
    if (match.won) entry.wins += 1;
    counts.set(match.role, entry);
  }

  const ranked = [...counts.entries()]
    .map(([role, v]) => ({ role, ...v }))
    .sort((a, b) => b.matches - a.matches || a.role.localeCompare(b.role));

  const head = ranked.slice(0, keep);
  const tail = ranked.slice(keep);

  if (tail.length > 0) {
    head.push({
      role: "Other",
      matches: tail.reduce((s, r) => s + r.matches, 0),
      wins: tail.reduce((s, r) => s + r.wins, 0),
    });
  }

  return head.map((r) => ({ ...r, share: r.matches / matches.length }));
}

export interface HeroShare {
  heroId: number;
  heroName: string;
  matches: number;
  wins: number;
}

/** Most-played heroes, for the portrait row. */
export function topHeroes(matches: Match[], limit = 5): HeroShare[] {
  const counts = new Map<number, HeroShare>();

  for (const match of matches) {
    const entry = counts.get(match.hero_id) ?? {
      heroId: match.hero_id,
      heroName: match.hero_name,
      matches: 0,
      wins: 0,
    };
    entry.matches += 1;
    if (match.won) entry.wins += 1;
    counts.set(match.hero_id, entry);
  }

  return [...counts.values()]
    .sort((a, b) => b.matches - a.matches || a.heroName.localeCompare(b.heroName))
    .slice(0, limit);
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
