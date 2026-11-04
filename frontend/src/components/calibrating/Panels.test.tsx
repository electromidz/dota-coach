import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { MethodologyNote } from "./MethodologyNote";
import { RankBreakdown } from "./RankBreakdown";
import { RankHeader } from "./RankHeader";
import { RESPONSE } from "./test-fixtures";

describe("RankHeader", () => {
  it("shows the medal with its status, and the settled count once", () => {
    render(<RankHeader data={RESPONSE} />);

    expect(screen.getByText("Archon 5")).toBeDefined();
    expect(screen.getByText("Calibrating")).toBeDefined();
    expect(screen.getByText("12 / 20")).toBeDefined();
    expect(screen.getAllByText(/12 \/ 20/)).toHaveLength(1);
  });

  it("labels every modeled number as an estimate", () => {
    render(<RankHeader data={RESPONSE} />);

    expect(screen.getByText("MMR · est.")).toBeDefined();
    expect(screen.getByText("~3,002")).toBeDefined();
    expect(screen.getByText("Next medal · est.")).toBeDefined();
    expect(screen.getByText("≈ 3 wins")).toBeDefined();
    expect(screen.getByText(/to Legend · 78 MMR/)).toBeDefined();
  });

  it("shows the streak and the last results in order", () => {
    render(<RankHeader data={RESPONSE} />);

    expect(screen.getByText("3W")).toBeDefined();
    expect(screen.getByRole("img", { name: /Last 6 ranked: 4 won/ })).toBeDefined();
  });

  it("shows no medal, MMR or next medal when none was reported", () => {
    render(
      <RankHeader
        data={{
          ...RESPONSE,
          established_rank: { rank_tier: null, label: null, leaderboard_rank: null, mmr: null, next_medal: null },
        }}
      />,
    );

    expect(screen.getByText("No rank")).toBeDefined();
    expect(screen.queryByText(/~/)).toBeNull();
    expect(screen.queryByText(/Unranked|Tier 0/)).toBeNull();
  });
});

describe("RankBreakdown", () => {
  it("names every position in the donut legend and the heroes' records", () => {
    render(<RankBreakdown data={RESPONSE} />);

    expect(screen.getByText("Carry")).toBeDefined();
    expect(screen.getByText("Mid")).toBeDefined();
    expect(screen.getByText("75%")).toBeDefined();
    expect(screen.getByText(/6–3 · 67%/)).toBeDefined();
    expect(screen.getByRole("img", { name: /Rating against result for 6 ranked games/ })).toBeDefined();
  });
});

describe("MethodologyNote", () => {
  /** Every number comes from the response, so the disclosure cannot drift
   *  from the model it describes. */
  it("quotes the server's model, not a hardcoded one", () => {
    render(
      <MethodologyNote
        methodology={{ win_base_mmr: 27, loss_base_mmr: 19, confidence_window_days: 14, confidence_target_matches: 12 }}
      />,
    );

    expect(screen.getByText("+27")).toBeDefined();
    expect(screen.getByText("−19")).toBeDefined();
    expect(screen.getByText("14")).toBeDefined();
    expect(screen.getByText(/12 settles it/)).toBeDefined();
    expect(screen.queryByText("+30")).toBeNull();
  });

  it("says plainly that the dashed points are not Valve's numbers", () => {
    render(<MethodologyNote methodology={RESPONSE.methodology} />);

    expect(screen.getByText(/has not published a per-match MMR change/)).toBeDefined();
  });
});
