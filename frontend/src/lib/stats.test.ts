import { describe, expect, it } from "vitest";

import {
  formatFixed,
  formatPercent,
  formatWhole,
  kdaTrend,
  matchKda,
  recentForm,
  roleBreakdown,
  summarize,
  topHeroes,
} from "./stats";
import type { Match } from "./types";

/** Minimal match; every test overrides only what it cares about. */
function match(overrides: Partial<Match> = {}): Match {
  return {
    id: crypto.randomUUID(),
    dota_player_id: "p",
    match_id: 1,
    hero_id: 35,
    hero_name: "Luna",
    role: "Carry",
    lane_role: 1,
    won: true,
    duration_seconds: 2400,
    kills: 8,
    deaths: 4,
    assists: 12,
    gpm: 500,
    xpm: 600,
    last_hits: 300,
    denies: 10,
    net_worth: 20000,
    hero_damage: 25000,
    tower_damage: 3000,
    hero_healing: 0,
    game_mode: 22,
    lobby_type: 7,
    party_size: 1,
    started_at: "2026-01-01T00:00:00Z",
    detail_synced: true,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

describe("matchKda", () => {
  it("counts kills and assists against deaths", () => {
    expect(matchKda(match({ kills: 8, deaths: 4, assists: 12 }))).toBe(5);
  });

  it("treats zero deaths as one rather than dividing by zero", () => {
    expect(matchKda(match({ kills: 5, deaths: 0, assists: 5 }))).toBe(10);
  });
});

describe("summarize", () => {
  it("reports nulls rather than NaN for an empty history", () => {
    const s = summarize([]);
    expect(s.matches).toBe(0);
    expect(s.winRate).toBeNull();
    expect(s.avgKda).toBeNull();
    expect(s.avgGpm).toBeNull();
  });

  it("counts wins and losses", () => {
    const s = summarize([
      match({ won: true }),
      match({ won: true }),
      match({ won: false }),
      match({ won: false }),
    ]);
    expect(s.matches).toBe(4);
    expect(s.wins).toBe(2);
    expect(s.losses).toBe(2);
    expect(s.winRate).toBe(0.5);
  });

  it("averages per match, so one zero-death game cannot dominate", () => {
    // Aggregate (K+A)/D would be 20/4 = 5. Per-match: 10/4 = 2.5 and
    // 10/max(0,1) = 10, so the mean is 6.25 — the deathless game pulls its
    // full weight instead of being diluted into a shared denominator.
    const s = summarize([
      match({ kills: 5, deaths: 4, assists: 5 }),
      match({ kills: 5, deaths: 0, assists: 5 }),
    ]);
    expect(s.avgKda).toBeCloseTo(6.25, 5);
  });

  it("averages the economy figures", () => {
    const s = summarize([match({ gpm: 400 }), match({ gpm: 600 })]);
    expect(s.avgGpm).toBe(500);
  });
});

describe("kdaTrend", () => {
  it("returns oldest first, because the API returns newest first", () => {
    const trend = kdaTrend([
      match({ kills: 3, deaths: 1, assists: 0 }), // newest -> 3
      match({ kills: 1, deaths: 1, assists: 0 }), // oldest -> 1
    ]);
    expect(trend).toEqual([1, 3]);
  });

  it("respects the limit", () => {
    expect(kdaTrend(Array.from({ length: 50 }, () => match()), 20)).toHaveLength(20);
  });

  it("survives an empty history", () => {
    expect(kdaTrend([])).toEqual([]);
  });
});

describe("recentForm", () => {
  it("returns oldest first", () => {
    const form = recentForm([match({ won: false }), match({ won: true })]);
    expect(form).toEqual([true, false]);
  });
});

describe("roleBreakdown", () => {
  it("ranks by matches played", () => {
    const rows = roleBreakdown([
      match({ role: "Mid" }),
      match({ role: "Carry" }),
      match({ role: "Carry" }),
    ]);
    expect(rows[0]).toMatchObject({ role: "Carry", matches: 2 });
    expect(rows[1]).toMatchObject({ role: "Mid", matches: 1 });
  });

  it("reports each role's share of the whole", () => {
    const rows = roleBreakdown([
      match({ role: "Carry" }),
      match({ role: "Carry" }),
      match({ role: "Mid" }),
      match({ role: "Support" }),
    ]);
    expect(rows.find((r) => r.role === "Carry")?.share).toBe(0.5);
  });

  it("folds the tail into Other rather than growing the category count", () => {
    const rows = roleBreakdown(
      [
        ...Array.from({ length: 5 }, () => match({ role: "Carry" })),
        ...Array.from({ length: 4 }, () => match({ role: "Mid" })),
        ...Array.from({ length: 3 }, () => match({ role: "Offlane" })),
        ...Array.from({ length: 2 }, () => match({ role: "Support" })),
        match({ role: "Hard Support" }),
        match({ role: "Roamer" }),
      ],
      4,
    );

    expect(rows).toHaveLength(5);
    expect(rows[4]).toMatchObject({ role: "Other", matches: 2 });
  });

  it("counts wins per role", () => {
    const rows = roleBreakdown([
      match({ role: "Carry", won: true }),
      match({ role: "Carry", won: false }),
    ]);
    expect(rows[0]).toMatchObject({ matches: 2, wins: 1 });
  });

  it("returns nothing for an empty history", () => {
    expect(roleBreakdown([])).toEqual([]);
  });
});

describe("topHeroes", () => {
  it("ranks by games played and keeps the win count", () => {
    const heroes = topHeroes([
      match({ hero_id: 1, hero_name: "Anti-Mage", won: true }),
      match({ hero_id: 1, hero_name: "Anti-Mage", won: false }),
      match({ hero_id: 5, hero_name: "Crystal Maiden", won: true }),
    ]);

    expect(heroes[0]).toMatchObject({
      heroId: 1,
      heroName: "Anti-Mage",
      matches: 2,
      wins: 1,
    });
    expect(heroes).toHaveLength(2);
  });

  it("respects the limit", () => {
    const many = Array.from({ length: 12 }, (_, i) =>
      match({ hero_id: i, hero_name: `Hero ${i}` }),
    );
    expect(topHeroes(many, 5)).toHaveLength(5);
  });
});

describe("formatters", () => {
  it("render an em dash instead of NaN when there is no value", () => {
    expect(formatPercent(null)).toBe("—");
    expect(formatFixed(null)).toBe("—");
    expect(formatWhole(null)).toBe("—");
  });

  it("round the way the UI expects", () => {
    expect(formatPercent(0.5432)).toBe("54%");
    expect(formatPercent(0)).toBe("0%");
    expect(formatFixed(3.14159)).toBe("3.1");
    expect(formatWhole(584.6)).toBe("585");
  });
});
