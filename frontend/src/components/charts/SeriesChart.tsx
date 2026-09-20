import type { MetricSeries, MetricUnit } from "@/lib/types";
import { cn } from "@/lib/utils";

const W = 320;
const H = 72;
const PAD_X = 8;
const PAD_Y = 10;

/**
 * One metric across coaching sessions, oldest to newest.
 *
 * The axis is chosen by the metric's **unit**, which is the same distinction
 * the progress engine makes server-side: a bounded scale is plotted against
 * its own range, an unbounded one against the data.
 *
 * That matters more than it sounds. A performance score autoscaled to its own
 * min and max turns 54 → 57 → 61 into a dramatic climb filling the frame; on a
 * fixed 0-100 axis it reads as the modest real improvement it is. Conversely a
 * gold-per-minute series has no ceiling to be a fraction of, so a fixed axis
 * would flatten it into a straight line.
 *
 * Deliberately not `Sparkline`, which always autoscales, and not
 * `PercentileTrend`, which is always 0-100. This one asks.
 */
export function SeriesChart({
  series,
  className,
}: {
  series: MetricSeries;
  className?: string;
}) {
  const values = series.points.map((p) => p.value);

  if (values.length < 2) {
    return (
      <p className={cn("text-sm text-ink-faint", className)}>
        One session so far. A second gives this a shape.
      </p>
    );
  }

  const [min, max] = range(series.unit, values);
  const span = max - min || 1;

  const x = (i: number) => PAD_X + (i / (values.length - 1)) * (W - PAD_X * 2);
  const y = (v: number) => H - PAD_Y - ((v - min) / span) * (H - PAD_Y * 2);

  const line = series.points
    .map((p, i) => `${x(i).toFixed(1)},${y(p.value).toFixed(1)}`)
    .join(" ");

  const first = values[0];
  const last = values[values.length - 1];
  // Direction-corrected: for deaths, falling is the good direction.
  const better = series.higher_is_better ? last > first : last < first;
  const moved = Math.abs(last - first) > Number.EPSILON;
  const stroke = !moved
    ? "var(--color-mark-line)"
    : better
      ? "var(--color-mark-win)"
      : "var(--color-mark-loss)";

  // Only meaningful on a bounded scale, where the midpoint is a real
  // reference rather than an artefact of the data's own range.
  const midpoint = bounded(series.unit) ? y((min + max) / 2) : null;

  return (
    <figure className={cn("m-0", className)}>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="h-[72px] w-full overflow-visible"
        role="img"
        aria-label={describe(series)}
      >
        {midpoint !== null ? (
          <line
            x1={PAD_X}
            x2={W - PAD_X}
            y1={midpoint}
            y2={midpoint}
            stroke="var(--color-ink-faint)"
            strokeOpacity={0.35}
            strokeDasharray="3 3"
            strokeWidth={1}
          />
        ) : null}

        <polyline
          points={line}
          fill="none"
          stroke={stroke}
          strokeWidth={2}
          strokeLinecap="round"
          strokeLinejoin="round"
        />

        {series.points.map((p, i) => (
          <circle
            key={p.session_id}
            cx={x(i)}
            cy={y(p.value)}
            r={i === values.length - 1 ? 4.5 : 3}
            fill={stroke}
            stroke="var(--color-surface-2)"
            strokeWidth={1.5}
          >
            <title>{`Session ${p.sequence}: ${formatValue(p.value, series.unit)}`}</title>
          </circle>
        ))}
      </svg>

      <figcaption className="mt-1 flex items-baseline justify-between text-[0.6875rem] text-ink-faint">
        <span className="font-mono tabular-nums">
          {formatValue(first, series.unit)}
        </span>
        <span>
          {series.points.length} sessions
        </span>
        <span
          className={cn(
            "font-mono tabular-nums",
            moved && (better ? "text-string" : "text-error"),
          )}
        >
          {formatValue(last, series.unit)}
        </span>
      </figcaption>
    </figure>
  );
}

/** Whether the unit has a natural range of its own. */
function bounded(unit: MetricUnit): boolean {
  return unit === "percentile" || unit === "score" || unit === "proportion";
}

/**
 * The axis.
 *
 * A bounded unit is plotted against its full range so the reader sees where
 * the value sits, not only how it moved. Everything else gets a padded window
 * around the data, because there is no range to sit within.
 */
function range(unit: MetricUnit, values: number[]): [number, number] {
  if (unit === "percentile" || unit === "score") return [0, 100];
  if (unit === "proportion") return [0, 1];

  const min = Math.min(...values);
  const max = Math.max(...values);
  const pad = (max - min) * 0.15 || Math.abs(max) * 0.1 || 1;
  return [min - pad, max + pad];
}

export function formatValue(value: number, unit: MetricUnit): string {
  if (unit === "proportion") return `${Math.round(value * 100)}%`;
  if (unit === "percentile") return `p${Math.round(value)}`;
  if (unit === "score") return `${Math.round(value)}`;
  if (Math.abs(value) >= 100) return Math.round(value).toLocaleString();
  if (Math.abs(value) >= 10) return value.toFixed(1);
  return value.toFixed(2);
}

function describe(series: MetricSeries): string {
  const values = series.points.map((p) => p.value);
  const first = formatValue(values[0], series.unit);
  const last = formatValue(values[values.length - 1], series.unit);

  return `${series.label} across ${series.points.length} coaching sessions, oldest to newest: from ${first} to ${last}.`;
}
