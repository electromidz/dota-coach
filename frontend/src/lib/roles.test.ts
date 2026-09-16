import { describe, expect, it } from "vitest";

import {
  formatComponentValue,
  formatWeight,
  gamesUntilRecommendable,
} from "./roles";
import type { ScoreComponent } from "./types";

function component(
  key: ScoreComponent["key"],
  value: number,
): ScoreComponent {
  return { key, label: key, value, normalized: 0, weight: 0.25, sample: 20 };
}

describe("formatComponentValue", () => {
  it("writes rates as percentages and counts in their own units", () => {
    expect(formatComponentValue(component("win_rate", 0.545))).toBe("55%");
    expect(formatComponentValue(component("kill_participation", 0.62))).toBe(
      "62%",
    );
    expect(formatComponentValue(component("kda", 3.456))).toBe("3.46");
    expect(formatComponentValue(component("deaths_per_10", 2.14))).toBe("2.1");
  });
});

describe("formatWeight", () => {
  it("renders the share of the score a measure carries", () => {
    expect(formatWeight(0.45)).toBe("45%");
    // Renormalised weights are rarely round; they still read as whole percents.
    expect(formatWeight(0.5625)).toBe("56%");
  });
});

describe("gamesUntilRecommendable", () => {
  it("counts down to the floor and then says nothing", () => {
    expect(gamesUntilRecommendable(7, 10)).toBe(3);
    expect(gamesUntilRecommendable(10, 10)).toBe(0);
    expect(gamesUntilRecommendable(40, 10)).toBe(0);
  });
});
