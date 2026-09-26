import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ApiError } from "@/lib/api";
import type { MatchListResponse, MatchView } from "@/lib/types";

import { MatchHistoryTable } from "./MatchHistoryTable";

const getMatches = vi.hoisted(() => vi.fn());
const syncMatches = vi.hoisted(() => vi.fn());
const replace = vi.hoisted(() => vi.fn());
const searchParams = vi.hoisted(() => ({ value: new URLSearchParams() }));

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getMatches, syncMatches };
});

vi.mock("next/navigation", () => ({
  useRouter: () => ({ replace }),
  useSearchParams: () => searchParams.value,
}));

function match(overrides: Partial<MatchView> = {}): MatchView {
  return {
    id: "match-1",
    dota_player_id: "player-1",
    match_id: 7_500_000_001,
    hero_id: 35,
    hero_name: "Luna",
    role: "Carry",
    lane_role: 1,
    won: true,
    duration_seconds: 2_445,
    kills: 10,
    deaths: 3,
    assists: 8,
    gpm: 620,
    xpm: 700,
    last_hits: 250,
    denies: 12,
    net_worth: 24_000,
    hero_damage: 30_000,
    tower_damage: 5_000,
    hero_healing: 0,
    game_mode: 22,
    lobby_type: 7,
    party_size: 1,
    started_at: "2026-09-24T12:00:00Z",
    detail_synced: true,
    replay_parsed: true,
    kda: 6,
    created_at: "2026-09-24T12:00:00Z",
    updated_at: "2026-09-24T12:00:00Z",
    eligible: true,
    mode_label: "Ranked All Pick",
    rating: 7.4,
    mmr_delta_estimate: 31,
    ...overrides,
  };
}

function page(overrides: Partial<MatchListResponse> = {}): MatchListResponse {
  return {
    matches: [match()],
    page: 1,
    limit: 20,
    total: 43,
    total_pages: 3,
    scope: "all",
    filtered: false,
    sort: "newest",
    mode: "all",
    filters: { heroes: [], roles: [] },
    lifetime_games: 43,
    last_synced_at: "2026-09-24T12:30:00Z",
    syncing: false,
    ...overrides,
  };
}

