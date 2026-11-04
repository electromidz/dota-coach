import { Icon } from "@/components/ui/Icon";
import { InfoTip } from "@/components/ui/InfoTip";
import type { BenchmarkResult, Consistency, EstablishedRank } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * The established rank, and the measurements standing behind it.
 *
 * "Official" because the medal itself is not modelled — it is whatever Valve
 * reported the last time the sync read the profile. Everything beside it is
 * measured too: the per-metric rows are the player's own figures against their
 * own bracket's real peer distribution, and consistency is arithmetic over
 * stored matches.
 *
 * Rank confidence is not repeated here: the header card shows it once.
 *
 * Two things this deliberately does not show:
 *
 *   * **An MMR number.** Valve stopped publishing per-match MMR years ago, so
 *     any exact figure is chosen rather than derived.
 *   * **A defaulted consistency.** Below the sample floor the server sends
 *     `null` and this says so. A plausible-looking stand-in would be a
 *     measurement nobody made.
 */
export function EstablishedRankPanel({
  rank,
  consistency,
  metrics,
  className,
}: {
  rank: EstablishedRank;
  consistency: Consistency | null;
  /** Placement against the player's own bracket, per metric. */
  metrics: BenchmarkResult[];
  className?: string;
}) {
  const ranked = metrics.filter((m) => m.percentile !== null);

  return (
    <div className={cn("flex flex-col gap-5", className)}>
      <div className="flex flex-wrap items-center gap-2">
        <span className="flex items-center gap-1.5 rounded-full border border-glass-edge bg-surface-2/60 px-2.5 py-1 text-xs font-medium text-ink-muted">
          <Icon name="shield" className="size-3.5 text-keyword" />
          {rank.label ? "Official established rank" : "No rank reported"}
        </span>

        {rank.label ? (
          <span className="font-display text-xl tracking-wide text-ink">
            {rank.label}
          </span>
        ) : null}

      </div>

      <dl className="grid grid-cols-2 gap-3 sm:grid-cols-3">
        <Stat
          label="Consistency"
          info="How steady your KDA is from game to game in ranked matches: 100 means every game looks alike, 0 means they swing wildly. Not how good the games were."
          value={
            consistency ? `${Math.round(consistency.percentage)}%` : "Not yet"
          }
          tone={consistency ? "neutral" : "muted"}
          caption={
            consistency
              ? `over ${consistency.matches} matches`
              : "needs 10 ranked matches"
          }
        />
      </dl>

      {ranked.length > 0 ? (
        <div className="flex flex-col gap-2.5">
          <p className="text-xs uppercase tracking-widest text-ink-faint">
            Against your own bracket
          </p>

          {ranked.map((metric) => (
            <div key={metric.metric} className="flex items-center gap-3">
              <span className="w-28 shrink-0 truncate text-xs text-ink-muted">
                {metric.label}
              </span>

              <div className="h-2 flex-1 overflow-hidden rounded-full bg-mark-track">
                <div
                  className="h-full rounded-r bg-operator transition-[width] duration-500 ease-out"
                  style={{ width: `${clamp(metric.percentile ?? 0)}%` }}
                />
              </div>

              <span className="w-10 shrink-0 text-right font-mono text-xs tabular-nums text-ink-muted">
                {Math.round(metric.percentile ?? 0)}
              </span>
            </div>
          ))}

          <p className="text-xs leading-relaxed text-ink-faint">
            Percentiles against the real peer distribution for your medal — 70
            means better than 70% of them. Measured, not modelled.
          </p>
        </div>
      ) : null}
    </div>
  );
}

function Stat({
  label,
  info,
  value,
  caption,
  tone,
}: {
  label: string;
  /** What the figure means, behind an (i). */
  info?: string;
  value: string;
  caption: string;
  tone: "good" | "warn" | "neutral" | "muted";
}) {
  return (
    <div className="flex flex-col gap-0.5 rounded-xl border border-glass-edge bg-surface-2/40 p-3">
      <dt className="flex items-center gap-1 text-[0.625rem] uppercase tracking-widest text-ink-faint">
        {label}
        {info ? <InfoTip label={`About ${label.toLowerCase()}`}>{info}</InfoTip> : null}
      </dt>
      <dd
        className={cn(
          "font-mono text-lg font-bold tabular-nums",
          tone === "good" && "text-string",
          tone === "warn" && "text-warn",
          tone === "neutral" && "text-ink",
          tone === "muted" && "text-ink-faint",
        )}
      >
        {value}
      </dd>
      <p className="text-[0.625rem] text-ink-faint">{caption}</p>
    </div>
  );
}

function clamp(value: number): number {
  return Math.max(0, Math.min(100, value));
}
