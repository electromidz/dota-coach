import { describe, expect, it } from "vitest";

import type { BenchmarkResult, TrainingFocus } from "@/lib/types";

import { impactOf, skillPoints } from "./overview";

function result(metric: string, percentile: number | null, higher = true): BenchmarkResult {
  return {
    metric,
    label: metric,
    higher_is_better: higher,
    player_value: 1,
    player_sample: 20,
    peer_median: 1,
    top_20_value: 2,
    percentile,
    gap_to_top_20: null,
    peer_sample_size: 500,
    confidence: "adequate",
    segmented_by: [],
    note: null,
  };
}

describe("skillPoints", () => {
  it("names axes by skill, in a fixed order, and keeps the server's percentile", () => {
    const points = skillPoints([
      result("deaths_per_min", 72, false),
      result("gold_per_min", 40),
      result("xp_per_min", 90),
      result("assists_per_min", 55),
    ]);

    expect(points.map((p) => p.axis)).toEqual(["Farming", "Supporting", "Survival"]);
    expect(points.find((p) => p.axis === "Survival")?.you).toBe(72);
  });

  it("drops an axis the server could not place", () => {
    expect(skillPoints([result("tower_damage", null)])).toEqual([]);
  });
});

describe("impactOf", () => {
  const focus = (score: number) =>
    ({ score_parts: [{ key: "impact", label: "Impact", score, weight: 0.2, detail: "" }] }) as unknown as TrainingFocus;

  it("bands the engine's impact score", () => {
    expect(impactOf(focus(80))).toBe("high");
    expect(impactOf(focus(50))).toBe("medium");
    expect(impactOf(focus(10))).toBe("low");
  });

  it("has no badge without an impact part", () => {
    expect(impactOf({ score_parts: [] } as unknown as TrainingFocus)).toBeNull();
  });
});
