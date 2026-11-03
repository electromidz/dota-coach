import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { HeroFit, HeroIntelligenceResponse, HeroPoolEntry } from "@/lib/types";

import { HeroIntelligence } from "./HeroIntelligence";

const getHeroIntelligence = vi.hoisted(() => vi.fn());
vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getHeroIntelligence };
});
vi.mock("@/lib/session-context", () => ({
  useSession: () => ({ session: { kind: "signed-in", me: { user: { is_admin: false } } }, setSession: vi.fn() }),
}));

function hero(id: number, name: string, over: Partial<HeroPoolEntry> = {}): HeroPoolEntry {
  return {
    hero_id: id,
    hero_name: name,
    role: "Support",
    matches: 7,
    wins: 4,
    losses: 3,
    win_rate: 4 / 7,
    shrunk_win_rate: 0.6,
    win_rate_delta: -0.03,
    recent_matches: 7,
    recent_win_rate: 4 / 7,
    avg_kda: 3,
    avg_gpm: 400,
    last_played_at: "2026-10-01T10:00:00Z",
    recent_results: [true, false, true, true, false, true, false],
    meta_strength: 58,
    tier: "comfort",
    tier_label: "Comfort",
    confidence: "low",
    ...over,
  };
}

function fit(id: number, name: string, matches: number, meta: number): HeroFit {
  return {
    hero_id: id,
    hero_name: name,
    fit_score: 64,
    level: "consider",
    level_label: "Consider",
    parts: [
      { component: "player_performance", label: "Player performance", score: 55, weight: 0.4, detail: "57% vs 67%." },
      { component: "meta_strength", label: "Meta strength", score: meta, weight: 0.3, detail: "Meta." },
    ],
    reasons: [],
    caveats: [],
    matches,
    tier: matches >= 5 ? "comfort" : null,
    meta_strength: meta,
    focus_adjustment: 0,
  };
}

function response(over: Partial<HeroIntelligenceResponse> = {}): HeroIntelligenceResponse {
  const stretch = Array.from({ length: 9 }, (_, i) =>
    hero(200 + i, `Stretch ${i}`, { matches: 1, wins: 1, losses: 0, win_rate: 1, win_rate_delta: null, tier: "stretch", tier_label: "Stretch", recent_results: [true] }),
  );
  return {
    scope: {
      role: "soft_support",
      role_label: "Soft Support",
      coaching_role: "soft_support",
      coaching_role_label: "Soft Support",
      population: "Ranked + All Pick",
      window: 100,
    },
    pool: [
      hero(9, "Mirana"),
      hero(87, "Disruptor", { matches: 6, wins: 1, losses: 5, win_rate: 1 / 6, win_rate_delta: -0.12, tier: "risk", tier_label: "Risk" }),
      ...stretch,
    ],
    summary: { heroes: 11, signature: 0, comfort: 1, stretch: 9, risk: 1, established: 2 },
    recommendations: [fit(9, "Mirana", 7, 58)],
    worth_trying: [fit(76, "Outworld Destroyer", 0, 89)],
    meta_leaders: [
      { hero_id: 76, hero_name: "Outworld Destroyer", roles: [], picks: 1000, wins: 560, win_rate: 0.56, pick_rate: 0.1, trend: null, meta_strength: 89, bracket: null },
      { hero_id: 9, hero_name: "Mirana", roles: [], picks: 900, wins: 470, win_rate: 0.52, pick_rate: 0.08, trend: null, meta_strength: 58, bracket: null },
    ],
    meta: { available: true, source: "OpenDota", bracket: "legend", bracket_label: "Legend", segmented_by: ["rank_bracket"], note: null },
    recent_window: 10,
    note: null,
    ...over,
  };
}

beforeEach(() => getHeroIntelligence.mockResolvedValue(response()));
afterEach(() => vi.clearAllMocks());

describe("HeroIntelligence", () => {
  it("states the scope and defaults to the coaching role", async () => {
    render(<HeroIntelligence />);
    expect(await screen.findByText("Soft Support · Ranked + All Pick · last 100")).toBeTruthy();
    expect(getHeroIntelligence).toHaveBeenCalledWith(undefined, undefined);
  });

  it("asks for every role when the filter is switched", async () => {
    render(<HeroIntelligence />);
    fireEvent.click(await screen.findByRole("radio", { name: "All roles" }));
    await waitFor(() => expect(getHeroIntelligence).toHaveBeenLastCalledWith(undefined, "all"));
  });

  it("plots only heroes with 3+ games on the fit-vs-meta chart", async () => {
    render(<HeroIntelligence />);
    const chart = await screen.findByRole("region", { name: "Fit vs meta" });
    expect(within(chart).getByRole("button", { name: /^Mirana:/ })).toBeTruthy();
    expect(within(chart).queryByRole("button", { name: /^Stretch 0:/ })).toBeNull();
  });

  it("keeps few-game meta heroes out of the recommendations", async () => {
    render(<HeroIntelligence />);
    const recommended = await screen.findByRole("region", { name: "Recommended for you" });
    expect(within(recommended).getByText("Mirana")).toBeTruthy();
    expect(within(recommended).queryByText("Outworld Destroyer")).toBeNull();
    const worth = screen.getByRole("region", { name: "Worth trying" });
    expect(within(worth).getByText("Outworld Destroyer")).toBeTruthy();
    expect(within(worth).getByText("0 games")).toBeTruthy();
    // No prose deltas anywhere.
    expect(screen.queryByText(/points more often/)).toBeNull();
  });

  it("orders the pool Signature, Comfort, Risk, Stretch and collapses Stretch", async () => {
    render(<HeroIntelligence />);
    const pool = await screen.findByRole("region", { name: "Your hero pool" });
    const tiers = within(pool)
      .getAllByRole("heading", { level: 3 })
      .map((h) => h.textContent?.replace(/\d+/g, "").trim().toLowerCase());
    expect(tiers).toEqual(["comfort", "risk", "stretch"]);
    fireEvent.click(within(pool).getByRole("button", { name: "+3 more" }));
    expect(within(pool).getByText("Stretch 8")).toBeTruthy();
  });

  it("highlights heroes from the pool in the meta chart", async () => {
    render(<HeroIntelligence />);
    const meta = await screen.findByRole("region", { name: "Current meta" });
    expect(within(meta).getByLabelText(/Mirana: meta 58.*in your pool/)).toBeTruthy();
    expect(
      within(meta).getByLabelText(/Outworld Destroyer: meta 89/).getAttribute("aria-label"),
    ).not.toMatch(/in your pool/);
  });
});
