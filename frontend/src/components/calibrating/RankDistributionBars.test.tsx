import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type {
  BracketResemblance,
  RankBracket,
  RankDistribution,
} from "@/lib/types";

import { RankDistributionBars } from "./RankDistributionBars";

function share(
  bracket: RankBracket,
  label: string,
  percentage: number,
  overrides: Partial<BracketResemblance> = {},
): BracketResemblance {
  return {
    bracket,
    label,
    percentage,
    is_highest: false,
    is_player_bracket: false,
    ...overrides,
  };
}

function distribution(
  overrides: Partial<RankDistribution> = {},
): RankDistribution {
  return {
    hero_id: 35,
    hero_name: "Luna",
    sample: 24,
    fits: [],
    closest: "archon",
    resemblance: [
      share("archon", "Archon", 46, { is_highest: true, is_player_bracket: true }),
      share("legend", "Legend", 28),
      share("crusader", "Crusader", 18),
      share("ancient", "Ancient", 8),
    ],
    own_bracket_metrics: [],
    consistency: { percentage: 72, matches: 24 },
    note: null,
    ...overrides,
  };
}

describe("RankDistributionBars", () => {
  it("draws one bar per bracket with its share, strongest first", () => {
    const { container } = render(
      <RankDistributionBars distribution={distribution()} />,
    );

    expect(screen.getByText("46%")).toBeDefined();
    expect(screen.getByText("28%")).toBeDefined();

    const labels = Array.from(container.querySelectorAll("[title]")).map(
      (el) => el.textContent,
    );
    expect(labels).toEqual(["Archon", "Legend", "Crusader", "Ancient"]);
  });

  it("marks the player's own medal", () => {
    render(<RankDistributionBars distribution={distribution()} />);

    expect(screen.getByText("your medal")).toBeDefined();
  });

  /**
   * The reason the server sends resemblance rather than percentiles. A
   * percentile chart sorted descending puts the bracket the player *crushes*
   * at the top — beating 98% of Heralds is the highest number on the board and
   * the furthest thing from being a Herald.
   */
  it("leads with the bracket they resemble, not the one they beat", () => {
    const { container } = render(
      <RankDistributionBars
        distribution={distribution({
          resemblance: [
            share("archon", "Archon", 52, { is_highest: true }),
            share("legend", "Legend", 30),
            share("herald", "Herald", 4),
          ],
        })}
      />,
    );

    const first = container.querySelector("[title]");
    expect(first?.textContent).toBe("Archon");
  });

  /** A percentage next to a rank invites reading it as odds. It is not. */
  it("states plainly that the shares are not calibration odds", () => {
    render(<RankDistributionBars distribution={distribution()} />);

    expect(
      screen.getByText(/Not a chance of calibrating there/i),
    ).toBeDefined();
    expect(
      screen.getByText(/Valve publishes no calibration outcomes/i),
    ).toBeDefined();
  });

  it("names the hero and sample the comparison rests on", () => {
    render(<RankDistributionBars distribution={distribution()} />);

    expect(screen.getByText(/Luna numbers over 24 matches/)).toBeDefined();
  });

  it("surfaces the server's note when brackets are missing", () => {
    render(
      <RankDistributionBars
        distribution={distribution({
          note: "3 of 8 brackets could not be fetched and are shown without a placement.",
        })}
      />,
    );

    expect(screen.getByText(/3 of 8 brackets could not be fetched/)).toBeDefined();
  });

  it("falls back to the note when nothing could be placed", () => {
    render(
      <RankDistributionBars
        distribution={distribution({
          resemblance: [],
          closest: null,
          note: "Benchmarks are unavailable right now.",
        })}
      />,
    );

    expect(screen.getByText("Benchmarks are unavailable right now.")).toBeDefined();
  });
});
