import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { MatchComparisonResponse } from "@/lib/types";

import { MatchComparison } from "./MatchComparison";

const getMatchComparison = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getMatchComparison };
});

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
    note: null,
    ...overrides,
  };
}

afterEach(() => getMatchComparison.mockReset());

describe("MatchComparison", () => {
  it("leads with where this game sits against the player's own bracket", async () => {
    getMatchComparison.mockResolvedValue(comparison());

    render(<MatchComparison id={ID} />);

    // The headline names the bracket rather than "peers": the whole point of
    // the rank segmentation is that the reader knows who they are measured
    // against.
    expect(await screen.findByText("72%")).toBeDefined();
    expect(screen.getByText(/of Legend players on/)).toBeDefined();
    expect(screen.getByText(/\+14 vs your last game/)).toBeDefined();
  });

  it("names strengths, weaknesses and one thing to work on", async () => {
    getMatchComparison.mockResolvedValue(comparison());

    render(<MatchComparison id={ID} />);

    expect(await screen.findByText("Went well")).toBeDefined();
    expect(screen.getByText("Held you back")).toBeDefined();
    expect(screen.getByText("Work on this")).toBeDefined();
    expect(screen.getByText(/sat lowest against Legend players/)).toBeDefined();
  });

  it("shows a Turbo game's figures but never a percentile for it", async () => {
    // The rule the backend enforces, asserted at the surface a user reads:
    // values present, percentiles withheld, and the reason stated.
    getMatchComparison.mockResolvedValue(
      comparison({
        comparable: false,
        standing: {
          this_match: null,
          hero_average: 54,
          metrics_counted: 2,
          peer_sample_size: null,
        },
        metrics: [
          {
            metric: "gold_per_min",
            label: "Gold per minute",
            higher_is_better: true,
            this_match: { value: 550, percentile: null },
            hero_average: {
              value: 498,
              percentile: 52,
              sample: 12,
              confidence: "adequate",
            },
            peer_median: 480,
            top_20_value: 620,
          },
        ],
        pros: [],
        cons: [],
        suggestion: null,
        note: "This was a Turbo game.",
      }),
    );

    render(<MatchComparison id={ID} />);

    expect(await screen.findByText("This was a Turbo game.")).toBeDefined();
    expect(screen.getByText("550")).toBeDefined();
    expect(screen.getByText("not ranked")).toBeDefined();
    expect(screen.queryByText("Work on this")).toBeNull();
    expect(screen.queryByText("72%")).toBeNull();
  });

  it("surfaces a provider outage as a note rather than an empty panel", async () => {
    getMatchComparison.mockResolvedValue(
      comparison({
        comparable: false,
        standing: {
          this_match: null,
          hero_average: null,
          metrics_counted: 0,
          peer_sample_size: null,
        },
        trend: [],
        pros: [],
        cons: [],
        suggestion: null,
        note: "Peer comparison is unavailable right now.",
      }),
    );

    render(<MatchComparison id={ID} />);

    expect(
      await screen.findByText("Peer comparison is unavailable right now."),
    ).toBeDefined();
    // The metric rows still render the player's own figures.
    expect(screen.getByText("Gold per minute")).toBeDefined();
  });

  it("says so when there is no earlier game to compare against", async () => {
    getMatchComparison.mockResolvedValue(
      comparison({ delta_vs_previous: null }),
    );

    render(<MatchComparison id={ID} />);

    expect(
      await screen.findByText("no earlier game to compare"),
    ).toBeDefined();
  });
});
