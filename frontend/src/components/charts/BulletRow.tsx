import type { BenchmarkResult, TargetMetric } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * One metric against its peer distribution — a bullet chart.
 *
 * The right form for "a value and the reference points around it": a bar for
 * the player, ticks for the references. A grouped bar chart would imply
 * independent quantities; they are one quantity and its thresholds.
 *
 * The references depend on what is being asked:
 *
 *   - **No target** — the player's own bracket alone. Median for context, the
 *     top-20% line as the thing to reach. That is the page's original question,
 *     "how am I doing", and it is unchanged.
 *   - **A target** — their bracket's median and the target bracket's, so the
 *     rung they are on and the rung above it sit on one scale. The top-20% tick
 *     steps aside here rather than joining them: three reference marks on a
 *     bar this size is over-plotting, and the aspirational one the reader chose
 *     wins over the one they did not.
 *
 * The scale runs past whichever mark is largest, so nothing is ever clipped,
 * and every mark keeps a text label — colour alone never carries identity.
 */
export function BulletRow({
  result,
  target,
  ownLabel,
  targetLabel,
}: {
  result: BenchmarkResult;
  /** This metric's figures in the bracket being aimed at, when there is one. */
  target?: TargetMetric;
  /** Names the player's own bracket, e.g. "Legend". */
  ownLabel?: string;
  /** Names the bracket being aimed at, e.g. "Ancient". Comes from the parent:
   *  `TargetMetric.label` is the *metric's* name, not the bracket's. */
  targetLabel?: string;
}) {
  const { player_value, peer_median, top_20_value, percentile } = result;

  // With a target on the row the top-20% line stands down — see the note above.
  const topLine = target ? null : top_20_value;
  const targetMedian = target?.peer_median ?? null;

  // A little headroom so a bar at the maximum does not touch the edge, and so
  // a target median above everything else still lands inside the track.
  const ceiling =
    Math.max(player_value, peer_median ?? 0, topLine ?? 0, targetMedian ?? 0) *
      1.08 || 1;
  const pct = (v: number) => `${Math.max(0, Math.min(1, v / ceiling)) * 100}%`;

  // "Ahead" means past the reference that is currently the target, in the
  // direction that is good. With a bracket to aim at, that is its median;
  // without one, the top-20% line.
  const ahead = target
    ? target.cleared
    : topLine !== null &&
      (result.higher_is_better
        ? player_value >= topLine
        : player_value <= topLine);

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

      <div
        className="relative h-3 w-full rounded-full bg-mark-track"
        role="img"
        aria-label={describe(result, target, ownLabel, targetLabel)}
      >
        {/* The player's value. */}
        <div
          className={cn(
            "absolute inset-y-0 left-0 rounded-r-full",
            ahead ? "bg-mark-win" : "bg-mark-line",
          )}
          style={{ width: pct(player_value) }}
        />

        {/* The player's own bracket: a recessive hairline, since it is context
            rather than a target. */}
        {peer_median !== null ? (
          <span
            aria-hidden
            title={`${ownLabel ?? "Peer"} median ${formatMetricValue(peer_median)}`}
            className="absolute inset-y-[-3px] w-px bg-ink-faint"
            style={{ left: pct(peer_median) }}
          />
        ) : null}

        {/* The bracket being aimed at. Heavier than the hairline because it is
            the thing to reach, and ringed in the surface colour so it stays
            legible when the two brackets sit almost on top of each other. */}
        {targetMedian !== null ? (
          <span
            aria-hidden
            title={`${targetLabel ?? "Target"} median ${formatMetricValue(targetMedian)}`}
            className="absolute inset-y-[-5px] w-1 rounded bg-number ring-2 ring-surface-2"
            style={{ left: pct(targetMedian) }}
          />
        ) : null}

        {/* Top 20% of the player's own bracket. Only without a target. */}
        {topLine !== null ? (
          <span
            aria-hidden
            title={`Top 20% ${formatMetricValue(topLine)}`}
            className="absolute inset-y-[-4px] w-0.5 rounded bg-number"
            style={{ left: pct(topLine) }}
          />
        ) : null}
      </div>

      {/* Every mark named with its value. The chart is never read by position
          and colour alone. */}
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[0.6875rem] text-ink-faint">
        {peer_median !== null ? (
          <span className="inline-flex items-center gap-1.5">
            <span aria-hidden className="h-2.5 w-px bg-ink-faint" />
            {ownLabel ?? "median"} {formatMetricValue(peer_median)}
          </span>
        ) : null}

        {targetMedian !== null ? (
          <span className="inline-flex items-center gap-1.5 text-ink-muted">
            <span aria-hidden className="h-2.5 w-1 rounded bg-number" />
            {targetLabel} {formatMetricValue(targetMedian)}
          </span>
        ) : null}

        {topLine !== null ? (
          <span className="inline-flex items-center gap-1.5">
            <span aria-hidden className="h-2.5 w-0.5 rounded bg-number" />
            top 20% {formatMetricValue(topLine)}
          </span>
        ) : null}

        {/* The sentence the row exists to deliver. */}
        {target ? (
          <TargetGap target={target} label={targetLabel} />
        ) : result.gap_to_top_20 !== null ? (
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
 * How far short of the target bracket this metric is, or that it is already
 * past it.
 *
 * `gap_to_median` arrives signed so positive always means work to do,
 * whichever direction the metric runs — nothing here inverts anything for
 * deaths.
 */
function TargetGap({
  target,
  label,
}: {
  target: TargetMetric;
  label?: string;
}) {
  if (target.gap_to_median === null) return null;
  const bracket = label ?? "the target";

  if (target.cleared) {
    return <span className="text-string">already past {bracket}</span>;
  }

  return (
    <span className="text-ink-muted">
      {formatMetricValue(target.gap_to_median)} short of {bracket}
    </span>
  );
}

/**
 * The row as a sentence, for anyone who cannot see the marks.
 *
 * Follows `PercentileRow.describe` — charts here never carry meaning in
 * position alone, so every mark that exists visually is named here too.
 */
function describe(
  result: BenchmarkResult,
  target: TargetMetric | undefined,
  ownLabel: string | undefined,
  targetLabel: string | undefined,
): string {
  const parts: string[] = [
    `${result.label}: you ${formatMetricValue(result.player_value)}`,
  ];

  if (result.percentile !== null) {
    parts.push(`better than ${Math.round(result.percentile)}% of your bracket`);
  }
  if (result.peer_median !== null) {
    parts.push(
      `${ownLabel ?? "peer"} median ${formatMetricValue(result.peer_median)}`,
    );
  }

  if (target) {
    const bracket = targetLabel ?? "the target";
    if (target.peer_median !== null) {
      parts.push(`${bracket} median ${formatMetricValue(target.peer_median)}`);
    }
    if (target.gap_to_median !== null) {
      parts.push(
        target.cleared
          ? `already past ${bracket}`
          : `${formatMetricValue(target.gap_to_median)} short of ${bracket}`,
      );
    }
  } else if (result.top_20_value !== null) {
    parts.push(`top 20% ${formatMetricValue(result.top_20_value)}`);
  }

  return `${parts.join("; ")}.`;
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
