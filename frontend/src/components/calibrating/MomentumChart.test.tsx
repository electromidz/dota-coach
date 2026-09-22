import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { Momentum, MomentumPoint } from "@/lib/types";

import { MomentumChart } from "./MomentumChart";

function point(
  index: number,
  won: boolean,
  delta: number,
  cumulative: number,
): MomentumPoint {
  return {
    index,
    match_id: 9_000 + index,
    hero_name: "Luna",
    won,
    delta,
    cumulative,
    started_at: `2026-09-${String(index).padStart(2, "0")}T12:00:00Z`,
  };
}

function momentum(overrides: Partial<Momentum> = {}): Momentum {
  return {
    points: [
      point(1, true, 30, 30),
      point(2, false, -25, 5),
      point(3, true, 30, 35),
      point(4, true, 30, 65),
    ],
    net: 65,
    wins: 3,
    losses: 1,
    window: 20,
    ...overrides,
  };
}

describe("MomentumChart", () => {
  it("reports the net movement and the record behind it", () => {
    render(<MomentumChart momentum={momentum()} />);

    expect(screen.getByText("+65")).toBeDefined();
    expect(screen.getByText("3W")).toBeDefined();
    expect(screen.getByText("1L")).toBeDefined();
    expect(screen.getByText(/Last 4 ranked matches/)).toBeDefined();
  });

  it("signs a falling window and says which way it went", () => {
    render(
      <MomentumChart
        momentum={momentum({
          points: [point(1, false, -25, -25), point(2, false, -25, -50)],
          net: -50,
          wins: 0,
          losses: 2,
        })}
      />,
    );

    expect(screen.getByText("−50")).toBeDefined();
    expect(screen.getByText(/down from where the window started/)).toBeDefined();
  });

  /**
   * The line this chart exists to hold. Other rank trackers print an absolute
   * MMR under the same curve; Valve publishes no figure that could confirm one,
   * so this reports movement relative to the start of the window and the
   * accessible description says so in words.
   */
  it("describes itself as relative and modeled, never as a rating", () => {
    render(<MomentumChart momentum={momentum()} />);

    const description = screen.getByRole("img").getAttribute("aria-label") ?? "";

    expect(description).toMatch(/modeled/i);
    expect(description).toMatch(/relative to the start of the window/i);
    expect(description).toMatch(/not an absolute rating/i);
  });

  it("marks each match by result, not only by the line's colour", () => {
    const { container } = render(<MomentumChart momentum={momentum()} />);

    const titles = Array.from(container.querySelectorAll("title")).map(
      (t) => t.textContent ?? "",
    );

    expect(titles.filter((t) => t.startsWith("Win"))).toHaveLength(3);
    expect(titles.filter((t) => t.startsWith("Loss"))).toHaveLength(1);
    expect(titles[0]).toMatch(/\+30 modeled/);
  });

  /**
   * The per-match movement is the content of this chart, not a detail of it.
   * Leaving it in a hover tooltip hides it from every touch device.
   */
  it("prints every match's own delta on the plot, not only in a tooltip", () => {
    const { container } = render(<MomentumChart momentum={momentum()} />);

    const labels = Array.from(container.querySelectorAll("text")).map(
      (t) => t.textContent,
    );

    expect(labels).toEqual(["+30", "−25", "+30", "+30"]);
  });

  it("puts a win's delta above its point and a loss's below", () => {
    const { container } = render(<MomentumChart momentum={momentum()} />);

    const circles = Array.from(container.querySelectorAll("circle"));
    const labels = Array.from(container.querySelectorAll("text"));

    for (const [i, label] of labels.entries()) {
      const pointY = Number(circles[i].getAttribute("cy"));
      const labelY = Number(label.getAttribute("y"));
      const won = momentum().points[i].won;

      // SVG y grows downward, so "above" is a smaller number.
      expect(won ? labelY < pointY : labelY > pointY).toBe(true);
    }
  });

  it("says so rather than drawing a line through a single match", () => {
    render(
      <MomentumChart
        momentum={momentum({ points: [point(1, true, 30, 30)], net: 30 })}
      />,
    );

    expect(screen.getByText(/One ranked match so far/)).toBeDefined();
  });

  it("is empty rather than a flat line with no matches", () => {
    render(
      <MomentumChart
        momentum={momentum({ points: [], net: 0, wins: 0, losses: 0 })}
      />,
    );

    expect(screen.getByText(/No recent ranked matches to plot/)).toBeDefined();
  });
});
