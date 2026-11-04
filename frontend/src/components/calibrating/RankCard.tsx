import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import type { EstablishedRank, RankConfidence } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * The headline: where the player stands, and how settled that is.
 *
 * The medal is a *measurement* — it is what Valve reported the last time the
 * sync read the profile — so it is stated plainly and without hedging. The
 * confidence beside it is a count: ranked games in the last N days against a
 * target, "21 / 20 · Settled". This is the only place the page shows it.
 *
 * An account with no medal shows no medal. There is no "Unranked" placeholder
 * in the tier position, because a placeholder in the shape of a rank reads as
 * a rank.
 */
export function RankCard({
  rank,
  confidence,
  className,
}: {
  rank: EstablishedRank;
  confidence: RankConfidence;
  className?: string;
}) {

  return (
    <Card className={cn("flex flex-col gap-5", className)}>
      <div className="flex items-start justify-between gap-4">
        <div className="flex flex-col gap-1">
          <p className="text-xs uppercase tracking-widest text-ink-faint">
            Established rank
          </p>

          {rank.label ? (
            <p className="font-display text-3xl tracking-wide text-ink sm:text-4xl">
              {rank.label}
            </p>
          ) : (
            <p className="text-lg leading-snug text-ink-muted">
              No rank reported
              <span className="mt-1 block text-sm text-ink-faint">
                Your Dota profile is private, or you have not calibrated yet.
              </span>
            </p>
          )}

          {rank.mmr ? (
            <p className="mt-1.5 flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
              <span className="font-mono text-lg font-bold tabular-nums text-ink">
                ~{rank.mmr.midpoint.toLocaleString()}
                <span className="ml-1 text-xs font-normal text-ink-faint">
                  MMR
                </span>
              </span>
              {/* The band is the part that is actually pinned down by the
                  medal; the single figure above is its middle. Showing both
                  keeps the headline number from reading as a measurement of
                  where inside the band this player sits — Valve publishes
                  nothing that could say. */}
              <span className="font-mono text-xs tabular-nums text-ink-faint">
                {rank.mmr.high === null
                  ? `${rank.mmr.low.toLocaleString()}+`
                  : `${rank.mmr.low.toLocaleString()}–${rank.mmr.high.toLocaleString()}`}
                <span className="ml-1 font-sans">estimated from your medal</span>
              </span>
            </p>
          ) : null}

          {rank.leaderboard_rank !== null ? (
            <p className="mt-1 font-mono text-sm tabular-nums text-keyword">
              Leaderboard #{rank.leaderboard_rank.toLocaleString()}
            </p>
          ) : null}
        </div>

        {confidence.is_calibrated ? (
          <span className="flex shrink-0 items-center gap-1.5 rounded-full bg-mark-win/15 px-2.5 py-1 text-xs font-semibold text-string">
            <Icon name="check" className="size-3.5" />
            Calibrated
          </span>
        ) : (
          <span className="flex shrink-0 items-center gap-1.5 rounded-full bg-warn/15 px-2.5 py-1 text-xs font-semibold text-warn">
            <Icon name="clock" className="size-3.5" />
            Calibrating
          </span>
        )}
      </div>

      <ConfidenceMeter confidence={confidence} />
    </Card>
  );
}

/**
 * Rank confidence as a count: ranked games in the window against the target.
 * The bar fills at the target; the count keeps going ("21 / 20").
 */
function ConfidenceMeter({ confidence }: { confidence: RankConfidence }) {
  const { matches_counted: count, target_matches: target, window_days: days } = confidence;
  const settled = confidence.is_calibrated;
  const pct = Math.max(0, Math.min(100, confidence.confidence_pct));

  return (
    <figure className="m-0 flex flex-col gap-2">
      <figcaption className="flex items-baseline justify-between gap-3">
        <span className="text-xs uppercase tracking-wider text-ink-faint">
          Rank confidence
        </span>
        <span className="font-mono text-sm tabular-nums text-ink">
          {count} / {target}
          <span className={cn("ml-1.5 font-sans text-xs", settled ? "text-string" : "text-warn")}>
            · {settled ? "Settled" : "Settling"}
          </span>
        </span>
      </figcaption>

      <div
        role="meter"
        aria-valuenow={count}
        aria-valuemin={0}
        aria-valuemax={target}
        aria-label={`Rank confidence: ${count} of ${target} ranked games in the last ${days} days. ${
          settled ? "Settled" : "Not settled yet"
        }.`}
        className="h-2.5 w-full overflow-hidden rounded-full bg-mark-track"
      >
        <div
          className={cn(
            "h-full rounded-r transition-[width] duration-500 ease-out",
            settled ? "bg-mark-win" : "bg-warn",
          )}
          style={{ width: `${pct}%` }}
        />
      </div>

      <p className="text-xs text-ink-faint">
        Ranked games in the last {days} days
      </p>
    </figure>
  );
}
