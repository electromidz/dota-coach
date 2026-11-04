import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { TrajectoryPoint } from "@/lib/types";

import { TrajectoryChart } from "./TrajectoryChart";

/** Points dated relative to now, so the default 90-day range holds them. */
function point(tier: number, daysAgo: number, estimated: boolean, gap_before = false): TrajectoryPoint {
  return {
    rank_tier: tier,
    label: `Tier ${tier}`,
    at: new Date(Date.now() - daysAgo * 86_400_000).toISOString(),
    estimated,
    gap_before,
  };
}

function plot(container: HTMLElement) {
  const svg = container.querySelector("svg[role='img']")!;
  return { lines: Array.from(svg.querySelectorAll("line")).filter((l) => l.getAttribute("stroke") === "var(--color-mark-line)"), circles: Array.from(svg.querySelectorAll("circle")), svg };
}

describe("TrajectoryChart", () => {
  it("renders estimated points visibly differently from recorded ones", () => {
    const { container } = render(
      <TrajectoryChart points={[point(43, 10, false), point(44, 8, true), point(45, 6, false)]} />,
    );
    const { lines, circles } = plot(container);

    expect(lines.every((l) => l.getAttribute("stroke-dasharray") === "4 4")).toBe(true);
    const hollow = circles.filter((c) => c.getAttribute("fill") === "var(--color-surface-2)");
    expect(hollow).toHaveLength(1);
  });

  it("keeps a segment between two recorded readings solid", () => {
    const { container } = render(<TrajectoryChart points={[point(44, 10, false), point(45, 5, false)]} />);
    const { lines } = plot(container);

    expect(lines).toHaveLength(1);
    expect(lines[0].getAttribute("stroke-dasharray")).toBeNull();
  });

  it("draws no line across a stretch with no ranked games", () => {
    const { container } = render(
      <TrajectoryChart points={[point(44, 30, false), point(44, 29, false), point(43, 5, false, true)]} />,
    );
    const { lines, circles } = plot(container);

    expect(circles).toHaveLength(3);
    expect(lines).toHaveLength(1);
  });

  it("labels medal changes and draws medal bands on the axis", () => {
    const { container } = render(<TrajectoryChart points={[point(45, 10, false), point(51, 5, false)]} />);
    const { svg } = plot(container);

    expect(svg.textContent).toContain("→ Legend");
    expect(svg.textContent).toContain("Archon");
  });

  it("narrows to the chosen range", () => {
    render(<TrajectoryChart points={[point(44, 200, false), point(45, 150, false), point(45, 5, false)]} />);
    // 90 days by default: one point left, so no line.
    expect(screen.getByText(/Fewer than two readings in this range/)).toBeDefined();

    fireEvent.click(screen.getByRole("radio", { name: "All" }));
    expect(screen.getByRole("img", { name: /3 recorded rank readings/ })).toBeDefined();
  });

  it("asks for a second reading when there is only one", () => {
    render(<TrajectoryChart points={[point(44, 1, false)]} />);
    expect(screen.getByText(/One rank reading so far/)).toBeDefined();
  });
});
