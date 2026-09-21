import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  BenchmarkResponse,
  ResolvedBracket,
  TargetComparison,
} from "@/lib/types";

import { Benchmark } from "./Benchmark";

const getBenchmark = vi.hoisted(() => vi.fn());
const getStats = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getBenchmark, getStats };
});

vi.mock("@/lib/session-context", () => ({
  useSession: () => ({
    session: { kind: "signed-in", me: {} },
    setSession: vi.fn(),
  }),
}));

const BRACKETS = [
  "herald",
  "guardian",
  "crusader",
  "archon",
  "legend",
  "ancient",
  "divine",
  "immortal",
] as const;

/** Ancient's numbers, sitting above the player on gold and below on deaths. */
function target(overrides: Partial<TargetComparison> = {}): TargetComparison {
  return {
    bracket: {
      requested: "ancient",
      used: "ancient",
      label: "Ancient",
      fell_back: false,
    },
    label: "Ancient",
    metrics: [
      {
        metric: "gold_per_min",
        label: "Gold per minute",
        higher_is_better: true,
        peer_median: 612,
        top_20_value: 700,
        percentile: 21,
        gap_to_median: 100,
        cleared: false,
      },
      {
        metric: "deaths_per_min",
        label: "Deaths per minute",
        higher_is_better: false,
        peer_median: 0.2,
        top_20_value: 0.14,
        percentile: 62,
        gap_to_median: -0.02,
        cleared: true,
      },
    ],
    metrics_cleared: 1,
    metrics_compared: 2,
    ...overrides,
  };
}

function response(
  overrides: Partial<BenchmarkResponse> = {},
  bracket: Partial<ResolvedBracket> = {},
): BenchmarkResponse {
  return {
    hero_id: 35,
    hero_name: "Luna",
    sample: 20,
    results: [
      {
        metric: "gold_per_min",
        label: "Gold per minute",
        higher_is_better: true,
        player_value: 512,
        player_sample: 20,
        peer_median: 540,
        top_20_value: 610,
        percentile: 44,
        gap_to_top_20: 98,
        peer_sample_size: null,
        confidence: "adequate",
        segmented_by: ["hero", "rank_bracket"],
        note: null,
      },
      {
        metric: "deaths_per_min",
        label: "Deaths per minute",
        higher_is_better: false,
        player_value: 0.18,
        player_sample: 20,
        peer_median: 0.19,
        top_20_value: 0.13,
        percentile: 55,
        gap_to_top_20: 0.05,
        peer_sample_size: null,
        confidence: "adequate",
        segmented_by: ["hero", "rank_bracket"],
        note: null,
      },
    ],
    segmented_by: ["hero", "rank_bracket"],
    context: {
      hero_id: 35,
      hero_name: "Luna",
      role: null,
      role_label: null,
      rank_tier: 55,
      bracket: {
        requested: "legend",
        used: "legend",
        label: "Legend",
        fell_back: false,
        ...bracket,
      },
      requested: ["hero", "role", "rank_bracket", "patch"],
      segmented_by: ["hero", "rank_bracket"],
      unavailable: [],
      population: {
        player: "Your eligible matches on Luna.",
        peers: "Public matches on Luna in the Legend bracket.",
        comparable: false,
        note: "Read these percentiles as a close placement.",
      },
    },
    target: target(),
    brackets: BRACKETS.map((value) => ({
      value,
      label: value[0].toUpperCase() + value.slice(1),
      is_player_rank: value === "legend",
    })),
    note: null,
    ...overrides,
  };
}

