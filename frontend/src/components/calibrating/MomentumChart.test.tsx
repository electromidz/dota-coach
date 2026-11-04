import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { MomentumCard } from "./MomentumChart";
import { momentumPoints, RESPONSE } from "./test-fixtures";

const momentum = RESPONSE.momentum;

describe("MomentumCard", () => {
  it("leads with the record, the net estimate and the averages", () => {
    render(<MomentumCard momentum={momentum} />);

    expect(screen.getByText("4W – 2L")).toBeDefined();
    expect(screen.getByText("+70 est.")).toBeDefined();
    expect(screen.getByText(/Avg \+30 \/\s*−25/)).toBeDefined();
  });

  it("draws one bar per game and labels only the last point", () => {
    const { container } = render(<MomentumCard momentum={momentum} />);
    const plot = container.querySelector("svg[role='img']")!;

    // One link per game, each opening its match.
    const links = plot.querySelectorAll("a");
    expect(links).toHaveLength(6);
    expect(links[0].getAttribute("href")).toBe("/matches/match-1");
    // The last point is labelled with the running total: 4 × 30 − 2 × 25.
    expect(plot.textContent).toContain("+70");
  });

  it("shows hero, result, K/D/A, rating and the estimate in the tooltip only", () => {
    const { container } = render(<MomentumCard momentum={momentum} />);
    expect(screen.queryByRole("status")).toBeNull();

    const hit = container.querySelectorAll("svg[role='img'] a rect")[1];
    fireEvent.pointerEnter(hit);

    const tip = screen.getByRole("status");
    expect(tip.textContent).toContain("Luna");
    expect(tip.textContent).toContain("Lost");
    expect(tip.textContent).toContain("8/3/10");
    expect(tip.textContent).toContain("rating 6.5");
    expect(tip.textContent).toContain("−25 est.");
  });

  it("shows an empty state below five ranked games", () => {
    render(
      <MomentumCard momentum={{ ...momentum, points: momentumPoints([true, false, true]), wins: 2, losses: 1, net: 35 }} />,
    );

    expect(screen.getByText(/3 ranked games so far/)).toBeDefined();
    expect(document.querySelector("svg[role='img']")).toBeNull();
  });

  it("says when there are no ranked games at all", () => {
    render(<MomentumCard momentum={{ ...momentum, points: [], wins: 0, losses: 0, net: 0, avg_win_delta: null, avg_loss_delta: null }} />);
    expect(screen.getByText(/No ranked games yet/)).toBeDefined();
  });
});