beforeEach(() => {
  searchParams.value = new URLSearchParams();
  getMatches.mockResolvedValue(page());
  syncMatches.mockResolvedValue({});
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

/** What the component asked the backend for, on its most recent call. */
function lastCall() {
  return getMatches.mock.calls[getMatches.mock.calls.length - 1];
}

describe("MatchHistoryTable", () => {
  it("counts the games on screen out of the whole filtered list", async () => {
    render(<MatchHistoryTable />);

    expect(
      await screen.findByText("Displaying games 1–20 of 43 valid matches"),
    ).toBeTruthy();
  });

  it("renders the server's figures rather than recomputing them", async () => {
    render(<MatchHistoryTable />);

    expect(await screen.findByText("Luna")).toBeTruthy();
    expect(screen.getByText("Win")).toBeTruthy();
    expect(screen.getByText("620")).toBeTruthy();
    expect(screen.getByText("700")).toBeTruthy();
    expect(screen.getByText("40:45")).toBeTruthy();
    expect(screen.getByText("7.4")).toBeTruthy();
    expect(screen.getByText("Sep 24, 26")).toBeTruthy();
  });

  /// The honesty requirement, as a test: a bare `+31` would read as Valve's own
  /// number, and Valve publishes no per-match MMR.
  it("labels the mmr movement as an estimate", async () => {
    render(<MatchHistoryTable />);

    expect(await screen.findByText("+31")).toBeTruthy();
    expect(screen.getByText("est.")).toBeTruthy();
  });

  it("shows no movement at all for a game that cannot move a medal", async () => {
    getMatches.mockResolvedValue(
      page({
        matches: [
          match({ game_mode: 23, lobby_type: 0, mmr_delta_estimate: null }),
        ],
      }),
    );
    render(<MatchHistoryTable />);

    expect(await screen.findByText("Turbo")).toBeTruthy();
    expect(screen.getByText("—")).toBeTruthy();
    expect(screen.queryByText("est.")).toBeNull();
  });

  it("asks the backend for the mode rather than filtering rows here", async () => {
    render(<MatchHistoryTable />);
    await screen.findByText("Luna");

    fireEvent.click(screen.getByText("Turbo"));
    await waitFor(() => expect(lastCall()[3]).toEqual({ mode: "turbo" }));

    fireEvent.click(screen.getByText("Ranked"));
    await waitFor(() => expect(lastCall()[3]).toEqual({ mode: "ranked" }));
  });

  it("reads its page from the url and writes the page it moves to", async () => {
    searchParams.value = new URLSearchParams("page=2");
    render(<MatchHistoryTable />);

    await waitFor(() => expect(lastCall()[0]).toBe(2));

    fireEvent.click(await screen.findByLabelText("Page 3"));
    expect(replace).toHaveBeenCalledWith("?page=3", { scroll: false });
  });

  it("drops the page parameter rather than writing page=1", async () => {
    searchParams.value = new URLSearchParams("page=3");
    render(<MatchHistoryTable />);
    await screen.findByText("Luna");

    fireEvent.click(screen.getByLabelText("Page 1"));
    expect(replace).toHaveBeenCalledWith("?", { scroll: false });
  });

  it("ignores a page number that is not one", async () => {
    searchParams.value = new URLSearchParams("page=banana");
    render(<MatchHistoryTable />);

    await waitFor(() => expect(lastCall()[0]).toBe(1));
  });

  it("refreshes and then re-reads the page", async () => {
    render(<MatchHistoryTable />);
    await screen.findByText("Luna");
    const before = getMatches.mock.calls.length;

    fireEvent.click(screen.getByText("Refresh"));

    await waitFor(() => expect(syncMatches).toHaveBeenCalled());
    await waitFor(() =>
      expect(getMatches.mock.calls.length).toBeGreaterThan(before),
    );
  });

  /// A refused refresh is a message, not a broken screen: the backend says how
  /// long to wait, and the table it sits above still works.
  it("keeps the table when a refresh is rate limited", async () => {
    syncMatches.mockRejectedValue(
      new ApiError("RATE_LIMITED", "Already synced recently. Try again in 12s.", 429),
    );
    render(<MatchHistoryTable />);
    await screen.findByText("Luna");

    fireEvent.click(screen.getByText("Refresh"));

    expect(
      await screen.findByText("Already synced recently. Try again in 12s."),
    ).toBeTruthy();
    expect(screen.getByText("Luna")).toBeTruthy();
  });

  it("looks again once after the server says it is syncing, and only once", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    getMatches.mockResolvedValue(page({ syncing: true }));
    render(<MatchHistoryTable />);

    await waitFor(() => expect(screen.getByText("Luna")).toBeTruthy());
    const before = getMatches.mock.calls.length;

    await act(() => vi.advanceTimersByTimeAsync(5_000));
    expect(getMatches.mock.calls.length).toBe(before + 1);

    // Still `syncing: true`, so a naive effect would poll forever.
    await act(() => vi.advanceTimersByTimeAsync(20_000));
    expect(getMatches.mock.calls.length).toBe(before + 1);
  });

  it("explains an empty history rather than showing an empty table", async () => {
    getMatches.mockResolvedValue(
      page({ matches: [], total: 0, total_pages: 0, lifetime_games: 0 }),
    );
    render(<MatchHistoryTable />);

    expect(await screen.findByText("No matches to display")).toBeTruthy();
    expect(screen.getByText(/No matches stored yet/)).toBeTruthy();
  });

  /// The one failure the player can fix themselves, so it names the setting.
  it("tells a private profile which dota setting to turn on", async () => {
    getMatches.mockRejectedValue(
      new ApiError("UPSTREAM_UNAVAILABLE", "Provider failed.", 502),
    );
    render(<MatchHistoryTable />);

    expect(await screen.findByText("Expose Public Match Data")).toBeTruthy();
  });
});
