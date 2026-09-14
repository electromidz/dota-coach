import { describe, expect, it } from "vitest";

import { formatFocusValue, trendDirection } from "./training";
import type { ProgressSeries } from "./types";

function series(
  values: number[],
  higher_is_better: boolean,
): ProgressSeries {
  return {
    measure: higher_is_better ? "gold_per_min" : "deaths_per_10",
    label: "Measure",
    higher_is_better,
    points: values.map((value, index) => ({
      matches: 10,
      value,
      at: new Date(1_700_000_000_000 + index * 3_600_000).toISOString(),
    })),
    window: 10,
    target_value: null,
  };
}

describe("formatFocusValue", () => {
  it("renders rates as percentages and counts as counts", () => {
    expect(formatFocusValue("pattern_rate", 0.42)).toBe("42%");
    expect(formatFocusValue("kill_participation", 0.6)).toBe("60%");
    expect(formatFocusValue("deaths_per_10", 2.44)).toBe("2.4");
    expect(formatFocusValue("gold_per_min", 512.6)).toBe("513");
  });

  it("renders an em dash when there is nothing measured", () => {
    expect(formatFocusValue("deaths_per_10", null)).toBe("—");
  });
});

describe("trendDirection", () => {
  it("honours direction, so falling deaths and rising gold both improve", () => {
    expect(trendDirection(series([4, 3, 2], false))).toBe("improving");
    expect(trendDirection(series([400, 500, 600], true))).toBe("improving");
  });

  it("reports the wrong way as worsening", () => {
    expect(trendDirection(series([2, 3, 4], false))).toBe("worsening");
    expect(trendDirection(series([600, 500, 400], true))).toBe("worsening");
  });

  it("treats a hair either way as flat rather than a trend", () => {
    expect(trendDirection(series([3.0, 3.01], false))).toBe("flat");
  });

  it("has nothing to say about a single point or no series", () => {
    expect(trendDirection(series([3], false))).toBeNull();
    expect(trendDirection(null)).toBeNull();
  });
});
