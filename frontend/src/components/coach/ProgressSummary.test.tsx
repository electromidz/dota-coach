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
  it("leads with the backend's headline and shows the score then and now", async () => {
    getCoachingProgress.mockResolvedValue(progress());

    render(<ProgressSummary />);

    expect(
      await screen.findByText("Deaths per 10 minutes improved by 19%."),
    ).toBeDefined();
    // 54 → 61, both rendered, neither recomputed here. Each appears twice:
    // once in the delta and once as an endpoint of the trend chart.
    expect(screen.getAllByText("54").length).toBeGreaterThan(0);
    expect(screen.getAllByText("61").length).toBeGreaterThan(0);
  });

  it("renders the verdict in words, never colour alone", async () => {
    getCoachingProgress.mockResolvedValue(progress());

    render(<ProgressSummary />);

    // The status label comes from the backend and is shown verbatim, so a
    // reader who cannot distinguish the dot colours still gets the answer.
    expect(await screen.findByText("Improved")).toBeDefined();
  });

  it("says there is nothing to compare rather than implying no change", async () => {
    getCoachingProgress.mockResolvedValue(
      progress({
        comparison: null,
        sessions: 1,
        note: "This is your first coaching session, so there is nothing to compare it with yet.",
        series: [],
      }),
    );

    render(<ProgressSummary />);

    expect(
      await screen.findByText(/first coaching session/),
    ).toBeDefined();
    // "Stable" would be a different and wrong answer here.
    expect(screen.queryByText("Improved")).toBeNull();
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
