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
    ...overrides,
  };
}

describe("MatchCard", () => {
  it("names the mode every match was played in", () => {
    render(<MatchCard match={match()} />);

    expect(screen.getByText("Ranked All Pick")).toBeDefined();
  });

  /**
   * The reconciliation the whole label exists for: a player looking at Turbo
   * games in their history and a dashboard that counts fewer matches needs
   * something connecting the two.
   */
  it("marks a match the coach does not read", () => {
    render(
      <MatchCard
        match={match({ eligible: false, mode_label: "Turbo", game_mode: 23 })}
      />,
    );

    expect(screen.getByText("Turbo")).toBeDefined();
    expect(screen.getByText("Not coached")).toBeDefined();
  });

  it("does not label an eligible match as excluded", () => {
    render(<MatchCard match={match()} />);

    expect(screen.queryByText("Not coached")).toBeNull();
  });
});
