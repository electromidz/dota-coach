import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { TrajectoryPoint } from "@/lib/types";

import { TrajectoryChart } from "./TrajectoryChart";

function point(
  tier: number,
  day: number,
  estimated: boolean,
  label: string | null = `Tier ${tier}`,
): TrajectoryPoint {
  return {
    rank_tier: tier,
    label,
    at: `2026-09-${String(day).padStart(2, "0")}T12:00:00Z`,
    estimated,
  };
}

/** Two readings with three modeled points between them. */
const SERIES: TrajectoryPoint[] = [
  point(43, 1, false, "Archon 3"),
  point(44, 2, true, "Archon 4"),
  point(44, 3, true, "Archon 4"),
  point(45, 4, true, "Archon 5"),
  point(45, 5, false, "Archon 5"),
];

function marks(container: HTMLElement) {
  // The legend swatches live in their own `aria-hidden` SVGs; the plot is the
  // first one, and only its marks are the data.
  const plot = container.querySelector("svg[role='img']")!;
  return {
    lines: Array.from(plot.querySelectorAll("line")),
    circles: Array.from(plot.querySelectorAll("circle")),
  };
}

describe("TrajectoryChart", () => {
  /**
   * The honesty guarantee of the whole feature. Valve publishes no per-match
   * MMR, so a modeled point rendered like a measured one is a guess presented
   * as Valve's number. If this ever passes with identical styling, the chart
   * is lying.
   */
  it("renders estimated points visibly differently from recorded ones", () => {
    const { container } = render(<TrajectoryChart points={SERIES} />);
    const { lines, circles } = marks(container);

    const dashed = lines.filter((l) => l.getAttribute("stroke-dasharray"));
    const solid = lines.filter((l) => !l.getAttribute("stroke-dasharray"));

    expect(dashed.length).toBeGreaterThan(0);
    expect(solid.length).toBe(0);
    // Every segment here touches a modeled end, so all four are dashed — a
    // segment is only as certain as its least certain end.
    expect(dashed.length).toBe(4);

    // Markers: measured ones are filled with the series colour, modeled ones
    // are hollow — the surface shows through.
    const measured = circles.filter(
      (c) => c.getAttribute("fill") === "var(--color-mark-line)",
    );
    const modeled = circles.filter(
      (c) => c.getAttribute("fill") === "var(--color-surface-2)",
    );

    expect(measured).toHaveLength(2);
    expect(modeled).toHaveLength(3);

    // And the two differ in radius as well as fill, so the distinction
    // survives a forced-colors mode that flattens fills.
    expect(measured[0].getAttribute("r")).not.toBe(modeled[0].getAttribute("r"));
  });

  it("keeps a segment between two recorded readings solid", () => {
    const { container } = render(
      <TrajectoryChart
        points={[point(43, 1, false, "Archon 3"), point(45, 5, false, "Archon 5")]}
      />,
    );
    const { lines } = marks(container);

    expect(lines).toHaveLength(1);
    expect(lines[0].getAttribute("stroke-dasharray")).toBeNull();
  });

  /**
   * The distinction has to reach a reader who never sees the line style, so it
   * is stated in words too — not only in the stroke.
   */
  it("states the measured and modeled split in the accessible description", () => {
    render(<TrajectoryChart points={SERIES} />);

    const figure = screen.getByRole("img");
    const description = figure.getAttribute("aria-label") ?? "";

    expect(description).toMatch(/2 recorded rank readings/);
    expect(description).toMatch(/3 estimated points/);
    expect(description).toMatch(/Archon 3/);
    expect(description).toMatch(/Archon 5/);
  });

  it("names both states in a legend, so nothing is carried by styling alone", () => {
    render(<TrajectoryChart points={SERIES} />);

    expect(screen.getByText("Recorded rank")).toBeDefined();
    expect(screen.getByText("Estimated")).toBeDefined();
  });

  it("says so rather than drawing a line through a single reading", () => {
    render(<TrajectoryChart points={[point(43, 1, false)]} />);

    expect(screen.getByText(/One rank reading so far/)).toBeDefined();
  });

  it("falls back to the raw tier rather than shipping a second medal table", () => {
    const { container } = render(
      <TrajectoryChart
        points={[point(43, 1, false, null), point(45, 5, false, null)]}
      />,
    );

    expect(container.querySelector("title")?.textContent).toMatch(/Tier 43/);
  });
});
