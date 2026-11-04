import { KpiTile } from "@/components/charts/KpiTile";
import { Ring } from "@/components/charts/Ring";
import { Icon } from "@/components/ui/Icon";
import type { CalibrationResponse, MmrEstimate } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * The Rank page's header tiles: medal and status, MMR estimate inside the
 * medal band, the next medal, how settled the rank is, and the current
 * streak. Every figure is the server's; "est." marks the modeled ones.
 */
export function RankHeader({ data }: { data: CalibrationResponse }) {
  const { established_rank: rank, confidence, streak, momentum } = data;
  const lastTen = momentum.points.slice(-10).map((p) => p.won);
  const settled = confidence.is_calibrated;

  return (
    <section aria-label="Rank summary" className="grid grid-cols-2 gap-3 lg:grid-cols-5">
      <KpiTile
        label="Medal"
        className="col-span-2 lg:col-span-1"
        value={rank.label ?? "No rank"}
        badge={
          <span
            className={cn(
              "flex items-center gap-1 rounded-full px-2 py-0.5 text-[0.625rem] font-semibold",
              settled ? "bg-mark-win/15 text-string" : "bg-warn/15 text-warn",
            )}
          >
            <Icon name={settled ? "check" : "clock"} className="size-3" />
            {settled ? "Calibrated" : "Calibrating"}
          </span>
        }
        comparison={
          rank.leaderboard_rank !== null ? (
            <span className="font-mono text-xs tabular-nums text-keyword">Leaderboard #{rank.leaderboard_rank.toLocaleString("en-US")}</span>
          ) : rank.label ? null : (
            <span className="text-xs text-ink-faint">Profile private, or not calibrated yet</span>
          )
        }
      />

      <KpiTile
        label="MMR · est."
        info="The middle of the band your medal and stars imply. Valve does not publish where inside it you sit."
        value={rank.mmr ? `~${rank.mmr.midpoint.toLocaleString("en-US")}` : "—"}
        comparison={rank.mmr ? <MedalBand mmr={rank.mmr} /> : null}
      />

      <KpiTile
        label="Next medal · est."
        info="Wins needed at the modeled MMR per win, from the middle of your current star. An estimate."
        value={rank.next_medal ? `≈ ${rank.next_medal.wins_needed} wins` : "—"}
        comparison={
          rank.next_medal && rank.mmr ? (
            <span className="flex flex-col gap-1">
              <span className="text-xs text-ink-faint">
                to {rank.next_medal.label} · {rank.next_medal.mmr_needed.toLocaleString("en-US")} MMR
              </span>
              <NextMedalBar mmr={rank.mmr} />
            </span>
          ) : (
            <span className="text-xs text-ink-faint">{rank.rank_tier === 80 ? "Top medal" : " "}</span>
          )
        }
      />

      <KpiTile
        label="Settled"
        info={`Ranked games in the last ${confidence.window_days} days against ${confidence.target_matches}. At the target your rank reads as settled.`}
        value={`${confidence.matches_counted} / ${confidence.target_matches}`}
        visual={
          <Ring
            share={confidence.confidence_pct / 100}
            label={`${confidence.matches_counted} of ${confidence.target_matches} ranked games in ${confidence.window_days} days`}
            text={settled ? "✓" : `${Math.round(confidence.confidence_pct)}%`}
            tone={settled ? "good" : "warn"}
          />
        }
        comparison={<span className="text-xs text-ink-faint">last {confidence.window_days} days</span>}
      />

      <KpiTile
        label="Streak"
        className="col-span-2 lg:col-span-1"
        value={
          streak.kind === null ? (
            "—"
          ) : (
            <span className={streak.kind === "win" ? "text-string" : "text-error"}>
              {streak.count}
              {streak.kind === "win" ? "W" : "L"}
            </span>
          )
        }
        comparison={
          lastTen.length > 0 ? (
            <span role="img" aria-label={`Last ${lastTen.length} ranked: ${lastTen.filter(Boolean).length} won, oldest first`} className="flex gap-1">
              {lastTen.map((won, i) => (
                <span key={i} className={cn("size-2 rounded-full", won ? "bg-mark-win" : "bg-mark-loss")} />
              ))}
            </span>
          ) : (
            <span className="text-xs text-ink-faint">no ranked games</span>
          )
        }
      />
    </section>
  );
}

/** The star's band inside the whole medal's band, midpoint marked. */
function MedalBand({ mmr }: { mmr: MmrEstimate }) {
  if (mmr.medal_high === null || mmr.high === null) {
    return <span className="text-xs text-ink-faint">{mmr.low.toLocaleString("en-US")}+</span>;
  }
  const span = mmr.medal_high - mmr.medal_low || 1;
  const at = (v: number) => `${((v - mmr.medal_low) / span) * 100}%`;
  return (
    <span className="flex flex-col gap-1">
      <span
        role="img"
        aria-label={`Band ${mmr.low}-${mmr.high} within the medal's ${mmr.medal_low}-${mmr.medal_high}`}
        className="relative block h-1.5 w-full rounded-full bg-mark-track"
      >
        <span className="absolute inset-y-0 rounded-full bg-you/60" style={{ left: at(mmr.low), width: `calc(${at(mmr.high)} - ${at(mmr.low)})` }} />
        <span className="absolute -inset-y-0.5 w-0.5 rounded bg-ink" style={{ left: at(mmr.midpoint) }} />
      </span>
      <span className="flex justify-between font-mono text-[0.625rem] tabular-nums text-ink-faint">
        <span>{mmr.medal_low.toLocaleString("en-US")}</span>
        <span>{mmr.medal_high.toLocaleString("en-US")}</span>
      </span>
    </span>
  );
}

/** How far through the current medal the midpoint is. */
function NextMedalBar({ mmr }: { mmr: MmrEstimate }) {
  if (mmr.medal_high === null) return null;
  const share = (mmr.midpoint - mmr.medal_low) / (mmr.medal_high + 1 - mmr.medal_low || 1);
  return (
    <span className="block h-1.5 w-full overflow-hidden rounded-full bg-mark-track">
      <span className="block h-full rounded-r bg-you" style={{ width: `${Math.max(0, Math.min(1, share)) * 100}%` }} />
    </span>
  );
}
