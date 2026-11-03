import { describe, expect, it } from "vitest";

import type { BenchmarkResult, TargetComparison } from "@/lib/types";

import {
  averagePercentile,
  extremes,
  gapToTarget,
  toPercentileVector,
} from "./benchmark-view";

function result(overrides: Partial<BenchmarkResult>): BenchmarkResult {
  return {
    metric: "gold_per_min",
    label: "Gold per minute",
    higher_is_better: true,
    player_value: 500,
    player_sample: 20,
    peer_median: 480,
    top_20_value: 560,
    percentile: 60,
    gap_to_top_20: 60,
    peer_sample_size: null,
    confidence: "adequate",
    segmented_by: [],
    note: null,
    ...overrides,
  };
}

const deaths = result({
  metric: "deaths_per_min",
  label: "Deaths per minute",
  higher_is_better: false,
  player_value: 0.15,
  peer_median: 0.2,
  percentile: 80,
});

const target: TargetComparison = {
  bracket: { requested: "ancient", used: "ancient", label: "Ancient", fell_back: false },
  label: "Ancient",
  metrics: [
    {
      metric: "gold_per_min",
      label: "Gold per minute",
      higher_is_better: true,
      peer_median: 550,
      top_20_value: 620,
      percentile: 40,
      gap_to_median: 50,
      cleared: false,
      median_percentile: 72,
    },
    {
      metric: "deaths_per_min",
      label: "Deaths per minute",
      higher_is_better: false,
      peer_median: 0.18,
      top_20_value: 0.14,
      percentile: 65,
      gap_to_median: -0.03,
      cleared: true,
      median_percentile: 58,
    },
  ],
  metrics_cleared: 1,
  metrics_compared: 2,
};

describe("toPercentileVector", () => {
  it("passes server percentiles through without inverting deaths", () => {
    const [gpm, d] = toPercentileVector([result({}), deaths], target);
    expect(gpm).toMatchObject({ axis: "GPM", you: 60, rankMedian: 50, targetMedian: 72 });
    expect(d).toMatchObject({ axis: "Deaths/min", you: 80, rankMedian: 50, targetMedian: 58 });
    expect(d.raw).toEqual({ you: 0.15, rankMedian: 0.2, targetMedian: 0.18 });
  });

  it("leaves the target series empty without a target", () => {
    const [gpm] = toPercentileVector([result({})], null);
    expect(gpm.targetMedian).toBeNull();
    expect(gpm.raw.targetMedian).toBeNull();
  });

  it("does not invent a rank median the provider did not publish", () => {
    const [gpm] = toPercentileVector([result({ peer_median: null })], null);
    expect(gpm.rankMedian).toBeNull();
  });
});

describe("gapToTarget", () => {
  it("makes positive mean ahead for every metric and sorts worst first", () => {
    const rows = gapToTarget([result({}), deaths], target);
    expect(rows.map((r) => r.metric)).toEqual(["gold_per_min", "deaths_per_min"]);
    expect(rows[0].percent).toBeCloseTo((-50 / 550) * 100);
    expect(rows[1].percent).toBeCloseTo((0.03 / 0.18) * 100);
  });

  it("is empty with no target", () => {
    expect(gapToTarget([result({})], null)).toEqual([]);
  });
});

describe("summaries", () => {
  it("finds the strongest and weakest ranked metric", () => {
    const points = toPercentileVector(
      [result({}), deaths, result({ metric: "xp_per_min", percentile: null })],
      null,
    );
    const found = extremes(points);
    expect(found?.strongest.metric).toBe("deaths_per_min");
    expect(found?.weakest.metric).toBe("gold_per_min");
  });

  it("averages only ranked percentiles", () => {
    expect(averagePercentile([result({}), deaths, result({ percentile: null })])).toBe(70);
    expect(averagePercentile([result({ percentile: null })])).toBeNull();
  });
});
