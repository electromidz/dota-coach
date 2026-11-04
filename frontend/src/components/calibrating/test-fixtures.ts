import type { CalibrationResponse, MomentumPoint } from "@/lib/types";

/** Test data only. One ranked game, oldest first by `index`. */
export function momentumPoint(index: number, won: boolean, overrides: Partial<MomentumPoint> = {}): MomentumPoint {
  const delta = won ? 30 : -25;
  return {
    index,
    match_id: 1000 + index,
    hero_name: "Luna",
    won,
    delta,
    cumulative: 0,
    started_at: `2026-09-${String(index).padStart(2, "0")}T12:00:00Z`,
    id: `match-${index}`,
    hero_id: 48,
    kills: 8,
    deaths: 3,
    assists: 10,
    rating: 6.5,
    ...overrides,
  };
}

/** `results` oldest first; cumulative filled in. */
export function momentumPoints(results: boolean[]): MomentumPoint[] {
  let running = 0;
  return results.map((won, i) => {
    const p = momentumPoint(i + 1, won);
    running += p.delta;
    return { ...p, cumulative: running };
  });
}

export const RESPONSE: CalibrationResponse = {
  established_rank: {
    rank_tier: 45,
    label: "Archon 5",
    leaderboard_rank: null,
    mmr: { low: 2926, high: 3079, midpoint: 3002, medal_low: 2310, medal_high: 3079 },
    next_medal: { label: "Legend", mmr_needed: 78, wins_needed: 3 },
  },
  confidence: { confidence_pct: 60, matches_counted: 12, target_matches: 20, window_days: 30, is_calibrated: false },
  trajectory: [
    { rank_tier: 44, label: "Archon 4", at: "2026-09-15T12:00:00Z", estimated: false, gap_before: false },
    { rank_tier: 45, label: "Archon 5", at: "2026-09-18T12:00:00Z", estimated: true, gap_before: false },
    { rank_tier: 45, label: "Archon 5", at: "2026-09-22T12:00:00Z", estimated: false, gap_before: false },
  ],
  streak: { count: 3, kind: "win" },
  momentum: {
    points: momentumPoints([true, false, true, true, true, false]),
    net: 70,
    wins: 4,
    losses: 2,
    window: 20,
    avg_win_delta: 30,
    avg_loss_delta: -25,
  },
  role_preference: [
    { role: "Carry", position: "carry", pct: 75, matches: 9 },
    { role: "Mid", position: "mid", pct: 25, matches: 3 },
  ],
  hero_win_rates: [{ hero_id: 48, hero_name: "Luna", matches: 9, wins: 6, win_rate: 66.7 }],
  methodology: { win_base_mmr: 30, loss_base_mmr: 25, confidence_window_days: 30, confidence_target_matches: 20 },
  calibration_version: 1,
};
