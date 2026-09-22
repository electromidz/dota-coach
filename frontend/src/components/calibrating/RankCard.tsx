import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import type { EstablishedRank, RankConfidence } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * The headline: where the player stands, and how settled that is.
 *
 * The medal is a *measurement* — it is what Valve reported the last time the
 * sync read the profile — so it is stated plainly and without hedging. The
 * confidence meter beside it is Valve's own 0→100% model, and the threshold
 * mark is where the game stops calling an account uncalibrated.
 *
 * An account with no medal shows no medal. There is no "Unranked" placeholder
 * in the tier position, because a placeholder in the shape of a rank reads as
 * a rank.
 */
export function RankCard({
  rank,
  confidence,
  thresholdPct,
  className,
}: {
  rank: EstablishedRank;
  confidence: RankConfidence;
  /** Where the game considers an account calibrated. From the server. */
  thresholdPct: number;
  className?: string;
}) {
  const pct = Math.max(0, Math.min(100, confidence.confidence_pct));

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

      <ConfidenceMeter
        pct={pct}
        thresholdPct={thresholdPct}
        matchesCounted={confidence.matches_counted}
        isCalibrated={confidence.is_calibrated}
      />
    </Card>
  );
}

/**
 * Rank confidence, 0→100%, with the calibration threshold marked on the track.
 *
 * Not `Meter`: this one carries a reference line the shared component has no
 * concept of, and the threshold is the part a player actually reads the bar
 * for — "am I past it yet". The geometry otherwise matches the house spec, so
 * it sits beside a `Meter` without looking like a different product.
 */
function ConfidenceMeter({
  pct,
  thresholdPct,
  matchesCounted,
  isCalibrated,
}: {
  pct: number;
  thresholdPct: number;
  matchesCounted: number;
  isCalibrated: boolean;
}) {
  return (
    <figure className="m-0 flex flex-col gap-2">
      <figcaption className="flex items-baseline justify-between gap-3">
        <span className="text-xs uppercase tracking-wider text-ink-faint">
          Rank confidence
        </span>
        <span className="font-mono text-sm tabular-nums text-ink">
          {pct.toFixed(0)}%
        </span>
      </figcaption>

      <div className="relative">
        <div
          role="meter"
          aria-valuenow={Math.round(pct)}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label={`Rank confidence, ${Math.round(pct)} percent of 100. ${
            isCalibrated ? "Calibrated" : "Calibrated at"
          } ${thresholdPct} percent.`}
          className="h-2.5 w-full overflow-hidden rounded-full bg-mark-track"
        >
          <div
            className={cn(
              "h-full rounded-r transition-[width] duration-500 ease-out",
              isCalibrated ? "bg-mark-win" : "bg-warn",
            )}
            style={{ width: `${pct}%` }}
          />
        </div>

        {/* The threshold, drawn over the track rather than baked into it, so
            moving the server's value moves the mark. */}
        <span
          aria-hidden
          className="absolute top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-full bg-ink-faint"
          style={{ left: `${Math.max(0, Math.min(100, thresholdPct))}%` }}
        />
      </div>

      <p className="text-xs leading-relaxed text-ink-faint">
        {matchesCounted === 0
          ? "No recent ranked matches. Confidence decays while an account sits idle."
          : isCalibrated
            ? `Past the ${thresholdPct}% mark, from ${matchesCounted} recent ranked ${
                matchesCounted === 1 ? "match" : "matches"
              }.`
            : `${matchesCounted} recent ranked ${
                matchesCounted === 1 ? "match" : "matches"
              } so far — the mark at ${thresholdPct}% is where the game calls a rank settled.`}
      </p>
    </figure>
  );
}
