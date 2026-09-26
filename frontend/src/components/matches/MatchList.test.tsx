import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { MatchListResponse, MatchView } from "@/lib/types";

import { MatchList } from "./MatchList";

const getMatches = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getMatches };
});

vi.mock("@/lib/session-context", () => ({
  useSession: () => ({
    session: { kind: "signed-in", me: {} },
    setSession: vi.fn(),
  }),
}));

function match(overrides: Partial<MatchView> = {}): MatchView {
  return {
    id: "match-1",
    dota_player_id: "player-1",
    match_id: 1,
    hero_id: 35,
    hero_name: "Luna",
    role: "Carry",
    lane_role: 1,
    won: true,
    duration_seconds: 2400,
    kills: 10,
    deaths: 2,
    assists: 8,
    gpm: 620,
    xpm: 700,
    last_hits: 250,
    denies: 12,
    net_worth: 24000,
    hero_damage: 30000,
    tower_damage: 5000,
    hero_healing: 0,
    game_mode: 22,
    lobby_type: 7,
    party_size: 1,
    started_at: "2026-01-01T12:00:00Z",
    detail_synced: true,
    replay_parsed: true,
    kda: 9,
    created_at: "2026-01-01T12:00:00Z",
    updated_at: "2026-01-01T12:00:00Z",
    eligible: true,
    mode_label: "Ranked All Pick",
    rating: 7.2,
    mmr_delta_estimate: 29,
    ...overrides,
  };
}

function page(overrides: Partial<MatchListResponse> = {}): MatchListResponse {
  return {
    matches: [match()],
    page: 1,
    limit: 20,
    total: 1,
    total_pages: 1,
    scope: "all",
    filtered: false,
    sort: "newest",
    mode: "all",
    lifetime_games: 1,
    last_synced_at: "2026-01-01T12:05:00Z",
    syncing: false,
    filters: {
      heroes: [
        { value: "35", label: "Luna", matches: 6 },
        { value: "26", label: "Lion", matches: 2 },
      ],
      roles: [
        { value: "carry", label: "Carry", matches: 5 },
        { value: "mid", label: "Mid", matches: 3 },
      ],
    },
    ...overrides,
  };
}

beforeEach(() => {
  getMatches.mockResolvedValue(page());
  window.scrollTo = vi.fn();
});

afterEach(() => {
  vi.clearAllMocks();
});

/** What the component asked the backend for, on its most recent call. */
function lastCall() {
  return getMatches.mock.calls[getMatches.mock.calls.length - 1];
}

describe("MatchList filters", () => {
  it("offers only heroes and roles the backend reported, with counts", async () => {
    render(<MatchList />);

    expect(await screen.findByText("Luna (6)")).toBeTruthy();
    expect(screen.getByText("Lion (2)")).toBeTruthy();
    expect(screen.getByText("Carry (5)")).toBeTruthy();
    expect(screen.getByText("All heroes")).toBeTruthy();
  });

  it("sends each filter to the backend rather than trimming the page here", async () => {
    render(<MatchList />);
    await screen.findByText("Luna (6)");

    fireEvent.change(screen.getByLabelText("Hero"), { target: { value: "35" } });
    await waitFor(() => expect(lastCall()[3].heroId).toBe(35));

    fireEvent.change(screen.getByLabelText("Role"), {
      target: { value: "mid" },
    });
    await waitFor(() => expect(lastCall()[3].role).toBe("mid"));

    fireEvent.change(screen.getByLabelText("Result"), {
      target: { value: "loss" },
    });
    await waitFor(() => expect(lastCall()[3].result).toBe("loss"));

    fireEvent.change(screen.getByLabelText("Sort"), {
      target: { value: "gpm_desc" },
    });

    // All four travel together: filters combine rather than replacing.
    await waitFor(() => {
      const [, , , params] = lastCall();
      expect(params).toEqual({
        heroId: 35,
        role: "mid",
        result: "loss",
        sort: "gpm_desc",
      });
    });
  });

  it("returns to page one when a filter changes", async () => {
    getMatches.mockResolvedValue(page({ total: 60, total_pages: 3 }));
    render(<MatchList />);
    await screen.findByText("Luna (6)");

    fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    await waitFor(() => expect(lastCall()[0]).toBe(2));

    fireEvent.change(screen.getByLabelText("Result"), {
      target: { value: "win" },
    });

    // Page two of the unfiltered list is not page two of the filtered one.
    await waitFor(() => expect(lastCall()[0]).toBe(1));
  });

  it("issues exactly one request per change", async () => {
    render(<MatchList />);
    await screen.findByText("Luna (6)");
    expect(getMatches).toHaveBeenCalledTimes(1);

    fireEvent.change(screen.getByLabelText("Hero"), { target: { value: "35" } });
    await waitFor(() => expect(getMatches).toHaveBeenCalledTimes(2));
  });

  it("explains an empty filtered list and offers a way out", async () => {
    getMatches.mockResolvedValue(
      page({ matches: [], total: 0, total_pages: 0, filtered: true }),
    );
    render(<MatchList />);
    await screen.findByText("Luna (6)");

    fireEvent.change(screen.getByLabelText("Hero"), { target: { value: "26" } });

    expect(await screen.findByText("No matches found")).toBeTruthy();
    // The filter bar stays put — an empty state that hides the controls that
    // produced it is a dead end.
    expect(screen.getByLabelText("Hero")).toBeTruthy();

    // One in the filter bar, one in the empty card. The card's is the one a
    // reader who has just hit the dead end will reach for.
    const clear = screen.getAllByRole("button", { name: "Clear filters" });
    expect(clear).toHaveLength(2);
    fireEvent.click(clear[1]);
    await waitFor(() => {
      const [, , , params] = lastCall();
      expect(params).toEqual({
        heroId: undefined,
        role: undefined,
        result: "all",
        sort: "newest",
      });
    });
  });

  it("keeps the untouched empty history message when nothing is filtered", async () => {
    getMatches.mockResolvedValue(page({ matches: [], total: 0, total_pages: 0 }));
    render(<MatchList />);

    expect(await screen.findByText(/No matches stored yet/)).toBeTruthy();
    expect(screen.queryByText("No matches found")).toBeNull();
  });
});
