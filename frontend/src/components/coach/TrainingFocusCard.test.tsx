import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { PreliminaryFocus, TrainingFocusResponse } from "@/lib/types";

import { TrainingFocusCard } from "./TrainingFocusCard";

const getTrainingFocus = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getTrainingFocus };
});

function preliminary(
  overrides: Partial<PreliminaryFocus> = {},
): PreliminaryFocus {
  return {
    metric: "last_hits_per_min",
    label: "Last hits per minute",
    higher_is_better: true,
    player_value: 4.1,
    player_sample: 11,
    peer_median: 5.6,
    percentile: 27,
    confidence: "low",
    why: "Of everything measured on your most-played hero, last hits per minute is where you place lowest against your peers — the 27th percentile.",
    to_confirm:
      "This rests on 11 matches. At 15 it becomes a firm reading rather than an early one.",
    ...overrides,
  };
}

function response(
  overrides: Partial<TrainingFocusResponse> = {},
): TrainingFocusResponse {
  return {
    focus: null,
    preliminary: null,
    progress: null,
    next_up: [],
    history: [],
    note: null,
    ...overrides,
  };
}

afterEach(() => {
  vi.clearAllMocks();
});

describe("TrainingFocusCard", () => {
  it("offers the weakest measured metric when no focus clears the bar", async () => {
    getTrainingFocus.mockResolvedValue(
      response({ preliminary: preliminary() }),
    );

    render(<TrainingFocusCard />);

    expect(await screen.findByText("Potential training focus")).toBeTruthy();
    expect(screen.getByText("Last hits per minute")).toBeTruthy();
    expect(screen.getByText("p27")).toBeTruthy();
  });

  it("labels the reading as preliminary rather than as a conclusion", async () => {
    getTrainingFocus.mockResolvedValue(
      response({ preliminary: preliminary() }),
    );

    render(<TrainingFocusCard />);

    // The heading must not be the one a real focus uses, and the caveat has to
    // be on the card rather than implied by its absence.
    expect(await screen.findByText("Potential training focus")).toBeTruthy();
    expect(screen.queryByText("Current training focus")).toBeNull();
    expect(screen.getByText("Low confidence")).toBeTruthy();
    expect(screen.getByText(/early signal, not a reliable conclusion/)).toBeTruthy();
    expect(screen.getByText(/rests on 11 matches/)).toBeTruthy();
    // No goal was set, so nothing may claim progress toward one.
    expect(screen.queryByText("Progress toward the target")).toBeNull();
    expect(screen.queryByRole("meter")).toBeNull();
  });

  it("shows no percentile when the backend withheld one", async () => {
    getTrainingFocus.mockResolvedValue(
      response({
        preliminary: preliminary({
          percentile: null,
          confidence: "insufficient",
          player_sample: 4,
          why: "Your last hits per minute sits at 4.10 against a peer median of 5.60 on your most-played hero. That is a comparison of averages, not a ranking — there are too few matches to place you in the distribution.",
          to_confirm:
            "5 matches on this hero are needed before you can be placed in the distribution at all; you have 4.",
        }),
      }),
    );

    render(<TrainingFocusCard />);

    expect(await screen.findByText("unranked")).toBeTruthy();
    expect(screen.queryByText(/^p\d+$/)).toBeNull();
    expect(screen.getByText("Not enough data")).toBeTruthy();
    expect(screen.getByText(/not a ranking/)).toBeTruthy();
  });

  it("falls back to the note when nothing at all was measurable", async () => {
    getTrainingFocus.mockResolvedValue(
      response({ note: "Nothing stands out as a training focus right now." }),
    );

    render(<TrainingFocusCard />);

    expect(
      await screen.findByText(/Nothing stands out as a training focus/),
    ).toBeTruthy();
    expect(screen.queryByText("Potential training focus")).toBeNull();
  });

  it("leaves a real focus untouched", async () => {
    getTrainingFocus.mockResolvedValue(
      response({
        focus: {
          id: "focus-1",
          key: "benchmark.gold_per_min",
          title: "Improve your gold per minute",
          why: "You sit at the 20th percentile for gold per minute on your most-played hero.",
          source: "benchmark",
          measure: "gold_per_min",
          measure_label: "Gold per minute",
          pattern_id: null,
          higher_is_better: true,
          baseline_value: 420,
          target_value: 540,
          current_value: 480,
          progress: 0.5,
          target_met: false,
          status: "active",
          status_label: "Active",
          score: 62,
          score_parts: [],
          confidence: "adequate",
          sample: 20,
          started_at: "2026-01-01T12:00:00Z",
          ended_at: null,
        },
        // A preliminary reading must never be rendered beside a real focus;
        // the backend only sets one, and the card honours that order.
        preliminary: preliminary(),
      }),
    );

    render(<TrainingFocusCard />);

    expect(await screen.findByText("Current training focus")).toBeTruthy();
    await waitFor(() =>
      expect(screen.queryByText("Potential training focus")).toBeNull(),
    );
  });
});
