/**
 * Reshapes a benchmark response for the charts.
 *
 * Nothing here computes a statistic. Every percentile, median and gap comes
 * from the server; these functions only regroup them per axis and rescale a
 * gap to a share of its median so metrics with different units fit one chart.
 */

import type {
  BenchmarkResult,
  TargetComparison,
  TargetMetric,
} from "@/lib/types";

/** Short axis names for the radar and the compact rows. */
const SHORT_LABEL: Record<string, string> = {
  gold_per_min: "GPM",
  xp_per_min: "XPM",
  last_hits_per_min: "LH/min",
  kills_per_min: "Kills/min",
  assists_per_min: "Assists/min",
  deaths_per_min: "Deaths/min",
  hero_damage_per_min: "Hero dmg/min",
  tower_damage: "Tower dmg",
};

export function shortLabel(metric: string, fallback: string): string {
  return SHORT_LABEL[metric] ?? fallback;
}

export interface PercentilePoint {
  axis: string;
  /** Full metric name, for tooltips and screen readers. */
  label: string;
  metric: string;
  /** The player's percentile in their own bracket. */
  you: number | null;
  /** The own bracket's median is p50 by definition. */
  rankMedian: number | null;
  /** The target bracket's median placed on the own bracket's scale. */
  targetMedian: number | null;
  raw: {
    you: number;
    rankMedian: number | null;
    targetMedian: number | null;
  };
  higherIsBetter: boolean;
}

/**
 * One radar axis per metric, every value on the same 0-100 percentile scale.
 *
 * Percentiles arrive direction-corrected from the server (90 means "better
 * than 90%" for deaths too), so nothing is inverted here. Inverting again
 * would flip deaths back to the wrong way round.
 */
export function toPercentileVector(
  results: BenchmarkResult[],
  target: TargetComparison | null,
): PercentilePoint[] {
  const byMetric = targetByMetric(target);

  return results.map((result) => {
    const aim = byMetric.get(result.metric);
    return {
      axis: shortLabel(result.metric, result.label),
      label: result.label,
      metric: result.metric,
      you: result.percentile,
      rankMedian: result.peer_median !== null ? 50 : null,
      targetMedian: aim?.median_percentile ?? null,
      raw: {
        you: result.player_value,
        rankMedian: result.peer_median,
        targetMedian: aim?.peer_median ?? null,
      },
      higherIsBetter: result.higher_is_better,
    };
  });
}

export function targetByMetric(
  target: TargetComparison | null,
): Map<string, TargetMetric> {
  return new Map(target?.metrics.map((m) => [m.metric, m]) ?? []);
}

export interface GapRow {
  metric: string;
  axis: string;
  label: string;
  /** Percent above (+) or below (−) the target median. Positive is good. */
  percent: number;
  cleared: boolean;
}

/**
 * The server's gap to the target median, as a share of that median, sorted
 * from furthest behind to furthest ahead.
 *
 * `gap_to_median` is signed so positive means work to do, whichever way the
 * metric runs, so flipping its sign gives "positive is good" for every row,
 * deaths included.
 */
export function gapToTarget(
  results: BenchmarkResult[],
  target: TargetComparison | null,
): GapRow[] {
  if (!target) return [];
  const byMetric = targetByMetric(target);

  const rows: GapRow[] = [];
  for (const result of results) {
    const aim = byMetric.get(result.metric);
    if (!aim || aim.gap_to_median === null || !aim.peer_median) continue;
    rows.push({
      metric: result.metric,
      axis: shortLabel(result.metric, result.label),
      label: result.label,
      percent: (-aim.gap_to_median / Math.abs(aim.peer_median)) * 100,
      cleared: aim.cleared,
    });
  }
  return rows.sort((a, b) => a.percent - b.percent);
}

/** Strongest and weakest ranked metric, by the player's own percentile. */
export function extremes(points: PercentilePoint[]): {
  strongest: PercentilePoint;
  weakest: PercentilePoint;
} | null {
  const ranked = points.filter((p) => p.you !== null);
  if (ranked.length < 2) return null;
  const sorted = [...ranked].sort((a, b) => (b.you ?? 0) - (a.you ?? 0));
  return { strongest: sorted[0], weakest: sorted[sorted.length - 1] };
}

/** Mean of the ranked percentiles, or null when none are ranked. */
export function averagePercentile(results: BenchmarkResult[]): number | null {
  const ranked = results
    .map((r) => r.percentile)
    .filter((p): p is number => p !== null);
  if (ranked.length === 0) return null;
  return ranked.reduce((sum, p) => sum + p, 0) / ranked.length;
}
