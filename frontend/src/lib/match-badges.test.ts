import { describe, expect, it } from "vitest";

import type { MatchListSummary, MatchView } from "@/lib/types";

import { matchBadges } from "./match-badges";

const summary = (best: string | null, matches = 10) =>
  ({ matches, best_gpm_match_id: best }) as MatchListSummary;
const game = (id: string, deaths: number) => ({ id, deaths }) as MatchView;

describe("matchBadges", () => {
  it("marks the server's best-GPM game and deathless games", () => {
    expect(matchBadges(game("a", 0), summary("a"))).toEqual(["Best GPM", "0 deaths"]);
    expect(matchBadges(game("b", 3), summary("a"))).toEqual([]);
  });

  it("gives no best-GPM badge when there is only one game", () => {
    expect(matchBadges(game("a", 2), summary("a", 1))).toEqual([]);
  });
});
