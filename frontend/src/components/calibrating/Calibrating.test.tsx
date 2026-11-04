import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiError } from "@/lib/api";
import type { CalibrationResponse } from "@/lib/types";

import { Calibrating } from "./Calibrating";

const getCalibration = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getCalibration };
});

vi.mock("@/lib/session-context", () => ({
  useSession: () => ({
    session: { kind: "signed-in", me: {} },
    setSession: vi.fn(),
  }),
}));

// The match history reads its own endpoint and the URL's page number, and has
// its own tests. Stubbed here so these stay about the calibration panels — and
// so a table failure could not be mistaken for one of them.
vi.mock("@/components/calibrating/MatchHistoryTable", () => ({
  MatchHistoryTable: () => <div data-testid="match-history" />,
}));

const RESPONSE: CalibrationResponse = {
  established_rank: {
    rank_tier: 45,
    label: "Archon 5",
    leaderboard_rank: null,
    mmr: { low: 2926, high: 3079, midpoint: 3002 },
  },
  confidence: {
    confidence_pct: 60,
    matches_counted: 12,
    target_matches: 20,
    window_days: 30,
    is_calibrated: false,
  },
  trajectory: [
    { rank_tier: 44, label: "Archon 4", at: "2026-09-15T12:00:00Z", estimated: false, gap_before: false },
    { rank_tier: 45, label: "Archon 5", at: "2026-09-18T12:00:00Z", estimated: true, gap_before: false },
    { rank_tier: 45, label: "Archon 5", at: "2026-09-22T12:00:00Z", estimated: false, gap_before: false },
  ],
  streak: { count: 3, kind: "win" },
  momentum: {
    points: [
      { index: 1, match_id: 1, hero_name: "Luna", won: true, delta: 30, cumulative: 30, started_at: "2026-09-20T12:00:00Z" },
      { index: 2, match_id: 2, hero_name: "Luna", won: false, delta: -25, cumulative: 5, started_at: "2026-09-21T12:00:00Z" },
      { index: 3, match_id: 3, hero_name: "Luna", won: true, delta: 30, cumulative: 35, started_at: "2026-09-22T12:00:00Z" },
    ],
    net: 35,
    wins: 2,
    losses: 1,
    window: 20,
  },
  role_preference: [
    { role: "Carry", pct: 75, matches: 9 },
    { role: "Mid", pct: 25, matches: 3 },
  ],
  methodology: {
    win_base_mmr: 30,
    loss_base_mmr: 25,
    confidence_window_days: 30,
    confidence_target_matches: 20,
  },
  calibration_version: 1,
};

afterEach(() => {
  getCalibration.mockReset();
});

describe("Calibrating", () => {
  it("renders rank, confidence, streak and roles from the response", async () => {
    getCalibration.mockResolvedValue(RESPONSE);

    render(<Calibrating />);

    expect(await screen.findByText("Archon 5")).toBeDefined();
    expect(screen.getByText(/12 \/ 20/)).toBeDefined();
    expect(screen.getByText("Win streak")).toBeDefined();
    expect(screen.getByText("Carry")).toBeDefined();
    expect(screen.getByText("Mid")).toBeDefined();
  });

  /**
   * The 409 the API answers before anything is synced is a state to explain,
   * not a failure to apologise for — and it must not read as "something broke".
   */
  it("explains the unsynced state instead of reporting a failure", async () => {
    getCalibration.mockRejectedValue(
      new ApiError("PRECONDITION_UNMET", "Sync your matches first.", 409),
    );

    render(<Calibrating />);

    expect(await screen.findByText(/Nothing to calibrate yet/)).toBeDefined();
    expect(screen.queryByText(/Could not load/)).toBeNull();
  });

  it("surfaces a real failure as an error", async () => {
    getCalibration.mockRejectedValue(
      new ApiError("INTERNAL", "Database unavailable.", 500),
    );

    render(<Calibrating />);

    expect(await screen.findByText("Database unavailable.")).toBeDefined();
  });

  /**
   * Every figure on this screen is computed server-side. A component deriving
   * its own percentage or streak would be the failure the whole feature is
   * arranged to prevent, so the numbers rendered are exactly the ones sent.
   */
  it("renders the server's numbers rather than deriving any", async () => {
    getCalibration.mockResolvedValue({
      ...RESPONSE,
      confidence: {
        confidence_pct: 100,
        matches_counted: 21,
        target_matches: 15,
        window_days: 14,
        is_calibrated: true,
      },
    });

    render(<Calibrating />);

    // The server's target and window, not the usual 20 and 30.
    expect(await screen.findByText(/21 \/ 15/)).toBeDefined();
    expect(screen.getByText("Calibrated")).toBeDefined();
    await waitFor(() =>
      expect(screen.getByText("Ranked games in the last 14 days")).toBeDefined(),
    );
  });
});
