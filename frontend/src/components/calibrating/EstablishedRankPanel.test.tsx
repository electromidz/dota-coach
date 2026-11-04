import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { BenchmarkResult, EstablishedRank } from "@/lib/types";

import { EstablishedRankPanel } from "./EstablishedRankPanel";

const RANK: EstablishedRank = {
  rank_tier: 45,
  label: "Archon 5",
  leaderboard_rank: null,
  mmr: { low: 2926, high: 3079, midpoint: 3002 },
};

function metric(
  name: string,
  label: string,
  percentile: number | null,
): BenchmarkResult {
  return {
    metric: name,
    label,
    higher_is_better: true,
    player_value: 512,
    player_sample: 24,
    peer_median: 480,
    top_20_value: 600,
    percentile,
    gap_to_top_20: null,
    peer_sample_size: 500,
    confidence: "adequate",
    segmented_by: ["hero", "rank_bracket"],
    note: null,
  };
}

function panel(props: Partial<Parameters<typeof EstablishedRankPanel>[0]> = {}) {
  return (
    <EstablishedRankPanel
      rank={RANK}
      consistency={{ percentage: 72, matches: 24 }}
      metrics={[
        metric("gold_per_min", "Gold per minute", 70),
        metric("kda", "KDA", 55),
      ]}
      {...props}
    />
  );
}

describe("EstablishedRankPanel", () => {
  it("leads with the medal, and leaves confidence to the rank card", () => {
    render(panel());

    expect(screen.getByText("Official established rank")).toBeDefined();
    expect(screen.getByText("Archon 5")).toBeDefined();
    // Shown once on the page, in the header card; no "% match" badge.
    expect(screen.queryByText(/Rank confidence/)).toBeNull();
    expect(screen.queryByText("Calibrated")).toBeNull();
    expect(screen.queryByText(/% match/)).toBeNull();
  });

  it("defines consistency behind an (i)", () => {
    render(panel());

    expect(screen.getByRole("button", { name: /About consistency/ })).toBeDefined();
  });

  it("reports consistency with the sample behind it", () => {
    render(panel());

    expect(screen.getByText("72%")).toBeDefined();
    expect(screen.getByText(/over 24 matches/)).toBeDefined();
  });

  /**
   * The fabricated-default case. Some trackers answer a plausible-looking 75%
   * when they have nothing; below the floor the server sends null and this has
   * to say so rather than print a number nobody measured.
   */
  it("says consistency is unmeasured rather than inventing a figure", () => {
    render(panel({ consistency: null }));

    expect(screen.getByText(/needs 10 ranked matches/)).toBeDefined();
    expect(screen.queryByText("75%")).toBeNull();
  });

  it("places the player against their own bracket, per metric", () => {
    render(panel());

    expect(screen.getByText("Against your own bracket")).toBeDefined();
    expect(screen.getByText("Gold per minute")).toBeDefined();
    expect(screen.getByText("70")).toBeDefined();
    expect(screen.getByText(/Measured, not modelled/)).toBeDefined();
  });

  it("omits a metric the provider could not rank", () => {
    render(panel({ metrics: [metric("xp_per_min", "XP per minute", null)] }));

    expect(screen.queryByText("XP per minute")).toBeNull();
    expect(screen.queryByText("Against your own bracket")).toBeNull();
  });

  /**
   * The MMR figure lives on `RankCard`, beside the medal it is derived from —
   * not in this panel, which is about measurements. Keeping it in one place
   * stops the estimate appearing twice and reading as two findings.
   */
  it("leaves the MMR estimate to the rank card", () => {
    const { container } = render(panel());

    expect(container.textContent).not.toMatch(/\bMMR\b/);
  });

  it("shows no medal when none was reported", () => {
    render(
      panel({
        rank: { rank_tier: null, label: null, leaderboard_rank: null, mmr: null },
      }),
    );

    expect(screen.getByText("No rank reported")).toBeDefined();
    expect(screen.queryByText(/% match/)).toBeNull();
  });
});