beforeEach(() => {
  getBenchmark.mockResolvedValue(response());
  getStats.mockResolvedValue({ heroes: [] });
  sessionStorage.clear();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("Benchmark rank comparison", () => {
  it("shows the next rank up with no interaction", async () => {
    render(<Benchmark />);
    await screen.findByLabelText("Aiming at");

    // Absent, not "legend": asking for nothing is what asks the server for the
    // next rung, and the client never derives the rank order itself.
    expect(getBenchmark).toHaveBeenCalledWith(undefined, undefined, undefined);
    expect(screen.getByText("Next rank up (Ancient)")).toBeTruthy();
  });

  it("leads with how many metrics already clear the target", async () => {
    render(<Benchmark />);

    expect(
      await screen.findByText(/You already clear Ancient.s median on/),
    ).toBeTruthy();
    // The backend's count, rendered rather than recomputed from the rows.
    const headline = screen.getByText(/already clear Ancient/).textContent ?? "";
    expect(headline).toContain("1");
    expect(headline).toContain("2");
  });

  it("carries both rank marks and the shortfall on every row", async () => {
    render(<Benchmark />);
    await screen.findByLabelText("Aiming at");

    // Own bracket and target named with their values, so the marks are never
    // read by position and colour alone.
    expect(screen.getByText(/Legend 540/)).toBeTruthy();
    expect(screen.getByText(/Ancient 612/)).toBeTruthy();
    expect(screen.getByText("100 short of Ancient")).toBeTruthy();

    // Direction is honoured: fewer deaths than Ancient's median is ahead.
    expect(screen.getByText("already past Ancient")).toBeTruthy();

    // The top-20% line steps aside so three marks never share one bar.
    expect(screen.queryByText(/top 20%/)).toBeNull();
  });

  it("offers every bracket, plus the default and the opt-out", async () => {
    render(<Benchmark />);
    await screen.findByLabelText("Aiming at");

    expect(screen.getByText("Next rank up (Ancient)")).toBeTruthy();
    expect(screen.getByText("Your rank only (Legend)")).toBeTruthy();
    expect(screen.getByText("Legend — your rank")).toBeTruthy();
    expect(screen.getByText("Immortal")).toBeTruthy();
  });

  it("asks for a chosen bracket and for none when opted out", async () => {
    render(<Benchmark />);
    await screen.findByLabelText("Aiming at");

    fireEvent.change(screen.getByLabelText("Aiming at"), {
      target: { value: "divine" },
    });
    await waitFor(() =>
      expect(getBenchmark).toHaveBeenLastCalledWith(
        undefined,
        undefined,
        "divine",
      ),
    );

    fireEvent.change(screen.getByLabelText("Aiming at"), {
      target: { value: "none" },
    });
    await waitFor(() =>
      expect(getBenchmark).toHaveBeenLastCalledWith(
        undefined,
        undefined,
        "none",
      ),
    );
  });

  it("never moves the player's own standing when the target changes", async () => {
    render(<Benchmark />);
    await screen.findByLabelText("Aiming at");

    // p44 is where they sit in Legend. Aiming at Divine is a question about
    // Divine, and must not restate where the player stands.
    expect(screen.getByText("p44")).toBeTruthy();
    expect(screen.getByText(/vs Legend/)).toBeTruthy();

    getBenchmark.mockResolvedValue(
      response({
        target: target({
          label: "Divine",
          bracket: {
            requested: "divine",
            used: "divine",
            label: "Divine",
            fell_back: false,
          },
        }),
      }),
    );
    fireEvent.change(screen.getByLabelText("Aiming at"), {
      target: { value: "divine" },
    });

    await screen.findByText(/already clear Divine/);
    expect(screen.getByText("p44")).toBeTruthy();
    expect(screen.getByText(/vs Legend/)).toBeTruthy();
  });

  it("falls silent about a rank the provider has no data for", async () => {
    render(<Benchmark />);
    await screen.findByLabelText("Aiming at");

    getBenchmark.mockResolvedValue(response({ target: null }));
    fireEvent.change(screen.getByLabelText("Aiming at"), {
      target: { value: "immortal" },
    });

    expect(await screen.findByText("No data for that rank")).toBeTruthy();
    expect(screen.getByText(/nothing has been estimated/i)).toBeTruthy();
    // The player's own comparison survives, with its top-20% line back on
    // every row now that nothing is competing for the space.
    expect(screen.getByText("p44")).toBeTruthy();
    expect(screen.getAllByText(/top 20%/)).toHaveLength(2);
  });

  it("says nothing when the server itself chose no target", async () => {
    // An Immortal player has no rank above them. That is not a failed request
    // and must not be reported as one.
    getBenchmark.mockResolvedValue(response({ target: null }));
    render(<Benchmark />);

    await screen.findByLabelText("Aiming at");
    expect(screen.queryByText("No data for that rank")).toBeNull();
    expect(screen.queryByText(/already clear/)).toBeNull();
  });

  it("remembers the chosen rank for the session", async () => {
    const { unmount } = render(<Benchmark />);
    await screen.findByLabelText("Aiming at");

    fireEvent.change(screen.getByLabelText("Aiming at"), {
      target: { value: "divine" },
    });
    await waitFor(() =>
      expect(sessionStorage.getItem("benchmark.bracket")).toBe("divine"),
    );

    unmount();
    vi.clearAllMocks();
    getBenchmark.mockResolvedValue(response());
    render(<Benchmark />);

    // Restored in the first request rather than after an extra round trip.
    await waitFor(() => expect(getBenchmark).toHaveBeenCalledTimes(1));
    expect(getBenchmark).toHaveBeenCalledWith(undefined, undefined, "divine");
  });
});
