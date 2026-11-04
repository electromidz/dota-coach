import { BulletBar } from "@/components/charts/BulletBar";
import { KpiTile } from "@/components/charts/KpiTile";
import type { LaningReading, MatchComparisonResponse, MatchView, MetricComparison } from "@/lib/types";
import { cn, ordinal } from "@/lib/utils";

type Tile = {
  label: string;
  value: number | null;
  /** The comparison row behind the badge and delta, by metric slug. */
  metric: string | null;
};

/**
 * One row of headline figures. Each tile: the value, its percentile in this
 * game, and the difference from the player's own baseline as a percentage.
 * A percentage, because the baseline for totals is per minute: the relative
 * gap is the same, the absolute one would not be.
 */
export function KpiStrip({
  match,
  comparison,
}: {
  match: MatchView;
  comparison: MatchComparisonResponse | null;
}) {
  const tiles: Tile[] = [
    { label: "GPM", value: match.gpm, metric: "gold_per_min" },
    { label: "XPM", value: match.xpm, metric: "xp_per_min" },
    { label: "Last hits", value: match.last_hits, metric: "last_hits_per_min" },
    { label: "Net worth", value: match.net_worth, metric: null },
    { label: "Hero dmg", value: match.hero_damage, metric: "hero_damage_per_min" },
    { label: "Tower dmg", value: match.tower_damage, metric: "tower_damage" },
  ];
  const rows = new Map((comparison?.metrics ?? []).map((m) => [m.metric, m]));

  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-6">
      {tiles.map((tile) => {
        const row = tile.metric ? rows.get(tile.metric) : undefined;
        return (
          <KpiTile
            key={tile.label}
            label={tile.label}
            className="p-3"
            value={
              tile.value === null ? (
                <span className="text-ink-faint" title="Not available">—</span>
              ) : (
                <span className="font-mono text-xl tabular-nums text-number sm:text-2xl">
                  {tile.value.toLocaleString("en-US")}
                </span>
              )
            }
            badge={<PercentileBadge row={row} />}
            comparison={<BaselineDelta row={row} loading={comparison === null} />}
          />
        );
      })}
    </div>
  );
}

function PercentileBadge({ row }: { row: MetricComparison | undefined }) {
  const p = row?.this_match?.percentile ?? null;
  if (p === null) return null;
  return (
    <span
      title={`This game: ${ordinal(p)} percentile of peers`}
      className={cn(
        "shrink-0 rounded px-1.5 py-0.5 font-mono text-[0.625rem] tabular-nums",
        p >= 70 ? "bg-mark-win/15 text-string" : p <= 30 ? "bg-mark-loss/15 text-error" : "bg-mark-track text-ink-muted",
      )}
    >
      {ordinal(p)}
    </span>
  );
}

function BaselineDelta({ row, loading }: { row: MetricComparison | undefined; loading: boolean }) {
  if (loading) return <span className="text-[0.6875rem] text-ink-faint">…</span>;
  const current = row?.this_match?.value;
  const average = row?.hero_average?.value;
  if (current === undefined || average === undefined || average === 0) {
    return <span className="text-[0.6875rem] text-ink-faint">no baseline</span>;
  }
  const change = Math.round(((current - average) / Math.abs(average)) * 100);
  const better = row!.higher_is_better ? change > 0 : change < 0;
  return (
    <span
      className={cn(
        "font-mono text-[0.6875rem] tabular-nums",
        change === 0 ? "text-ink-faint" : better ? "text-string" : "text-error",
      )}
    >
      {change > 0 ? `+${change}` : change}% vs your avg
    </span>
  );
}

/**
 * Ten-minute laning figures as bullet bars against the player's own baseline.
 * No provider publishes a peer distribution for these, so none is drawn.
 */
export function LaningPanel({ readings }: { readings: LaningReading[] }) {
  return (
    <ul className="m-0 flex list-none flex-col gap-3 p-0">
      {readings.map((r) => (
        <BulletBar
          key={r.metric}
          label={r.label}
          fullLabel={r.label}
          value={r.value}
          percentile={null}
          rankMedian={r.baseline}
          targetMedian={null}
          cleared={null}
          shortfall={null}
          rankLabel="Your avg"
          trailing={
            <span className="whitespace-nowrap text-[0.625rem] text-ink-faint">
              {r.baseline !== null
                ? `avg ${Math.round(r.baseline).toLocaleString("en-US")}`
                : "no avg"}
            </span>
          }
        />
      ))}
    </ul>
  );
}
