import { describe, expect, it } from "vitest";

import {
  formatFixed,
  formatPercent,
  formatWhole,
  kdaSeries,
  recentForm,
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
    replay_parsed: false,
    kda: 5,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

describe("kdaSeries", () => {
  it("plots the backend's value rather than recomputing it", () => {
    // Deliberately inconsistent with kills/deaths/assists: the backend is the
    // single source of truth, so its number is the one that must appear.
    const series = kdaSeries([match({ kda: 9.9, kills: 0, assists: 0 })]);
    expect(series).toEqual([9.9]);
  });

  it("returns oldest first, because the API returns newest first", () => {
    expect(kdaSeries([match({ kda: 3 }), match({ kda: 1 })])).toEqual([1, 3]);
  });

  it("skips matches whose metrics have not been computed yet", () => {
    expect(kdaSeries([match({ kda: 2 }), match({ kda: null })])).toEqual([2]);
  });

  it("respects the limit", () => {
    expect(kdaSeries(Array.from({ length: 50 }, () => match()), 20)).toHaveLength(20);
  });

  it("survives an empty history", () => {
    expect(kdaSeries([])).toEqual([]);
  });
});

describe("recentForm", () => {
  it("returns oldest first", () => {
    expect(recentForm([match({ won: false }), match({ won: true })])).toEqual([
      true,
      false,
    ]);
  });

  it("respects the limit", () => {
    expect(recentForm(Array.from({ length: 30 }, () => match()), 12)).toHaveLength(12);
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
