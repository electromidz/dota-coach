import { describe, expect, it } from "vitest";

import type { TrajectoryPoint } from "@/lib/types";

import { medalChanges, niceTicks, streakRuns, withinDays } from "./rank-view";

const point = (tier: number, at: string): TrajectoryPoint => ({
  rank_tier: tier,
  label: null,
  at,
  estimated: false,
  gap_before: false,
});

describe("streakRuns", () => {
  it("finds runs of three or more, wins and losses alike", () => {
    const r = [true, true, true, false, true, false, false, false, false];
    expect(streakRuns(r)).toEqual([
      { start: 0, end: 2, won: true },
      { start: 5, end: 8, won: false },
    ]);
  });
  it("ignores shorter runs", () => {
    expect(streakRuns([true, true, false, false])).toEqual([]);
  });
});

describe("medalChanges", () => {
  it("marks where the medal changes, not the stars", () => {
    const pts = [point(44, "a"), point(45, "b"), point(51, "c"), point(45, "d")];
    expect(medalChanges(pts)).toEqual([2, 3]);
  });
});

describe("withinDays", () => {
  it("keeps only the window, or everything for null", () => {
    const now = new Date("2026-11-04T00:00:00Z");
    const pts = [point(44, "2026-07-01T00:00:00Z"), point(45, "2026-10-20T00:00:00Z")];
    expect(withinDays(pts, 30, now)).toHaveLength(1);
    expect(withinDays(pts, null, now)).toHaveLength(2);
  });
});

describe("niceTicks", () => {
  it("gives round steps that include zero", () => {
    const ticks = niceTicks(-60, 390);
    expect(ticks).toContain(0);
    expect(ticks.length).toBeGreaterThanOrEqual(4);
    expect(ticks.length).toBeLessThanOrEqual(6);
    expect(ticks[0]).toBeLessThanOrEqual(-60);
    expect(ticks[ticks.length - 1]).toBeGreaterThanOrEqual(390);
  });
});
