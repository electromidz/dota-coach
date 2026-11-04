import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ApiError } from "@/lib/api";

import { Calibrating } from "./Calibrating";
import { RESPONSE } from "./test-fixtures";

const getCalibration = vi.hoisted(() => vi.fn());
const getRankDistribution = vi.hoisted(() => vi.fn());
const getMatches = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getCalibration, getRankDistribution, getMatches };
});

vi.mock("@/lib/session-context", () => ({
  useSession: () => ({ session: { kind: "signed-in", me: {} }, setSession: vi.fn() }),
}));

beforeEach(() => {
  getRankDistribution.mockRejectedValue(new ApiError("PROVIDER_UNAVAILABLE", "down", 502));
  getMatches.mockResolvedValue({ matches: [] });
});

afterEach(() => vi.clearAllMocks());

describe("Calibrating", () => {
  it("renders the header, both charts and the breakdown from the response", async () => {
    getCalibration.mockResolvedValue(RESPONSE);
    render(<Calibrating />);

    expect(await screen.findByText("Archon 5")).toBeDefined();
    expect(screen.getByRole("region", { name: "Rank over time" })).toBeDefined();
    expect(screen.getByRole("region", { name: "Recent momentum" })).toBeDefined();
    expect(screen.getByRole("region", { name: "Positions" })).toBeDefined();
    // The duplicated full match table is gone.
    expect(screen.queryByText("Match history")).toBeNull();
  });

  it("keeps the page when the bracket comparison is unavailable", async () => {
    getCalibration.mockResolvedValue(RESPONSE);
    render(<Calibrating />);

    expect(await screen.findByText("Archon 5")).toBeDefined();
    expect(screen.queryByRole("region", { name: "Against your bracket" })).toBeNull();
  });

  /** The 409 before a sync is a state to explain, not a failure. */
  it("explains the unsynced state instead of reporting a failure", async () => {
    getCalibration.mockRejectedValue(new ApiError("PRECONDITION_UNMET", "Sync your matches first.", 409));
    render(<Calibrating />);

    expect(await screen.findByText(/Nothing to calibrate yet/)).toBeDefined();
  });

  it("surfaces a real failure as an error", async () => {
    getCalibration.mockRejectedValue(new ApiError("INTERNAL", "Database unavailable.", 500));
    render(<Calibrating />);

    expect(await screen.findByText("Database unavailable.")).toBeDefined();
  });

  it("renders the server's numbers rather than deriving any", async () => {
    getCalibration.mockResolvedValue({
      ...RESPONSE,
      confidence: { confidence_pct: 100, matches_counted: 21, target_matches: 15, window_days: 14, is_calibrated: true },
    });
    render(<Calibrating />);

    expect(await screen.findByText("21 / 15")).toBeDefined();
    expect(screen.getByText("Calibrated")).toBeDefined();
    expect(screen.getByText("last 14 days")).toBeDefined();
  });
});
