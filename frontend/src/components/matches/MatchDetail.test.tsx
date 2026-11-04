import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { CoachResponse, MatchComparisonResponse, MatchView } from "@/lib/types";

import { MatchDetail } from "./MatchDetail";

const getMatch = vi.hoisted(() => vi.fn());
const getMatchAnalysis = vi.hoisted(() => vi.fn());
const getMatchComparison = vi.hoisted(() => vi.fn());
const setMatchRole = vi.hoisted(() => vi.fn());
const getMatchTimeline = vi.hoisted(() =>
  vi.fn(() => Promise.resolve({ available: false, note: "x", duration_seconds: 0, net_worth_per_minute: [], deaths: [] })),
);

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getMatch, getMatchAnalysis, getMatchComparison, setMatchRole, getMatchTimeline };
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
    position: "carry",
    position_label: "Carry",
    position_source: "lane",
    peer_gpm: 500,
    peer_xpm: 550,
    rating: 7.2,
    mmr_delta_estimate: 29,
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
  item_timings: [],
  laning: [],
  note: null,
};

afterEach(() => {
  getMatch.mockReset();
  getMatchAnalysis.mockReset();
  getMatchComparison.mockReset();
});

describe("MatchDetail", () => {
  it("leads with a game score and one strip of figures against peers and your average", async () => {
    getMatch.mockResolvedValue({ match: match() });
    getMatchAnalysis.mockResolvedValue(ANALYSIS);
    getMatchComparison.mockResolvedValue(COMPARISON);

    render(<MatchDetail id="3fa85f64-5717-4562-b3fc-2c963f66afa6" />);

    expect(await screen.findByText("Luna", { selector: "h1" })).toBeDefined();
    expect(screen.getByText("8/4/12")).toBeDefined();
    expect(
      await screen.findByRole("img", { name: /Game score against Legend players, 72nd percentile, your avg 54th/ }),
    ).toBeDefined();

    // GPM tile: the value, this game's percentile, and the gap to your average.
    expect(screen.getAllByText("550").length).toBeGreaterThan(0);
    expect(screen.getAllByText("78th").length).toBeGreaterThan(0);
    expect(screen.getByText("+10% vs your avg")).toBeDefined();
    // Net worth has no peer metric, so no badge and no baseline.
    expect(screen.getAllByText("no baseline").length).toBeGreaterThan(0);

    // The duplicate bar cards are gone; the same-scale chart is central.
    expect(screen.queryByText("Combat")).toBeNull();
    expect(screen.queryByText("Impact")).toBeNull();
    expect(screen.getByText("Every metric, same scale")).toBeDefined();
  });

  it("shows a dash for figures the summary sync did not carry", async () => {
    getMatch.mockResolvedValue({
      match: match({ detail_synced: false, hero_damage: null, tower_damage: null, hero_healing: null }),
    });
    getMatchAnalysis.mockResolvedValue(ANALYSIS);
    getMatchComparison.mockResolvedValue(COMPARISON);

    render(<MatchDetail id="3fa85f64-5717-4562-b3fc-2c963f66afa6" />);

    expect(await screen.findByText(/Only the summary was available/)).toBeDefined();
    expect(screen.getAllByTitle("Not available").length).toBe(2);
  });

  it("shows the match id unformatted with provider links, and hides zero healing", async () => {
    getMatchAnalysis.mockReturnValue(new Promise(() => {}));
    getMatchComparison.mockReturnValue(new Promise(() => {}));
    getMatch.mockResolvedValue({
      match: match({ match_id: 8123456789, hero_healing: 0, started_at: "2026-09-28T14:23:00Z" }),
    });
    render(<MatchDetail id="m1" />);

    expect(await screen.findByText("#8123456789")).toBeDefined();
    expect(screen.queryByText("8,123,456,789")).toBeNull();
    expect(screen.getByRole("link", { name: "OpenDota" }).getAttribute("href")).toBe(
      "https://www.opendota.com/matches/8123456789",
    );
    expect(screen.queryByText("Hero healing")).toBeNull();
    // "Sep 28, 2026 · HH:MM" in the viewer's zone, no seconds.
    expect(screen.getByText(/^Sep 28, 2026 · \d{2}:\d{2}$/)).toBeDefined();
  });

  it("says how sure the position is and saves a correction", async () => {
    getMatchAnalysis.mockReturnValue(new Promise(() => {}));
    getMatchComparison.mockReturnValue(new Promise(() => {}));
    getMatch.mockResolvedValue({
      match: match({ position: "offlane", position_label: "Offlane", position_source: "estimated" }),
    });
    setMatchRole.mockResolvedValue({
      match: match({ position: "soft_support", position_label: "Soft Support", position_source: "user" }),
    });
    render(<MatchDetail id="m1" />);

    expect(await screen.findByText("estimated")).toBeDefined();
    fireEvent.change(screen.getByLabelText("Correct the position"), {
      target: { value: "soft_support" },
    });
    await waitFor(() => expect(setMatchRole).toHaveBeenCalledWith(
        "3fa85f64-5717-4562-b3fc-2c963f66afa6",
        "soft_support",
      ),
    );
    expect(await screen.findByText("set by you")).toBeDefined();
    expect(screen.getByText("Soft Support", { selector: "span" })).toBeDefined();
  });
});
