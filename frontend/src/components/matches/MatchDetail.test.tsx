import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { CoachResponse, MatchComparisonResponse, MatchView } from "@/lib/types";

import { MatchDetail } from "./MatchDetail";

const getMatch = vi.hoisted(() => vi.fn());
const getMatchAnalysis = vi.hoisted(() => vi.fn());
const getMatchComparison = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getMatch, getMatchAnalysis, getMatchComparison };
});

function match(overrides: Partial<MatchView> = {}): MatchView {
  return {
    id: "3fa85f64-5717-4562-b3fc-2c963f66afa6",
    dota_player_id: "player",
    match_id: 7_500_000_001,
    hero_id: 35,
    hero_name: "Luna",
    role: "Carry",
    lane_role: 1,
    won: true,
    duration_seconds: 2_400,
    kills: 8,
    deaths: 4,
    assists: 12,
    gpm: 550,
    xpm: 620,
    last_hits: 300,
    denies: 6,
    net_worth: 18_000,
    hero_damage: 22_000,
    tower_damage: 3_400,
    hero_healing: 0,
    game_mode: 22,
    lobby_type: 7,
    party_size: 1,
    started_at: new Date().toISOString(),
    detail_synced: true,
    replay_parsed: false,
    kda: 5,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    eligible: true,
    mode_label: "Ranked All Pick",
    ...overrides,
  };
}

const ANALYSIS: CoachResponse = {
  role: null,
  role_label: null,
  analysis: null,
  evidence: [],
  llm_available: false,
  stale: false,
  cached: false,
  patterns: [],
  note: null,
};

const COMPARISON: MatchComparisonResponse = {
  hero_id: 35,
  hero_name: "Luna",
  bracket: {
    requested: "legend",
    used: "legend",
    label: "Legend",
    fell_back: false,
  },
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
      match_id: "3fa85f64-5717-4562-b3fc-2c963f66afa6",
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
    text: "Deaths per minute is where this game sat lowest against Legend players on Luna (p18). Matching their median over 40 minutes is about 8 fewer deaths.",
  },
  context: {
    hero_id: 35,
    hero_name: "Luna",
    role: null,
    role_label: null,
    rank_tier: 54,
    bracket: {
      requested: "legend",
      used: "legend",
      label: "Legend",
      fell_back: false,
    },
    requested: ["hero", "role", "rank_bracket", "patch"],
    segmented_by: ["hero", "rank_bracket"],
    unavailable: [
      { segment: "role", label: "Role", reason: "No role segmentation." },
      { segment: "patch", label: "Patch", reason: "No patch stated." },
    ],
    population: {
      player: "12 matches on Luna",
      peers: "Public matches on Luna in the Legend bracket.",
      comparable: false,
      note: "",
    },
  },
  note: null,
};

afterEach(() => {
  getMatch.mockReset();
  getMatchAnalysis.mockReset();
  getMatchComparison.mockReset();
});

describe("MatchDetail", () => {
  it("renders the combat, impact and comparison sections alongside the existing result", async () => {
    getMatch.mockResolvedValue({ match: match() });
    getMatchAnalysis.mockResolvedValue(ANALYSIS);
    getMatchComparison.mockResolvedValue(COMPARISON);

    render(<MatchDetail id="3fa85f64-5717-4562-b3fc-2c963f66afa6" />);

    // The existing result panel is untouched.
    expect(await screen.findByText("Luna")).toBeDefined();
    expect(screen.getByText("8/4/12")).toBeDefined();

    // The charts are attached, not swapped in.
    expect(screen.getByText("Combat")).toBeDefined();
    expect(screen.getByText("Impact")).toBeDefined();
    expect(await screen.findByText("Against your rank")).toBeDefined();
    // It appears both as a metric row and as a named strength, which is the
    // point — so assert on presence, not on a single occurrence.
    expect((await screen.findAllByText(/Gold per minute/)).length).toBeGreaterThan(0);
  });

  it("skips the impact chart when detail was never synced", async () => {
    getMatch.mockResolvedValue({
      match: match({
        detail_synced: false,
        hero_damage: null,
        tower_damage: null,
        hero_healing: null,
      }),
    });
    getMatchAnalysis.mockResolvedValue(ANALYSIS);
    getMatchComparison.mockResolvedValue(COMPARISON);

    render(<MatchDetail id="3fa85f64-5717-4562-b3fc-2c963f66afa6" />);

    expect(await screen.findByText("Combat")).toBeDefined();
    expect(screen.queryByText("Impact")).toBeNull();
  });
});
