import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { MatchComparisonResponse } from "@/lib/types";

import { MatchComparison } from "./MatchComparison";

const ID = "3fa85f64-5717-4562-b3fc-2c963f66afa6";

function comparison(
  overrides: Partial<MatchComparisonResponse> = {},
): MatchComparisonResponse {
  const bracket = {
    requested: "legend" as const,
    used: "legend" as const,
    label: "Legend",
    fell_back: false,
  };

  return {
    hero_id: 35,
    hero_name: "Luna",
    bracket,
    comparable: true,
    standing: {
      this_match: 72,
      hero_average: 54,
      metrics_counted: 2,
      peer_sample_size: null,
    },
    metrics: [
      {
        metric: "gold_per_min",
        label: "Gold per minute",
        higher_is_better: true,
        this_match: { value: 550, percentile: 78 },
        hero_average: {
          value: 498,
          percentile: 52,
          sample: 12,
          confidence: "adequate",
        },
        peer_median: 480,
        top_20_value: 620,
      },
      {
        metric: "deaths_per_min",
        label: "Deaths per minute",
        higher_is_better: false,
        this_match: { value: 0.32, percentile: 18 },
        hero_average: {
          value: 0.24,
          percentile: 44,
          sample: 12,
          confidence: "adequate",
        },
        peer_median: 0.2,
        top_20_value: 0.12,
      },
    ],
    trend: [
      {
        match_id: ID,
        dota_match_id: 7_500_000_001,
        started_at: "2026-01-03T12:00:00Z",
        won: true,
        standing: 72,
        is_current: true,
      },
      {
        match_id: "older",
        dota_match_id: 7_500_000_000,
        started_at: "2026-01-02T12:00:00Z",
        won: false,
        standing: 58,
        is_current: false,
      },
    ],
    delta_vs_previous: 14,
    pros: [
      {
        metric: "gold_per_min",
        label: "Gold per minute",
        value: 550,
        percentile: 78,
        detail: "550 against a Legend median of 480.",
      },
    ],
    cons: [
      {
        metric: "deaths_per_min",
        label: "Deaths per minute",
        value: 0.32,
        percentile: 18,
        detail: "0.32 against a Legend median of 0.20.",
      },
    ],
    suggestion: {
      metric: "deaths_per_min",
      label: "Deaths per minute",
      percentile: 18,
      player_value: 0.32,
      peer_median: 0.2,
      whole_game_delta: 8,
      whole_game_unit: "deaths",
      text: "Deaths per minute is where this game sat lowest against Legend players on Luna (p18).",
    },
    context: {
      hero_id: 35,
      hero_name: "Luna",
      role: null,
      role_label: null,
      rank_tier: 54,
      bracket,
      requested: ["hero", "role", "rank_bracket", "patch"],
      segmented_by: ["hero", "rank_bracket"],
      unavailable: [
        { segment: "role", label: "Role", reason: "No role segmentation." },
      ],
      population: {
        player: "12 matches on Luna",
        peers: "Public matches on Luna in the Legend bracket.",
        comparable: false,
        note: "",
      },
    },
    item_timings: [],
    laning: [],
    note: null,
    ...overrides,
  };
}

function show(data: MatchComparisonResponse) {
  return render(<MatchComparison data={data} durationSeconds={2400} />);
}

const KILLS = {
  metric: "kills_per_min",
  label: "Kills per minute",
  higher_is_better: true,
  this_match: { value: 0.3, percentile: 60 },
  hero_average: { value: 0.25, percentile: 48, sample: 12, confidence: "adequate" as const },
  peer_median: 0.26,
  top_20_value: 0.4,
};

describe("MatchComparison", () => {
  it("puts every metric on one scale beside a radar of the same readings", () => {
    const base = comparison();
    show(comparison({ metrics: [...base.metrics, KILLS] }));

    expect(screen.getByText("Every metric, same scale")).toBeDefined();
    expect(screen.getByText("Shape of the game")).toBeDefined();
    expect(screen.getByText("Your Luna average")).toBeDefined();
    expect(screen.getByText(/\+14 vs your last game/)).toBeDefined();
    // The model's prose lists are gone from this section; findings carry them.
    expect(screen.queryByText("Went well")).toBeNull();
  });

  it("shows a Turbo game's figures but never a percentile for it", () => {
    show(
      comparison({
        comparable: false,
        standing: { this_match: null, hero_average: 54, metrics_counted: 2, peer_sample_size: null },
        metrics: [
          {
            metric: "gold_per_min",
            label: "Gold per minute",
            higher_is_better: true,
            this_match: { value: 550, percentile: null },
            hero_average: { value: 498, percentile: 52, sample: 12, confidence: "adequate" },
            peer_median: 480,
            top_20_value: 620,
          },
        ],
        note: "This was a Turbo game.",
      }),
    );

    expect(screen.getByText("This was a Turbo game.")).toBeDefined();
    expect(screen.getByText("550")).toBeDefined();
    expect(screen.getByText("not compared")).toBeDefined();
    expect(screen.getByText(/This game: not compared/)).toBeDefined();
    // No radar without three placed axes.
    expect(screen.queryByText("Shape of the game")).toBeNull();
  });

  it("surfaces a provider outage as a note rather than an empty panel", () => {
    show(comparison({ trend: [], note: "Peer comparison is unavailable right now." }));
    expect(screen.getByText("Peer comparison is unavailable right now.")).toBeDefined();
    expect(screen.getByText("Gold per minute")).toBeDefined();
  });

  it("says so when there is no earlier game to compare against", () => {
    show(comparison({ delta_vs_previous: null }));
    expect(screen.getByText("first game")).toBeDefined();
  });

  it("draws item timings against the peer median and laning against your average", () => {
    show(
      comparison({
        item_timings: [
          {
            item: "black_king_bar",
            label: "Black King Bar",
            seconds: 3122,
            peer_median_seconds: 1350,
            peer_games: 29,
            verdict: "late",
          },
          {
            item: "blink_dagger",
            label: "Blink Dagger",
            seconds: 900,
            peer_median_seconds: null,
            peer_games: 0,
            verdict: null,
          },
        ],
        laning: [
          { metric: "last_hits_at_10", label: "Last hits @10", value: 42, baseline: 38.4, baseline_sample: 6 },
          { metric: "gold_at_10", label: "Gold @10", value: 3900, baseline: null, baseline_sample: 1 },
        ],
      }),
    );

    expect(screen.getByText("52:02")).toBeDefined();
    expect(screen.getByText("late · p50 22:30")).toBeDefined();
    expect(
      screen.getByRole("img", { name: /Black King Bar at 52:02, peer median on Luna 22:30, late/ }),
    ).toBeDefined();
    expect(screen.getAllByText("not compared").length).toBeGreaterThan(0);
    expect(screen.getByText("avg 38")).toBeDefined();
    expect(screen.getByText("no avg")).toBeDefined();
  });
});
