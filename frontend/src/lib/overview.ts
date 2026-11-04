import { toPercentileVector, type PercentilePoint } from "@/lib/benchmark-view";
import type { BenchmarkResult, TrainingFocus } from "@/lib/types";

/**
 * The Overview radar's skill axes, one benchmark metric each. No averaging:
 * each axis is a percentile the server already computed. Vision has no data
 * source, so it is not drawn.
 */
export const SKILL_AXES: { axis: string; metric: string }[] = [
  { axis: "Farming", metric: "gold_per_min" },
  { axis: "Fighting", metric: "hero_damage_per_min" },
  { axis: "Supporting", metric: "assists_per_min" },
  { axis: "Pushing", metric: "tower_damage" },
  { axis: "Survival", metric: "deaths_per_min" },
];

/** Skill-named radar points, only for axes the server could place. */
export function skillPoints(results: BenchmarkResult[]): PercentilePoint[] {
  const byMetric = new Map(toPercentileVector(results, null).map((p) => [p.metric, p]));
  return SKILL_AXES.flatMap(({ axis, metric }) => {
    const point = byMetric.get(metric);
    return point && point.you !== null ? [{ ...point, axis }] : [];
  });
}

export type Impact = "high" | "medium" | "low";

/** The focus engine's own impact score, read as a badge. */
export function impactOf(focus: TrainingFocus): Impact | null {
  const part = focus.score_parts.find((p) => p.key === "impact");
  if (!part) return null;
  return part.score >= 67 ? "high" : part.score >= 34 ? "medium" : "low";
}
