import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { BenchmarkResponse, ResolvedBracket } from "@/lib/types";

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
    ],
    segmented_by: ["hero", "rank_bracket"],
    context: {
      hero_id: 35,
      hero_name: "Luna",
      role: null,
      role_label: null,
      rank_tier: 41,
      bracket: {
        requested: "archon",
        used: "archon",
        label: "Archon",
        fell_back: false,
        ...bracket,
      },
      requested: ["hero", "role", "rank_bracket", "patch"],
      segmented_by: ["hero", "rank_bracket"],
      unavailable: [],
      population: {
        player: "Your eligible matches on Luna.",
        peers: "Public matches on Luna in the Archon bracket.",
        comparable: false,
        note: "Read these percentiles as a close placement.",
      },
    },
    brackets: BRACKETS.map((value) => ({
      value,
      label: value[0].toUpperCase() + value.slice(1),
      is_player_rank: value === "archon",
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

describe("Benchmark rank selection", () => {
  it("offers every bracket the backend listed, marking the player's own", async () => {
    render(<Benchmark />);

    expect(await screen.findByLabelText("Compare against")).toBeTruthy();
    expect(screen.getByText("Your rank (Archon)")).toBeTruthy();
    expect(screen.getByText("Archon — your rank")).toBeTruthy();
    expect(screen.getByText("Ancient")).toBeTruthy();
    expect(screen.getByText("Immortal")).toBeTruthy();
  });

  it("defaults to the player's own rank without naming a bracket", async () => {
    render(<Benchmark />);
    await screen.findByLabelText("Compare against");

    // `undefined`, not "archon": "my rank" is a different request from any
    // named bracket, and the backend resolves it.
    expect(getBenchmark).toHaveBeenCalledWith(undefined, undefined, undefined);
  });

  it("asks for the chosen bracket and keeps the hero", async () => {
    render(<Benchmark />);
    await screen.findByLabelText("Compare against");

    fireEvent.change(screen.getByLabelText("Compare against"), {
      target: { value: "ancient" },
    });

    await waitFor(() =>
      expect(getBenchmark).toHaveBeenLastCalledWith(
        undefined,
        undefined,
        "ancient",
      ),
    );
  });

  it("remembers the bracket for the session", async () => {
    const { unmount } = render(<Benchmark />);
    await screen.findByLabelText("Compare against");

    fireEvent.change(screen.getByLabelText("Compare against"), {
      target: { value: "divine" },
    });
    await waitFor(() => expect(sessionStorage.getItem("benchmark.bracket")).toBe("divine"));

    unmount();
    vi.clearAllMocks();
    getBenchmark.mockResolvedValue(response());
    render(<Benchmark />);

    // Restored in the *first* request rather than after an extra round trip.
    await waitFor(() => expect(getBenchmark).toHaveBeenCalledTimes(1));
    expect(getBenchmark).toHaveBeenCalledWith(undefined, undefined, "divine");
  });

  it("says so outright when the chosen bracket had no data", async () => {
    render(<Benchmark />);
    await screen.findByLabelText("Compare against");

    getBenchmark.mockResolvedValue(
      response({}, { requested: "immortal", used: null, label: "All ranks", fell_back: true }),
    );
    fireEvent.change(screen.getByLabelText("Compare against"), {
      target: { value: "immortal" },
    });

    expect(await screen.findByText("No data for that bracket")).toBeTruthy();
    expect(screen.getByText(/cover every rank instead/)).toBeTruthy();
    // And the page reports the bracket it actually used, not the one asked for.
    expect(screen.getByText(/vs All ranks/)).toBeTruthy();
  });

  it("keeps the fallback quiet on the default path", async () => {
    // An unranked player falls back with nothing requested. That is not a
    // failed choice, so it must not raise an alert.
    getBenchmark.mockResolvedValue(
      response({}, { requested: null, used: null, label: "All ranks", fell_back: false }),
    );
    render(<Benchmark />);

    await screen.findByLabelText("Compare against");
    expect(screen.queryByText("No data for that bracket")).toBeNull();
  });
});
