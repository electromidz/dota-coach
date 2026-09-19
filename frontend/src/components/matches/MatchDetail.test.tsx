import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { BenchmarkResponse, CoachResponse, MatchView } from "@/lib/types";

import { MatchDetail } from "./MatchDetail";

const getMatch = vi.hoisted(() => vi.fn());
const getMatchAnalysis = vi.hoisted(() => vi.fn());
const getBenchmark = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getMatch, getMatchAnalysis, getBenchmark };
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

const BENCHMARK: BenchmarkResponse = {
  hero_id: 35,
  hero_name: "Luna",
  sample: 12,
  results: [
    {
      metric: "gold_per_min",
      label: "Gold per minute",
      higher_is_better: true,
      player_value: 550,
      player_sample: 12,
      peer_median: 480,
      top_20_value: 620,
      percentile: 68,
      gap_to_top_20: 70,
      peer_sample_size: 500,
      confidence: "adequate",
      segmented_by: ["hero"],
      note: null,
    },
  ],
  segmented_by: ["hero"],
  context: {
    hero_id: 35,
    hero_name: "Luna",
    role: null,
    role_label: null,
    rank_tier: null,
    requested: ["hero", "role", "rank_bracket", "patch"],
    segmented_by: ["hero"],
    unavailable: [],
    population: { player: "12 matches on Luna", peers: "OpenDota public sample", comparable: false, note: "" },
  },
  note: null,
};

afterEach(() => {
  getMatch.mockReset();
  getMatchAnalysis.mockReset();
  getBenchmark.mockReset();
});

describe("MatchDetail", () => {
  it("renders the combat, impact and benchmark charts alongside the existing result", async () => {
    getMatch.mockResolvedValue({ match: match() });
    getMatchAnalysis.mockResolvedValue(ANALYSIS);
    getBenchmark.mockResolvedValue(BENCHMARK);

    render(<MatchDetail id="3fa85f64-5717-4562-b3fc-2c963f66afa6" />);

    // The existing result panel is untouched.
    expect(await screen.findByText("Luna")).toBeDefined();
    expect(screen.getByText("8/4/12")).toBeDefined();

    // The new charts are attached, not swapped in.
    expect(screen.getByText("Combat")).toBeDefined();
    expect(screen.getByText("Impact")).toBeDefined();
    expect(await screen.findByText("Benchmark")).toBeDefined();
    expect(await screen.findByText(/Gold per minute/)).toBeDefined();
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
    getBenchmark.mockResolvedValue(BENCHMARK);

    render(<MatchDetail id="3fa85f64-5717-4562-b3fc-2c963f66afa6" />);

    expect(await screen.findByText("Combat")).toBeDefined();
    expect(screen.queryByText("Impact")).toBeNull();
  });
});
