import type { BenchmarkResult } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * One metric against its peer distribution — a bullet chart.
 *
 * The right form for "a value, a reference point and a target on one scale":
 * a bar for the player, a tick for the peer median, a tick for the top-20%
 * line. A grouped bar chart would imply three independent quantities; they are
 * one quantity and two thresholds.
 *
 * The scale runs to whichever is largest of the three, so nothing is ever
 * clipped, and every mark keeps a label — colour alone never carries identity.
 */
export function BulletRow({ result }: { result: BenchmarkResult }) {
  const { player_value, peer_median, top_20_value, percentile } = result;

  // A little headroom so a bar at the maximum does not touch the edge.
  const ceiling =
    Math.max(player_value, peer_median ?? 0, top_20_value ?? 0) * 1.08 || 1;
  const pct = (v: number) => `${Math.max(0, Math.min(1, v / ceiling)) * 100}%`;

  // "Ahead" means past the top-20% line in the direction that is good.
  const ahead =
    top_20_value !== null &&
    (result.higher_is_better
      ? player_value >= top_20_value
      : player_value <= top_20_value);

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-sm text-ink">{result.label}</span>
        <span className="flex items-baseline gap-2">
          <span className="font-mono text-sm tabular-nums text-number">
            {formatMetricValue(player_value)}
          </span>
          {percentile !== null ? (
            <span
              className={cn(
                "rounded px-1.5 py-0.5 font-mono text-[0.625rem] tabular-nums",
                ahead
                  ? "bg-mark-win/15 text-string"
                  : "bg-mark-track text-ink-muted",
              )}
            >
              p{Math.round(percentile)}
            </span>
          ) : (
            <span className="rounded bg-mark-track px-1.5 py-0.5 text-[0.625rem] text-ink-faint">
              unranked
            </span>
          )}
        </span>
      </div>

      <div className="relative h-3 w-full rounded-full bg-mark-track">
        {/* The player's value. */}
        <div
          className={cn(
            "absolute inset-y-0 left-0 rounded-r-full",
            ahead ? "bg-mark-win" : "bg-mark-line",
          )}
          style={{ width: pct(player_value) }}
        />

        {/* Peer median: a recessive hairline, since it is context not target. */}
        {peer_median !== null ? (
          <span
            aria-hidden
            title={`Peer median ${formatMetricValue(peer_median)}`}
            className="absolute inset-y-[-3px] w-px bg-ink-faint"
            style={{ left: pct(peer_median) }}
          />
        ) : null}

        {/* Top 20%: the target, so it reads heavier than the median. */}
        {top_20_value !== null ? (
          <span
            aria-hidden
            title={`Top 20% ${formatMetricValue(top_20_value)}`}
            className="absolute inset-y-[-4px] w-0.5 rounded bg-number"
            style={{ left: pct(top_20_value) }}
          />
        ) : null}
      </div>

      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[0.6875rem] text-ink-faint">
        {peer_median !== null ? (
          <span className="inline-flex items-center gap-1.5">
            <span aria-hidden className="h-2.5 w-px bg-ink-faint" />
            median {formatMetricValue(peer_median)}
          </span>
        ) : null}
        {top_20_value !== null ? (
          <span className="inline-flex items-center gap-1.5">
            <span aria-hidden className="h-2.5 w-0.5 rounded bg-number" />
            top 20% {formatMetricValue(top_20_value)}
          </span>
        ) : null}
        {result.gap_to_top_20 !== null ? (
          <span className={ahead ? "text-string" : undefined}>
            {ahead
              ? `${formatMetricValue(Math.abs(result.gap_to_top_20))} ahead`
              : `${formatMetricValue(result.gap_to_top_20)} to go`}
          </span>
        ) : null}
      </div>

      {result.note ? (
        <p className="text-[0.6875rem] leading-relaxed text-ink-faint">
          {result.note}
        </p>
      ) : null}
    </div>
  );
}

/**
 * A benchmark value in the units a player reads it in.
 *
 * Large figures read better whole; small rates need their decimals.
 *
 * Exported so anything else showing a figure from `BenchmarkResult` — the
 * preliminary training focus, for one — writes it the same way. Precision
 * scales with magnitude: 2 decimals on a deaths-per-minute rate, none on tower
 * damage.
 */
export function formatMetricValue(value: number): string {
  if (Math.abs(value) >= 100) return Math.round(value).toLocaleString();
  if (Math.abs(value) >= 10) return value.toFixed(1);
  return value.toFixed(2);
}
