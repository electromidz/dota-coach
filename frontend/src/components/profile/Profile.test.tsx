import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiError } from "@/lib/api";
import type { MatchView, RoleSelectionResponse, StatsResponse } from "@/lib/types";

import { PlayerSnapshot } from "./PlayerSnapshot";
import { PreferencesCard } from "./PreferencesCard";

const selectCoachingRole = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, selectCoachingRole };
});

afterEach(() => {
  cleanup();
  selectCoachingRole.mockReset();
});

function roles(selected: "carry" | "mid"): RoleSelectionResponse {
  return {
    profile: {
      id: "p",
      selected_role: selected,
      selected_role_label: selected === "carry" ? "Carry" : "Mid",
      recommended_role: null,
      recommended_role_label: null,
      overrode_recommendation: false,
      analyzed_matches: 20,
      selected_at: "2026-11-01T00:00:00Z",
      updated_at: "2026-11-01T00:00:00Z",
    },
    selectable_roles: [
      { role: "carry", label: "Carry", position: 1 },
      { role: "mid", label: "Mid", position: 2 },
    ],
  } as unknown as RoleSelectionResponse;
}

describe("PreferencesCard", () => {
  it("moves the role at once and steps back when the server refuses", async () => {
    let reject: (e: unknown) => void = () => undefined;
    selectCoachingRole.mockReturnValue(new Promise((_, r) => (reject = r)));

    render(<PreferencesCard roles={roles("carry")} onRolesChange={() => undefined} />);
    const select = screen.getByLabelText("Default role") as HTMLSelectElement;
    expect(select.value).toBe("carry");

    fireEvent.change(select, { target: { value: "mid" } });
    expect(select.value).toBe("mid");

    reject(new ApiError("SERVER", "Nope.", 500));
    await waitFor(() => expect(select.value).toBe("carry"));
    expect(screen.getByRole("status").textContent).toBe("Nope.");
  });

  it("hands a saved role back to the page", async () => {
    selectCoachingRole.mockResolvedValue(roles("mid"));
    const onRolesChange = vi.fn();

    render(<PreferencesCard roles={roles("carry")} onRolesChange={onRolesChange} />);
    fireEvent.change(screen.getByLabelText("Default role"), { target: { value: "mid" } });

    await waitFor(() => expect(onRolesChange).toHaveBeenCalledWith(roles("mid")));
  });
});

function stats(matches: number): StatsResponse {
  return {
    overall: { matches, win_rate: matches ? 0.55 : null },
    eligibility: { total_matches: matches, eligible_matches: matches, excluded: [] },
    trends: { window: 20, win_rate: { value: 60 } },
    heroes: [
      { hero_id: 1, hero_name: "Anti-Mage", matches: 12, wins: 7, win_rate: 0.583 },
    ],
    role_analysis: {
      roles: [{ role: "carry", role_label: "Carry", matches: 15 }],
    },
  } as unknown as StatsResponse;
}

function match(id: string, won: boolean): MatchView {
  return {
    id,
    won,
    hero_name: "Anti-Mage",
    started_at: "2026-11-01T00:00:00Z",
  } as MatchView;
}

describe("PlayerSnapshot", () => {
  it("offers a sync instead of empty figures when nothing is eligible", () => {
    const onSync = vi.fn();
    render(
      <PlayerSnapshot
        stats={stats(0)}
        recent={[]}
        coachingRoleLabel={null}
        error={null}
        onSync={onSync}
        syncing={false}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /sync matches/i }));
    expect(onSync).toHaveBeenCalled();
  });

  it("explains stored-but-ineligible games instead of offering a sync", () => {
    const turbo = {
      ...stats(0),
      eligibility: {
        total_matches: 20,
        eligible_matches: 0,
        excluded: [{ reason: "turbo", label: "Turbo", description: "", matches: 20 }],
      },
    } as unknown as StatsResponse;
    render(
      <PlayerSnapshot
        stats={turbo}
        recent={[]}
        coachingRoleLabel={null}
        error={null}
        onSync={() => undefined}
        syncing={false}
      />,
    );

    expect(screen.getByText(/None of your 20 stored matches are Ranked/)).toBeDefined();
    expect(screen.queryByRole("button", { name: /sync/i })).toBeNull();
  });

  it("shows the server's figures and links each result to its match", () => {
    render(
      <PlayerSnapshot
        stats={stats(30)}
        recent={[match("newest", true), match("older", false)]}
        coachingRoleLabel="Carry"
        error={null}
        onSync={() => undefined}
        syncing={false}
      />,
    );

    expect(screen.getByText("60%")).toBeDefined();
    expect(screen.getByText(/55% across all 30 eligible matches/)).toBeDefined();
    expect(screen.getByText(/12 games · 58% WR/)).toBeDefined();

    const results = screen.getAllByRole("link", { name: /on Anti-Mage/ });
    // Oldest first, so the newest match is the last mark.
    expect(results.map((a) => a.getAttribute("href"))).toEqual([
      "/matches/older",
      "/matches/newest",
    ]);
  });
});
