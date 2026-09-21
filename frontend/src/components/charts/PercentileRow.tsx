import type { MetricComparison } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * One metric on a fixed 0-100 percentile axis.
 *
 * The form exists for one reason: 672 gold per minute and 402 hero damage per
 * minute cannot share a value axis, but their *percentiles* can. Stacking
 * every metric on one identical scale is what turns eight rows into a single
 * read — a column of marks sitting right of centre means "above my rank", and
 * the eye gets that before it reads a word.
 *
 * `BulletRow` is the value-scale answer to a different question ("how far to
 * the top 20% on this one metric"), and stays in use on the benchmark page.
 *
 * Two marks per row: a filled dot for this match, a hollow ring for the
 * player's average on the hero. Shape distinguishes them, not colour alone,
 * and both carry text.
 */
export function PercentileRow({ row }: { row: MetricComparison }) {
  const match = row.this_match;
  const average = row.hero_average;

  // Both readings are direction-corrected server-side, so "further right is
  // better" holds for deaths as much as for gold. Nothing is inverted here.
  const matchPercentile = match?.percentile ?? null;
  const averagePercentile = average?.percentile ?? null;

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-sm text-ink">{row.label}</span>
        <span className="flex items-baseline gap-2">
          {match ? (
            <span className="font-mono text-sm tabular-nums text-number">
              {formatValue(match.value)}
            </span>
          ) : null}
          {matchPercentile !== null ? (
            <span
              className={cn(
                "rounded px-1.5 py-0.5 font-mono text-[0.625rem] tabular-nums",
                tone(matchPercentile),
              )}
            >
              p{Math.round(matchPercentile)}
            </span>
          ) : (
            <span className="rounded bg-mark-track px-1.5 py-0.5 text-[0.625rem] text-ink-faint">
              not ranked
            </span>
          )}
        </span>
      </div>

      <div
        className="relative h-6"
        role="img"
        aria-label={describe(row, matchPercentile, averagePercentile)}
      >
        {/* The track, with the weak and strong thirds shaded. The bands are
            what make a bare dot readable without a printed axis. */}
        <div className="absolute inset-x-0 top-1/2 h-2 -translate-y-1/2 overflow-hidden rounded-full bg-mark-track">
          <div className="absolute inset-y-0 left-0 w-[30%] bg-mark-loss/25" />
          <div className="absolute inset-y-0 right-0 w-[30%] bg-mark-win/25" />
        </div>

        {/* The midpoint. Every row shares it, which is what lets the column be
            scanned vertically. */}
        <span
          aria-hidden
          className="absolute top-1/2 h-4 w-px -translate-y-1/2 bg-ink-faint/60"
          style={{ left: "50%" }}
        />

        {averagePercentile !== null ? (
          <span
            aria-hidden
            title={`Your average on this hero: p${Math.round(averagePercentile)}`}
            className="absolute top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-ink-muted bg-surface-2"
            style={{ left: `${clamp(averagePercentile)}%` }}
          />
        ) : null}

        {matchPercentile !== null ? (
          <span
            aria-hidden
            title={`This match: p${Math.round(matchPercentile)}`}
            className={cn(
              "absolute top-1/2 size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-surface-2",
              mark(matchPercentile),
            )}
            style={{ left: `${clamp(matchPercentile)}%` }}
          />
        ) : null}
      </div>
    </div>
  );
}

/** Keeps a mark inside the track when a percentile sits at either extreme. */
function clamp(percentile: number): number {
  return Math.max(2, Math.min(98, percentile));
}

function tone(percentile: number): string {
  if (percentile >= 70) return "bg-mark-win/15 text-string";
  if (percentile <= 30) return "bg-mark-loss/15 text-error";
  return "bg-mark-track text-ink-muted";
}

function mark(percentile: number): string {
  if (percentile >= 70) return "bg-mark-win";
  if (percentile <= 30) return "bg-mark-loss";
  return "bg-mark-line";
}

/**
 * The row as a sentence, for anyone who cannot see the marks. Charts here
 * never carry meaning in position alone.
 */
function describe(
  row: MetricComparison,
  matchPercentile: number | null,
  averagePercentile: number | null,
): string {
  const parts: string[] = [row.label];

  if (row.this_match) {
    parts.push(
      matchPercentile !== null
        ? `this match ${formatValue(row.this_match.value)}, better than ${Math.round(matchPercentile)}% of peers`
        : `this match ${formatValue(row.this_match.value)}, not compared`,
    );
  }
  if (row.hero_average) {
    parts.push(
      averagePercentile !== null
        ? `your average ${formatValue(row.hero_average.value)}, better than ${Math.round(averagePercentile)}% of peers`
        : `your average ${formatValue(row.hero_average.value)}`,
    );
  }
  if (row.peer_median !== null) {
    parts.push(`peer median ${formatValue(row.peer_median)}`);
  }

  return `${parts.join("; ")}.`;
}

/** Large figures read better whole; small rates need their decimals. */
function formatValue(value: number): string {
  if (Math.abs(value) >= 100) return Math.round(value).toLocaleString();
  if (Math.abs(value) >= 10) return value.toFixed(1);
  return value.toFixed(2);
}
