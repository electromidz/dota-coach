import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { MetricProgress, ProgressResponse } from "@/lib/types";

import { ProgressSummary } from "./ProgressSummary";

const getCoachingProgress = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getCoachingProgress };
});

function metric(overrides: Partial<MetricProgress> = {}): MetricProgress {
  return {
    key: "overall.deaths",
    label: "Deaths per 10 minutes",
    unit: "per10",
    higher_is_better: false,
    previous: 7.2,
    current: 5.8,
    delta: -1.4,
    direction_delta: 1.4,
    percent_change: -0.19,
    previous_sample: 20,
    current_sample: 20,
    status: "improved",
    status_label: "Improved",
    note: null,
    ...overrides,
  };
}

function progress(overrides: Partial<ProgressResponse> = {}): ProgressResponse {
  return {
    role: "carry",
    role_label: "Carry",
    comparison: {
      role: "carry",
      role_label: "Carry",
      previous_session_id: "prev",
      previous_sequence: 1,
      previous_at: "2026-01-01T12:00:00Z",
      current_session_id: "cur",
      current_sequence: 2,
      current_at: "2026-01-08T12:00:00Z",
      performance: metric({
        key: "role.performance",
        label: "Role performance",
        unit: "score",
        higher_is_better: true,
        previous: 54,
        current: 61,
        delta: 7,
        direction_delta: 7,
        percent_change: null,
      }),
      metrics: [metric()],
      headline: "Deaths per 10 minutes improved by 19%.",
    },
    series: [
      {
        key: "role.performance",
        label: "Role performance",
        unit: "score",
        higher_is_better: true,
        points: [
          { session_id: "a", sequence: 1, at: "2026-01-01T12:00:00Z", value: 54 },
          { session_id: "b", sequence: 2, at: "2026-01-08T12:00:00Z", value: 61 },
        ],
      },
    ],
    sessions: 2,
    note: null,
    ...overrides,
  };
}

afterEach(() => getCoachingProgress.mockReset());

describe("ProgressSummary", () => {
  it("shows the score then and now, and each change as before -> after", async () => {
    getCoachingProgress.mockResolvedValue(progress());

    render(<ProgressSummary />);

    // 54 → 61, both rendered, neither recomputed here.
    expect((await screen.findAllByText("54")).length).toBeGreaterThan(0);
    expect(screen.getAllByText("61").length).toBeGreaterThan(0);
    expect(screen.getByText("Deaths per 10 minutes")).toBeTruthy();
    expect(screen.getByText("7.20")).toBeTruthy();
    expect(screen.getByText("5.80")).toBeTruthy();
    // No sentences: the headline is not printed.
    expect(screen.queryByText("Deaths per 10 minutes improved by 19%.")).toBeNull();
  });

  it("renders the verdict in words, never colour alone", async () => {
    getCoachingProgress.mockResolvedValue(progress());

    render(<ProgressSummary />);

    // Fewer deaths is an improvement: the word is there for screen readers,
    // whatever the arrow points at.
    expect((await screen.findAllByText(/Improved/)).length).toBeGreaterThan(0);
  });

  it("says there is nothing to compare rather than implying no change", async () => {
    getCoachingProgress.mockResolvedValue(
      progress({ comparison: null, sessions: 1, note: null, series: [] }),
    );

    render(<ProgressSummary />);

    expect(await screen.findByText(/nothing to compare yet/)).toBeDefined();
    expect(screen.queryByText(/Improved|unchanged/)).toBeNull();
  });

  it("reads from the analysis snapshot when given one, with one match count", async () => {
    getCoachingProgress.mockResolvedValue(progress());
    const snapshot = [
      { id: "progress.session", kind: "progress" as const, label: "Since your last session", statement: "", sample: 0, confidence: "insufficient" as const },
      {
        id: "progress.performance",
        kind: "progress" as const,
        label: "Performance since your last session",
        statement: "",
        sample: 8,
        confidence: "low" as const,
        figure: { metric: null, hero_id: null, value: 58.4, higher_is_better: true, unit: "score" as const, median: null, top_20: null, percentile: null, previous: 56, percent_change: null, status: "improved" as const },
      },
      {
        id: "progress.overall.xp_per_min",
        kind: "progress" as const,
        label: "XP per minute since your last session",
        statement: "",
        sample: 8,
        confidence: "low" as const,
        figure: { metric: null, hero_id: null, value: 702, higher_is_better: true, unit: "per_minute" as const, median: null, top_20: null, percentile: null, previous: 636, percent_change: 0.104, status: "improved" as const },
      },
      {
        id: "progress.benchmark.assists_per_min",
        kind: "progress" as const,
        label: "Assists per minute since your last session",
        statement: "",
        sample: 8,
        confidence: "low" as const,
        figure: { metric: null, hero_id: null, value: 59, higher_is_better: true, unit: "percentile" as const, median: null, top_20: null, percentile: null, previous: 48, percent_change: null, status: "improved" as const },
      },
    ];

    render(<ProgressSummary snapshot={snapshot} />);

    // The snapshot wins over the live comparison (54 → 61).
    expect(await screen.findByText("XP per minute")).toBeTruthy();
    expect(screen.getByText("636")).toBeTruthy();
    expect(screen.getByText("702")).toBeTruthy();
    expect(screen.getByText(/\+10%/)).toBeTruthy();
    // 56.0 → 58.4 reads as improved, not unchanged.
    expect(screen.getByText(/▲ \+2/)).toBeTruthy();
    // A percentile moving 48th -> 59th is eleven points, not "+11th".
    expect(screen.getByText(/\+11 pts/)).toBeTruthy();
    expect(screen.queryByText(/11th/)).toBeNull();
    // Role-wide rows say so, apart from the per-hero peer comparison.
    expect(screen.getByText("All Carry games")).toBeTruthy();
    // One count for the section, never 0 beside 8.
    expect(screen.getByText("8 matches")).toBeTruthy();
    expect(screen.queryByText(/^0 matches/)).toBeNull();
  });

  it("renders nothing at all before a first session exists", async () => {
    getCoachingProgress.mockResolvedValue(
      progress({ comparison: null, sessions: 0, series: [], note: "No coaching sessions yet." }),
    );

    const { container } = render(<ProgressSummary />);

    // The coach page has plenty to say before a player has any history; an
    // empty card would be noise.
    await vi.waitFor(() => expect(container.textContent).toBe(""));
  });

  it("stays silent when no role has been chosen", async () => {
    const error = Object.assign(new Error("Choose a role"), { status: 409 });
    getCoachingProgress.mockRejectedValue(error);

    const { container } = render(<ProgressSummary />);

    // The coach page already prompts for a role; a second prompt here would
    // be the same message twice.
    await vi.waitFor(() => expect(container.querySelector("[role=alert]")).toBeNull());
  });
});
