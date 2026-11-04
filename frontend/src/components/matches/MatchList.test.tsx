import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
    position: "carry",
    position_label: "Carry",
    position_source: "lane",
    peer_gpm: 500,
    peer_xpm: 550,
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
    mode_total: 1,
    hidden_turbo: 0,
    hidden_other: 0,
    filtered: false,
    sort: "newest",
    mode: "eligible",
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

/** Open the hero combobox and pick by label. */
async function pickHero(label: string) {
  fireEvent.focus(screen.getByRole("combobox", { name: "Hero" }));
  const list = await screen.findByRole("listbox", { name: "Hero" });
  fireEvent.click(within(list).getByRole("option", { name: new RegExp(`^${label}`) }));
}

describe("MatchList filters", () => {
  beforeEach(() => window.history.replaceState(null, "", "/matches"));

  it("defaults to Ranked + All Pick and says what it hides", async () => {
    getMatches.mockResolvedValue(page({ mode_total: 24, hidden_turbo: 88 }));
    render(<MatchList />);

    expect(await screen.findByText(/24 matches · Ranked \+ All Pick/)).toBeTruthy();
    expect(lastCall()[2].mode).toBe("eligible");
    fireEvent.click(screen.getByRole("button", { name: "Games not shown" }));
    expect(screen.getByText("+88 Turbo hidden")).toBeTruthy();
  });

  it("switches mode, mirrors it in the URL, and resets the hero", async () => {
    render(<MatchList />);
    await screen.findByRole("radio", { name: "Turbo" });

    await pickHero("Luna");
    await waitFor(() => expect(lastCall()[2].heroId).toBe(35));

    fireEvent.click(screen.getByRole("radio", { name: "Turbo" }));
    await waitFor(() => expect(lastCall()[2].mode).toBe("turbo"));
    expect(lastCall()[2].heroId).toBeUndefined();
    expect(window.location.search).toBe("?mode=turbo");

    fireEvent.click(screen.getByRole("radio", { name: "Ranked + All Pick" }));
    await waitFor(() => expect(lastCall()[2].mode).toBe("eligible"));
    expect(window.location.search).toBe("");
  });

  it("reads the mode from the URL on arrival", async () => {
    window.history.replaceState(null, "", "/matches?mode=all");
    render(<MatchList />);
    await waitFor(() => expect(lastCall()[2].mode).toBe("all"));
  });

  it("offers heroes in a searchable box, by games, without zero-game heroes", async () => {
    getMatches.mockResolvedValue(
      page({
        filters: {
          heroes: [
            { value: "26", label: "Lion", matches: 2 },
            { value: "35", label: "Luna", matches: 6 },
            { value: "1", label: "Anti-Mage", matches: 0 },
          ],
          roles: [{ value: "carry", label: "Carry", matches: 5 }],
        },
      }),
    );
    render(<MatchList />);
    const box = await screen.findByRole("combobox", { name: "Hero" });
    fireEvent.focus(box);
    const listed = () =>
      within(screen.getByRole("listbox", { name: "Hero" }))
        .getAllByRole("option")
        .map((o) => o.textContent);
    expect(listed()).toEqual(["All heroes", "Luna6", "Lion2"]);

    fireEvent.change(box, { target: { value: "li" } });
    expect(listed()).toEqual([
      "All heroes",
      "Lion2",
    ]);
    expect(screen.getByText("Carry (5)")).toBeTruthy();
  });

  it("sends each filter to the backend rather than trimming the page here", async () => {
    render(<MatchList />);
    await screen.findByRole("combobox", { name: "Hero" });

    await pickHero("Luna");
    await waitFor(() => expect(lastCall()[2].heroId).toBe(35));

    fireEvent.change(screen.getByLabelText("Role"), { target: { value: "mid" } });
    await waitFor(() => expect(lastCall()[2].role).toBe("mid"));

    fireEvent.change(screen.getByLabelText("Result"), { target: { value: "loss" } });
    await waitFor(() => expect(lastCall()[2].result).toBe("loss"));

    fireEvent.change(screen.getByLabelText("Sort"), { target: { value: "gpm_desc" } });

    // All travel together: filters combine rather than replacing.
    await waitFor(() => {
      expect(lastCall()[2]).toEqual({
        mode: "eligible",
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
    await screen.findByRole("combobox", { name: "Hero" });

    fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    await waitFor(() => expect(lastCall()[0]).toBe(2));

    fireEvent.change(screen.getByLabelText("Result"), { target: { value: "win" } });
    await waitFor(() => expect(lastCall()[0]).toBe(1));
  });

  it("issues exactly one request per change", async () => {
    render(<MatchList />);
    await screen.findByRole("combobox", { name: "Hero" });
    expect(getMatches).toHaveBeenCalledTimes(1);

    await pickHero("Luna");
    await waitFor(() => expect(getMatches).toHaveBeenCalledTimes(2));
  });

  it("offers a way out of an empty filtered list", async () => {
    getMatches.mockResolvedValue(
      page({ matches: [], total: 0, total_pages: 0, filtered: true }),
    );
    render(<MatchList />);
    await screen.findByRole("combobox", { name: "Hero" });
    await pickHero("Lion");

    expect(await screen.findByText("No matches for these filters")).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "Hero" })).toBeTruthy();

    const clear = screen.getAllByRole("button", { name: "Clear filters" });
    fireEvent.click(clear[clear.length - 1]);
    await waitFor(() => {
      expect(lastCall()[2]).toEqual({
        mode: "eligible",
        heroId: undefined,
        role: undefined,
        result: "all",
        sort: "newest",
      });
    });
  });

  it("offers every mode when the default view is empty", async () => {
    getMatches.mockResolvedValue(page({ matches: [], total: 0, total_pages: 0, mode_total: 0 }));
    render(<MatchList />);

    expect(await screen.findByText("No Ranked + All Pick games")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Show all games" }));
    await waitFor(() => expect(lastCall()[2].mode).toBe("all"));
  });
});
