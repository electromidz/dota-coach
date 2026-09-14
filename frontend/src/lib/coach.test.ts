import { describe, expect, it } from "vitest";

import { citedBy, formatGeneratedAt, groupEvidence } from "./coach";
import type { Evidence, Insight } from "./types";

function evidence(id: string, kind: Evidence["kind"] = "overall"): Evidence {
  return {
    id,
    kind,
    label: "Record",
    statement: "Across 20 stored matches, you have won 11 and lost 9 (55%).",
    sample: 20,
    confidence: "adequate",
  };
}

function insight(refs: string[]): Insight {
  return {
    kind: "weakness",
    kind_label: "Weakness",
    title: "Your farm trails your peers",
    explanation: "Closing the gap is your biggest lever.",
    evidence: refs,
  };
}

describe("citedBy", () => {
  it("resolves citations to the backend's own statements", () => {
    const all = [evidence("overall.record"), evidence("benchmark.gold_per_min")];
    const cited = citedBy(insight(["benchmark.gold_per_min"]), all);

    expect(cited).toHaveLength(1);
    expect(cited[0].id).toBe("benchmark.gold_per_min");
  });

  it("drops a reference with nothing behind it rather than rendering it", () => {
    // The backend already rejects these, so reaching here means something
    // upstream changed; the UI must not show a dangling citation either way.
    const cited = citedBy(insight(["benchmark.wards"]), [evidence("overall.record")]);
    expect(cited).toEqual([]);
  });

  it("preserves the order the insight cited them in", () => {
    const all = [evidence("a"), evidence("b"), evidence("c")];
    const cited = citedBy(insight(["c", "a"]), all);

    expect(cited.map((e) => e.id)).toEqual(["c", "a"]);
  });
});

describe("groupEvidence", () => {
  it("groups by kind in reading order and drops empty groups", () => {
    const groups = groupEvidence([
      evidence("hero.35", "hero"),
      evidence("match.kda", "match"),
      evidence("overall.record", "overall"),
    ]);

    expect(groups.map((g) => g.label)).toEqual(["This match", "Career", "Heroes"]);
    expect(groups[0].items).toHaveLength(1);
  });

  it("handles an empty list", () => {
    expect(groupEvidence([])).toEqual([]);
  });
});

describe("formatGeneratedAt", () => {
  it("renders a timestamp", () => {
    expect(formatGeneratedAt("2026-09-14T10:00:00Z")).toMatch(/2026/);
  });

  it("does not throw on a value it cannot read", () => {
    expect(formatGeneratedAt("not a date")).toBe("unknown");
  });
});
