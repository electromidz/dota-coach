import { cn } from "@/lib/utils";

const W = 480;
const H = 96;
const PAD = 8;

export interface SparklineSeries {
  key: string;
  label: string;
  /** A CSS colour, e.g. `"var(--color-function)"`. */
  color: string;
  /** Tailwind background class matching `color`, for the legend swatch. */
  swatch: string;
  /** SVG `stroke-dasharray`. Omit for a solid line — the first series
   *  usually should be, so at least one line reads unambiguously even in
   *  greyscale. */
  dashArray?: string;
}

export interface MultiSeriesDatum {
  date: string;
  [seriesKey: string]: string | number;
}

/**
 * Several related series over the same time axis and the same vertical
 * scale — signups, logins and purchases, where the comparison between them
 * is the point and three separate single-series charts would make a reader
 * hold two in their head while looking at the third.
 *
 * Unlike `Sparkline`, this carries a legend: more than one line needs one to
 * be told apart, and colour is paired with a line-style difference (solid,
 * dashed, …) so every series stays distinguishable without relying on hue
 * alone.
 */
export function MultiSparkline({
  data,
  series,
  className,
}: {
  data: MultiSeriesDatum[];
  series: SparklineSeries[];
  className?: string;
}) {
  if (data.length < 2) {
    return (
      <p className={cn("text-sm text-ink-faint", className)}>
        Not enough days in range to plot a trend.
      </p>
    );
  }

  const valueAt = (d: MultiSeriesDatum, key: string) => Number(d[key]) || 0;

  const max = Math.max(
    ...data.flatMap((d) => series.map((s) => valueAt(d, s.key))),
    1,
  );

  const x = (i: number) => PAD + (i / (data.length - 1)) * (W - PAD * 2);
  const y = (v: number) => H - PAD - (v / max) * (H - PAD * 2);

  const lineFor = (key: string) =>
    data.map((d, i) => `${x(i).toFixed(1)},${y(valueAt(d, key)).toFixed(1)}`).join(" ");

  const totalFor = (key: string) => data.reduce((sum, d) => sum + valueAt(d, key), 0);

  const summary = series
    .map((s) => `${totalFor(s.key)} total ${s.label.toLowerCase()}`)
    .join(", ");

  return (
    <figure className={cn("m-0 flex flex-col gap-2", className)}>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="h-24 w-full overflow-visible"
        role="img"
        aria-label={`${series.map((s) => s.label).join(", ")} over ${data.length} days: ${summary}.`}
      >
        {series.map((s) => (
          <polyline
            key={s.key}
            points={lineFor(s.key)}
            fill="none"
            stroke={s.color}
            strokeWidth={2}
            strokeDasharray={s.dashArray}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        ))}
      </svg>

      <figcaption className="flex flex-wrap items-center gap-4 text-xs text-ink-faint">
        {series.map((s) => (
          <Legend key={s.key} swatch={s.swatch} label={s.label} dashed={!!s.dashArray} />
        ))}
      </figcaption>
    </figure>
  );
}

function Legend({
  swatch,
  label,
  dashed = false,
}: {
  swatch: string;
  label: string;
  dashed?: boolean;
}) {
  return (
    <span className="flex items-center gap-1.5">
      <span
        aria-hidden
        className={cn("h-0.5 w-4 rounded-full", swatch, dashed && "opacity-70")}
      />
      {label}
    </span>
  );
}
