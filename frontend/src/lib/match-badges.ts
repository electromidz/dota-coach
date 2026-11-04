import type { MatchListSummary, MatchView } from "@/lib/types";

/**
 * Standout badges for a row. "Best GPM" is the server's pick across the
 * whole filtered list, not this page; it is only worth a badge when there is
 * more than one game to beat.
 */
export function matchBadges(match: MatchView, summary: MatchListSummary | null): string[] {
  const badges: string[] = [];
  if (summary && summary.matches > 1 && summary.best_gpm_match_id === match.id) badges.push("Best GPM");
  if (match.deaths === 0) badges.push("0 deaths");
  return badges;
}
