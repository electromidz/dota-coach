import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { MatchView } from "@/lib/types";

import { MatchCard } from "./MatchCard";

function match(overrides: Partial<MatchView> = {}): MatchView {
  return {
    id: "3fa85f64-5717-4562-b3fc-2c963f66afa6",
    dota_player_id: "player",
    match_id: 7_500_000_001,
    hero_id: 35,
    hero_name: "Luna",
    role: "Carry",
    lane_role: 1,
    won: true,
    duration_seconds: 2_400,
    kills: 8,
    deaths: 4,
    assists: 12,
    gpm: 550,
    xpm: 620,
    last_hits: 300,
    denies: null,
    net_worth: null,
    hero_damage: null,
    tower_damage: null,
    hero_healing: null,
    game_mode: 22,
    lobby_type: 7,
    party_size: 1,
    started_at: new Date().toISOString(),
    detail_synced: true,
    replay_parsed: false,
    kda: 5,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    eligible: true,
    mode_label: "Ranked All Pick",
    position: "carry",
    position_label: "Carry",
    peer_gpm: 500,
    peer_xpm: 550,
    rating: 7.2,
    mmr_delta_estimate: 29,
    ...overrides,
  };
}

describe("MatchCard", () => {
  it("shows the position from the one taxonomy", () => {
    render(<MatchCard match={match({ role: "Core", position: null, position_label: "Unknown" })} />);
    expect(screen.getByText("Unknown")).toBeDefined();
    expect(screen.queryByText("Core")).toBeNull();
  });

  it("labels nothing in the default view, where every game is coached", () => {
    render(<MatchCard match={match()} mode="eligible" />);
    expect(screen.queryByText("Not coached")).toBeNull();
    expect(screen.queryByText("Ranked All Pick")).toBeNull();
  });

  it("marks only ineligible games in the Turbo and All views", () => {
    const { rerender } = render(
      <MatchCard
        match={match({ eligible: false, mode_label: "Turbo", game_mode: 23 })}
        mode="all"
      />,
    );
    expect(screen.getByText("Not coached")).toBeDefined();

    rerender(<MatchCard match={match()} mode="all" />);
    expect(screen.queryByText("Not coached")).toBeNull();
  });

  it("measures GPM against the peer median, not the list", () => {
    render(<MatchCard match={match({ gpm: 600, peer_gpm: 500 })} />);
    expect(
      screen.getByRole("img", { name: /GPM 600, peer median on Luna 500/ }),
    ).toBeDefined();
  });

  it("draws no bar where there is no peer reference", () => {
    render(<MatchCard match={match({ peer_gpm: null, peer_xpm: null })} />);
    expect(screen.queryByRole("img", { name: /peer median/ })).toBeNull();
  });
});
