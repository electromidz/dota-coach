import { describe, expect, it } from "vitest";

import { byTier, formatScore, splitByLevel, TIER_ORDER } from "./hero-intel";
import type { HeroFit, HeroPoolEntry, HeroTier, RecommendationLevel } from "./types";

function entry(hero_id: number, tier: HeroTier): HeroPoolEntry {
  return {
    hero_id,
    hero_name: `Hero ${hero_id}`,
    role: "Carry",
    matches: 10,
    wins: 5,
    losses: 5,
    win_rate: 0.5,
    recent_matches: 10,
    recent_win_rate: 0.5,
    avg_kda: 3,
    avg_gpm: 500,
    last_played_at: new Date().toISOString(),
    tier,
    tier_label: tier,
    confidence: "adequate",
  };
}

function fit(hero_id: number, level: RecommendationLevel): HeroFit {
  return {
    hero_id,
    hero_name: `Hero ${hero_id}`,
    fit_score: 60,
    level,
    level_label: level,
    parts: [],
    reasons: [],
    caveats: [],
    matches: 10,
    tier: "comfort",
    meta_strength: 70,
    focus_adjustment: 0,
  };
}

describe("byTier", () => {
  it("groups strongest first and drops empty tiers", () => {
    const groups = byTier([entry(1, "risk"), entry(2, "signature"), entry(3, "risk")]);

    expect(groups.map((g) => g.tier)).toEqual(["signature", "risk"]);
    expect(groups[1].heroes).toHaveLength(2);
  });

  it("keeps every tier the backend can send", () => {
    const groups = byTier(TIER_ORDER.map((tier, i) => entry(i, tier)));
    expect(groups).toHaveLength(TIER_ORDER.length);
  });
});

describe("splitByLevel", () => {
  it("separates the heroes being put forward from the rest", () => {
    const { leading, rest } = splitByLevel([
      fit(1, "recommended"),
      fit(2, "consider"),
      fit(3, "avoid_for_now"),
    ]);

    expect(leading.map((f) => f.hero_id)).toEqual([1, 2]);
    expect(rest.map((f) => f.hero_id)).toEqual([3]);
  });

  it("still shows one hero when nothing clears the bar", () => {
    const { leading, rest } = splitByLevel([
      fit(1, "avoid_for_now"),
      fit(2, "avoid_for_now"),
    ]);

    // The page never goes blank, and the hero keeps its honest label.
    expect(leading.map((f) => f.hero_id)).toEqual([1]);
    expect(leading[0].level).toBe("avoid_for_now");
    // And it is not repeated in the collapsed tail.
    expect(rest.map((f) => f.hero_id)).toEqual([2]);
  });

  it("handles an empty list", () => {
    expect(splitByLevel([])).toEqual({ leading: [], rest: [] });
  });
});

describe("formatScore", () => {
  it("rounds and never invents precision", () => {
    expect(formatScore(84.62)).toBe("85");
    expect(formatScore(0)).toBe("0");
  });

  it("renders an em dash when there is no score", () => {
    expect(formatScore(null)).toBe("—");
  });
});
