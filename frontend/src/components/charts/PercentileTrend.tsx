import type { TrendPoint } from "@/lib/types";
import { cn } from "@/lib/utils";

const W = 320;
const H = 72;
const PAD_X = 8;
const PAD_Y = 10;

/**
 * Standing across recent games on one hero, oldest to newest.
 *
 * Deliberately **not** `Sparkline`, which autoscales to its own min and max.
 * That is right for a series with no natural range, and wrong for percentiles:
 * a run of 48, 50, 52 would fill the frame and read as a climb, when it is
 * three identical games. This one is pinned to 0-100 with the midpoint drawn,
 * so the shape of the line is the shape of the change.
 *
 * Wins and losses are marked by fill, and the match being viewed carries a
 * ring — the reader is usually asking "where does the game I just opened sit
 * in this run?".
 *
 * Input arrives newest first, the order the API and the match list use; it is
 * reversed here so time reads left to right.
 */
export function PercentileTrend({
  points,
  className,
}: {
  points: TrendPoint[];
  className?: string;
}) {
  if (points.length < 2) {
    return (
      <p className={cn("text-sm text-ink-faint", className)}>
        Not enough games on this hero yet to plot a trend.
      </p>
    );
  }

  const series = [...points].reverse();

  const x = (i: number) => PAD_X + (i / (series.length - 1)) * (W - PAD_X * 2);
  // 0 at the bottom, 100 at the top, always — that is the whole point.
  const y = (standing: number) => H - PAD_Y - (standing / 100) * (H - PAD_Y * 2);

  const line = series
    .map((p, i) => `${x(i).toFixed(1)},${y(p.standing).toFixed(1)}`)
    .join(" ");
  const midpoint = y(50);

  return (
    <figure className={cn("m-0", className)}>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="h-20 w-full overflow-visible"
        role="img"
        aria-label={describe(series)}
      >
        {/* The midpoint, which is the reference the whole chart hangs on. */}
        <line
          x1={PAD_X}
          x2={W - PAD_X}
          y1={midpoint}
          y2={midpoint}
          stroke="var(--color-ink-faint)"
          strokeOpacity={0.4}
          strokeDasharray="3 3"
          strokeWidth={1}
        />

        <polyline
          points={line}
          fill="none"
          stroke="var(--color-mark-line)"
          strokeWidth={2}
          strokeLinecap="round"
          strokeLinejoin="round"
        />

        {series.map((p, i) => (
          <circle
            key={p.match_id}
            cx={x(i)}
            cy={y(p.standing)}
            // The viewed match is larger as well as ringed: size survives a
            // screenshot at a glance where a ring alone does not.
            r={p.is_current ? 5 : 3.5}
            fill={
              p.won ? "var(--color-mark-win)" : "var(--color-mark-loss)"
            }
            stroke="var(--color-surface-2)"
            strokeWidth={p.is_current ? 2.5 : 1.5}
          >
            <title>
              {`${p.won ? "Win" : "Loss"} · p${Math.round(p.standing)}${
                p.is_current ? " · this match" : ""
              }`}
            </title>
          </circle>
        ))}
      </svg>

      <figcaption className="mt-1 flex items-center justify-between text-[0.6875rem] text-ink-faint">
        <span>oldest</span>
        <span className="flex items-center gap-3">
          <span className="inline-flex items-center gap-1.5">
            <span
              aria-hidden
              className="size-2 rounded-full bg-mark-win"
            />
            win
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span
              aria-hidden
              className="size-2 rounded-full bg-mark-loss"
            />
            loss
          </span>
        </span>
        <span>newest</span>
      </figcaption>
    </figure>
  );
}

function describe(series: TrendPoint[]): string {
  const first = series[0];
  const last = series[series.length - 1];
  const wins = series.filter((p) => p.won).length;

  return (
    `Standing across ${series.length} games on this hero, oldest to newest: ` +
    `from p${Math.round(first.standing)} to p${Math.round(last.standing)}, ` +
    `${wins} won.`
  );
}
