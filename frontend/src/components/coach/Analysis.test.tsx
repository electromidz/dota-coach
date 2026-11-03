import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { CoachResponse, Evidence, GoalResult, GoalView } from "@/lib/types";

import { Analysis, measuredEvidence } from "./Analysis";

// The progress panel fetches its own series; these tests are about the
// snapshot, so that read never settles.
vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getCoachingProgress: vi.fn(() => new Promise(() => {})) };
});

function peer(id: string, value: number, percentile: number, sample: number): Evidence {
  return {
    id,
    kind: "benchmark",
    label: "Tower damage vs peers",
    statement: `Statement for ${id}.`,
    sample,
    confidence: "low",
    figure: {
      metric: "tower_damage",
      hero_id: 105,
      value,
      higher_is_better: true,
      unit: null,
      median: 400,
      top_20: 600,
      percentile,
      previous: null,
      percent_change: null,
      status: null,
    },
  };
}

/** The analysis was written from 6 matches at the 46th; live says 7 at the 57th. */
const snapshot = [peer("benchmark.tower_damage", 380, 46, 6)];
const live = [peer("benchmark.tower_damage", 455, 57, 7)];

function response(overrides: Partial<CoachResponse> = {}): CoachResponse {
  return {
    role: "soft_support",
    role_label: "Soft Support",
    analysis: {
      id: "a1",
      scope: "role",
      match_id: null,
      model: "openai/gpt-oss-120b",
      summary: "Strong farm, weak structure damage.",
      insights: [
        {
          kind: "weakness",
          kind_label: "Weakness",
          title: "Below-average tower damage",
          explanation: "You hit buildings less than peers. Rotate to towers after a kill.",
          severity: null,
          timestamp: null,
          what_happened: null,
          why_it_matters: null,
          better_action: null,
          evidence: ["benchmark.tower_damage"],
        },
        {
          kind: "strength",
          kind_label: "Strength",
          title: "Strong economy",
          explanation: "Your farm is above peers.",
          severity: null,
          timestamp: null,
          what_happened: null,
          why_it_matters: null,
          better_action: null,
          evidence: [],
        },
      ],
      plan: [
        { position: 1, title: "Hit towers", action: "After each won fight, hit the nearest tower. Then reset.", evidence: ["benchmark.tower_damage"] },
      ],
      evidence: snapshot,
      generated_at: "2026-11-01T10:00:00Z",
    },
    evidence: live,
    llm_available: true,
    stale: true,
    cached: true,
    patterns: [],
    note: null,
    new_matches_since_analysis: 1,
    refreshing: false,
    goals: [],
    previous_goals: [],
    ...overrides,
  };
}

function renderAnalysis(data: CoachResponse, isAdmin = false) {
  return render(
    <Analysis
      data={data}
      onGenerate={vi.fn()}
      onReload={vi.fn(() => new Promise<CoachResponse>(() => {}))}
      generateLabel="Analyse"
      emptyHint="Nothing yet."
      isAdmin={isAdmin}
    />,
  );
}

describe("measuredEvidence", () => {
  it("uses the analysis snapshot, so the cards and the data agree", () => {
    expect(measuredEvidence(response())).toBe(snapshot);
  });

  it("falls back to live evidence when there is no analysis", () => {
    expect(measuredEvidence(response({ analysis: null }))).toBe(live);
  });
});

