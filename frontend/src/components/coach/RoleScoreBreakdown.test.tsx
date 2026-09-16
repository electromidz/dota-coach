import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { RolePerformance } from "@/lib/types";

import { RoleScoreBreakdown } from "./RoleScoreBreakdown";

function performance(overrides: Partial<RolePerformance> = {}): RolePerformance {
  return {
    role: "carry",
    role_label: "Carry",
    position: 1,
    matches: 30,
    wins: 18,
    losses: 12,
    win_rate: 0.6,
    avg_kda: 3.4,
    avg_gpm: 520,
    avg_xpm: 600,
    avg_last_hits_per_min: 5.1,
    avg_deaths_per_10: 1.8,
    avg_kill_participation: 0.62,
    kill_participation_sample: 30,
    performance: 64,
    raw_performance: 64,
    confidence: "moderate",
    components: [
      {
        key: "win_rate",
        label: "Win rate",
        value: 0.6,
        normalized: 60,
        weight: 0.45,
        sample: 30,
      },
      {
        key: "deaths_per_10",
        label: "Deaths per 10 minutes",
        value: 1.8,
        normalized: 40,
        weight: 0.55,
        sample: 30,
      },
    ],
    ...overrides,
  };
}

describe("RoleScoreBreakdown", () => {
  it("shows each measure in its own units beside the weight it carries", () => {
    render(<RoleScoreBreakdown performance={performance()} />);

    expect(screen.getByText("Win rate")).toBeDefined();
    expect(screen.getByText("60%")).toBeDefined();
    expect(screen.getByText("45% of score")).toBeDefined();
    expect(screen.getByText("1.8")).toBeDefined();
  });

  /**
   * The adjustment is the least intuitive part of the score, so a player
   * comparing a thin role against a thick one has to be told it happened.
   */
  it("explains the sample-size adjustment when it moved the number", () => {
    render(
      <RoleScoreBreakdown
        performance={performance({
          matches: 5,
          raw_performance: 85,
          performance: 62,
        })}
      />,
    );

    expect(screen.getByText(/Measured 85\/100/)).toBeDefined();
    expect(screen.getByText(/reported as 62/)).toBeDefined();
  });

  it("stays quiet when the adjustment changed nothing", () => {
    render(<RoleScoreBreakdown performance={performance()} />);

    expect(screen.queryByText(/Measured/)).toBeNull();
  });

  /** A role whose measures could not be computed has nothing to explain. */
  it("renders nothing without components", () => {
    const { container } = render(
      <RoleScoreBreakdown performance={performance({ components: [] })} />,
    );

    expect(container.firstChild).toBeNull();
  });
});
