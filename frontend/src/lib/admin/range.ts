/**
 * The dashboard's date range: a preset or a custom pair of days, always
 * resolved to concrete `YYYY-MM-DD` bounds so the date inputs can show what
 * is actually being measured. Days are UTC, matching the backend's daily
 * series.
 */

export type RangePreset = "7d" | "30d" | "90d" | "all";

export const RANGE_PRESETS: { value: RangePreset; label: string; days: number | null }[] = [
  { value: "7d", label: "7d", days: 7 },
  { value: "30d", label: "30d", days: 30 },
  { value: "90d", label: "90d", days: 90 },
  { value: "all", label: "All", days: null },
];

export const DEFAULT_PRESET: RangePreset = "30d";

/**
 * Where "All" starts. The API has no "since the first account" bound, and the
 * daily series is one point per day, so an open-ended start would mean decades
 * of empty days. This predates every account in the product.
 */
export const ALL_TIME_START = "2025-01-01";

export interface DateRange {
  preset: RangePreset | "custom";
  from: string;
  to: string;
}

const DAY = /^\d{4}-\d{2}-\d{2}$/;

export function isoDay(date: Date): string {
  return date.toISOString().slice(0, 10);
}

function addDays(day: string, n: number): string {
  const d = new Date(`${day}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + n);
  return isoDay(d);
}

/** From the URL: `?from=&to=` wins as a custom range; otherwise `?range=`. */
export function resolveRange(params: URLSearchParams, today: Date = new Date()): DateRange {
  const todayDay = isoDay(today);
  const from = params.get("from");
  const to = params.get("to");

  if ((from && DAY.test(from)) || (to && DAY.test(to))) {
    const end = to && DAY.test(to) ? to : todayDay;
    const start = from && DAY.test(from) ? from : addDays(end, -29);
    return start <= end
      ? { preset: "custom", from: start, to: end }
      : { preset: "custom", from: end, to: start };
  }

  const preset = RANGE_PRESETS.find((p) => p.value === params.get("range"))?.value ?? DEFAULT_PRESET;
  return presetRange(preset, today);
}

export function presetRange(preset: RangePreset, today: Date = new Date()): DateRange {
  const to = isoDay(today);
  const days = RANGE_PRESETS.find((p) => p.value === preset)?.days ?? null;
  return { preset, from: days === null ? ALL_TIME_START : addDays(to, -(days - 1)), to };
}

/** Whole days: the start of `from` to the last instant of `to`. */
export function rangeToApi(range: DateRange): { from: string; to: string } {
  return { from: `${range.from}T00:00:00Z`, to: `${range.to}T23:59:59Z` };
}

/** What to write back to the URL. The default preset leaves it clean. */
export function rangeToParams(range: DateRange): Record<string, string | null> {
  if (range.preset === "custom") return { range: null, from: range.from, to: range.to };
  return {
    range: range.preset === DEFAULT_PRESET ? null : range.preset,
    from: null,
    to: null,
  };
}

/** "4 Oct – 2 Nov 2026" */
export function rangeLabel(range: { from: string; to: string }): string {
  const fmt = (day: string, year: boolean) =>
    new Date(`${day}T00:00:00Z`).toLocaleDateString("en-GB", {
      day: "numeric",
      month: "short",
      ...(year ? { year: "numeric" } : {}),
      timeZone: "UTC",
    });
  const sameYear = range.from.slice(0, 4) === range.to.slice(0, 4);
  return `${fmt(range.from, !sameYear)} – ${fmt(range.to, true)}`;
}