describe("Analysis", () => {
  it("shows the snapshot numbers everywhere, never the live ones", () => {
    renderAnalysis(response());

    // The card's bullet and the measured table both read 380 / 46th.
    expect(screen.getAllByText("380").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("46th").length).toBeGreaterThanOrEqual(2);
    expect(screen.queryByText("455")).toBeNull();
    expect(screen.queryByText("57th")).toBeNull();
  });

  it("keeps the measured data collapsed by default", () => {
    renderAnalysis(response());
    const details = screen.getByText("What the coach measured").closest("details");
    expect(details?.open).toBe(false);
    expect(screen.getByText("Show the data behind this")).toBeTruthy();
  });

  it("offers one refresh banner when newer matches exist", () => {
    renderAnalysis(response({ new_matches_since_analysis: 2 }));
    expect(screen.getByText(/new matches since this analysis/)).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Refresh" })).toHaveLength(1);
  });

  it("shows no banner when nothing has changed", () => {
    renderAnalysis(response({ new_matches_since_analysis: 0, stale: false }));
    expect(screen.queryByText(/since this analysis/)).toBeNull();
  });

  it("offers a refresh when the data changed without new matches", () => {
    renderAnalysis(response({ new_matches_since_analysis: 0, stale: true }));
    expect(screen.getByText("Data updated since this analysis")).toBeTruthy();
  });

  it("says it is updating while a background refresh runs", () => {
    renderAnalysis(response({ new_matches_since_analysis: 3, refreshing: true }));
    expect(screen.getByText(/Updating with 3 new matches/)).toBeTruthy();
  });

  it("puts the model name behind the (i), for admins only", () => {
    const { unmount } = renderAnalysis(response());
    expect(screen.queryByText(/gpt-oss/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "About this analysis" }));
    expect(screen.queryByText(/gpt-oss/)).toBeNull();
    unmount();

    renderAnalysis(response(), true);
    expect(screen.queryByText(/gpt-oss/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "About this analysis" }));
    expect(screen.getByText(/gpt-oss/)).toBeTruthy();
  });

  it("gives each card one sentence and keeps the rest in a tooltip", () => {
    renderAnalysis(response());
    const card = screen.getByRole("heading", { name: "Below-average tower damage" }).closest("article")!;
    expect(within(card).getByText("You hit buildings less than peers.")).toBeTruthy();
    expect(within(card).queryByText(/Rotate to towers/)).toBeNull();
  });

  it("puts two strengths, two weaknesses and the focus in the summary chips", () => {
    render(
      <Analysis
        data={response()}
        onGenerate={vi.fn()}
        generateLabel="Analyse"
        emptyHint="Nothing yet."
        focusTitle="Tower damage"
      />,
    );
    const summary = screen.getByRole("region", { name: "Summary" });
    expect(within(summary).getByText("Strong economy")).toBeTruthy();
    expect(within(summary).getByText("Below-average tower damage")).toBeTruthy();
    expect(within(summary).getByText("Focus: Tower damage")).toBeTruthy();
  });

  it("shows each goal's target, progress and last games", () => {
    const goal: GoalView = {
      position: 1,
      evidence_id: "benchmark.tower_damage",
      metric: "tower_damage",
      label: "Tower damage",
      hero_id: 105,
      higher_is_better: true,
      baseline: 380,
      target: 400,
      target_label: "Peer median",
      current: 380,
      progress: 0.95,
      recent: [false, true, true],
    };
    renderAnalysis(response({ goals: [goal] }));

    expect(screen.getByRole("meter", { name: /Tower damage: 380 of a peer median target of 400/ })).toBeTruthy();
    expect(screen.getByRole("img", { name: "Last 3 games: 2 hit the target" })).toBeTruthy();
  });

  it("scores the previous plan's goals at the top", () => {
    const previous: GoalResult[] = [
      { position: 1, evidence_id: "benchmark.tower_damage", metric: "tower_damage", label: "Tower damage", hero_id: 105, higher_is_better: true, baseline: 300, target: 400, target_label: "Peer median", current: 380, outcome: "in_progress" },
      { position: 2, evidence_id: "benchmark.gold_per_min", metric: "gold_per_min", label: "Gold per minute", hero_id: 105, higher_is_better: true, baseline: 380, target: 400, target_label: "Peer median", current: 420, outcome: "hit" },
    ];
    renderAnalysis(response({ previous_goals: previous }));

    const list = screen.getByRole("list", { name: "Last plan's goals" });
    expect(within(list).getByText("Hit:")).toBeTruthy();
    expect(within(list).getByText("In progress:")).toBeTruthy();
  });
});
