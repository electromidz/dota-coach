import type { FocusMeasure, ProgressSeries } from "./types";

/**
 * Presentation helpers for the training focus.
 *
 * As everywhere else in `lib`, no arithmetic on match data: the baseline,
 * target, current value and the whole progress series are computed in Rust.
 * What remains is deciding whether a number is a rate, a count or a decimal.
 */

/** Measures whose values are shares rather than counts. */
const RATE_MEASURES: FocusMeasure[] = ["kill_participation", "pattern_rate"];

/** `0.42` -> `"42%"`, `2.44` -> `"2.4"`, `512.6` -> `"513"`. */
export function formatFocusValue(
  measure: FocusMeasure,
  value: number | null,
): string {
  if (value === null) return "—";
  if (RATE_MEASURES.includes(measure)) return `${Math.round(value * 100)}%`;
  if (measure === "deaths_per_10") return value.toFixed(1);
  return Math.round(value).toString();
}

/**
 * Whether the series is moving the right way.
 *
 * Compares the first plotted bucket with the last, honouring direction, so
 * "improving" means the same thing for deaths as for gold per minute.
 * `null` when there is nothing to compare.
 */
export function trendDirection(
  series: ProgressSeries | null,
): "improving" | "worsening" | "flat" | null {
  if (!series || series.points.length < 2) return null;

  const first = series.points[0].value;
  const last = series.points[series.points.length - 1].value;
  const change = series.higher_is_better ? last - first : first - last;

  // A hair either way is noise, not a trend. 2% of the starting value.
  const threshold = Math.abs(first) * 0.02;
  if (Math.abs(change) <= threshold) return "flat";
  return change > 0 ? "improving" : "worsening";
}
