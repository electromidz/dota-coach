import type { RankDistribution } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * How much the player's numbers resemble each rank bracket.
 *
 * Strongest match first, one bar per bracket, a share out of 100 — the shape
 * every rank tracker draws for this, because it reads at a glance.
 *
 * The quantity behind the bars is not a percentile, and that matters. Sorting
 * percentiles descending inverts the chart's meaning: beating 98% of Heralds
 * is the highest number on the board and the furthest thing from being a
 * Herald, so a sorted percentile chart would put Herald on top for a Divine
 * player. The server scores each bracket by how near the middle of it the
 * player sits and normalises those scores, which is a quantity that survives
 * being sorted.
 *
 * It is still not a probability of calibrating into the bracket. Valve
 * publishes no calibration outcomes, so no honest model can be fitted to them,
 * and the caption says so rather than letting a percentage imply it.
 */
export function RankDistributionBars({
  distribution,
  className,
}: {
  distribution: RankDistribution;
  className?: string;
}) {
  const { resemblance } = distribution;

  if (resemblance.length === 0) {
    return (
      <p className={cn("text-sm text-ink-faint", className)}>
        {distribution.note ?? "Nothing to place against a bracket yet."}
      </p>
    );
  }

  return (
    <figure className={cn("m-0 flex flex-col gap-4", className)}>
      <figcaption className="sr-only">
        How much this player&rsquo;s {distribution.hero_name} numbers resemble
        each rank bracket, strongest match first.
      </figcaption>

      <div className="flex flex-col gap-2.5">
        {resemblance.map((row) => (
          <div key={row.bracket} className="flex items-center gap-3">
            <span
              title={row.label}
              className={cn(
                "w-20 shrink-0 truncate text-right text-xs uppercase tracking-widest",
                row.is_highest ? "text-ink" : "text-ink-faint",
              )}
            >
              {row.label}
            </span>

            <div className="h-2.5 flex-1 overflow-hidden rounded-full bg-mark-track">
              <div
                className={cn(
                  "h-full rounded-r transition-[width] duration-500 ease-out",
                  row.is_highest ? "bg-keyword" : "bg-operator/40",
                )}
                style={{ width: `${clamp(row.percentage)}%` }}
              />
            </div>

            <span
              className={cn(
                "w-9 shrink-0 text-right font-mono text-xs font-bold tabular-nums",
                row.is_highest ? "text-keyword" : "text-ink-faint",
              )}
            >
              {Math.round(row.percentage)}%
            </span>

            {row.is_player_bracket ? (
              <span className="shrink-0 text-[0.625rem] text-ink-faint">
                your medal
              </span>
            ) : null}
          </div>
        ))}
      </div>

      <p className="text-xs leading-relaxed text-ink-faint">
        <span className="font-semibold text-ink-muted">What this is.</span> How
        closely your {distribution.hero_name} numbers over {distribution.sample}{" "}
        {distribution.sample === 1 ? "match" : "matches"} resemble each
        bracket&rsquo;s real peers, as a share of the whole. Not a chance of
        calibrating there — Valve publishes no calibration outcomes, so nobody
        can honestly put odds on it.
      </p>

      {distribution.note ? (
        <p className="text-xs text-ink-faint">{distribution.note}</p>
      ) : null}
    </figure>
  );
}

function clamp(value: number): number {
  return Math.max(0, Math.min(100, value));
}
