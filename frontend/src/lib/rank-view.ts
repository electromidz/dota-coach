import type { TrajectoryPoint } from "@/lib/types";

/** A run of consecutive identical results, by index (inclusive). */
export interface Run {
  start: number;
  end: number;
  won: boolean;
}

/** Runs of `min` or more wins or losses in a row, for streak bands. */
export function streakRuns(results: boolean[], min = 3): Run[] {
  const runs: Run[] = [];
  let start = 0;
  for (let i = 1; i <= results.length; i++) {
    if (i === results.length || results[i] !== results[start]) {
      if (i - start >= min) runs.push({ start, end: i - 1, won: results[start] });
      start = i;
    }
  }
  return runs;
}

/** The medal number (1 Herald … 8 Immortal) a `rank_tier` belongs to. */
export function medalOf(tier: number): number {
  return Math.floor(tier / 10);
}

/** Indices where the medal differs from the previous point. */
export function medalChanges(points: TrajectoryPoint[]): number[] {
  return points.flatMap((p, i) => (i > 0 && medalOf(p.rank_tier) !== medalOf(points[i - 1].rank_tier) ? [i] : []));
}

/** Points inside the last `days` days; `null` keeps every point. */
export function withinDays(points: TrajectoryPoint[], days: number | null, now: Date = new Date()): TrajectoryPoint[] {
  if (days === null) return points;
  const from = now.getTime() - days * 86_400_000;
  return points.filter((p) => new Date(p.at).getTime() >= from);
}

/** Four or five round axis ticks covering [lo, hi], always including 0
 *  when the range crosses it. */
export function niceTicks(lo: number, hi: number, count = 4): number[] {
  const span = hi - lo || Math.abs(hi) || 1;
  const raw = span / count;
  const magnitude = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * magnitude).find((s) => s >= raw) ?? raw;
  const first = Math.floor(lo / step) * step;
  const ticks: number[] = [];
  for (let v = first; v <= hi + step * 0.5; v += step) ticks.push(Math.round(v * 1000) / 1000);
  return ticks;
}
