import { Delta } from "@/components/charts/Delta";
import { KpiTile } from "@/components/charts/KpiTile";
import type { CalibrationResponse, KpiTrend, OverviewTrends } from "@/lib/types";

/**
 * The KPI row: rank, win rate, KDA, GPM and matches this month. Every value,
 * delta and series comes from the server; this only formats them.
 */
export function OverviewKpis({
  trends,
  calibration,
}: {
  trends: OverviewTrends;
  /** Absent while loading, on the free plan, or before a sync. */
  calibration: CalibrationResponse | null;
}) {
  const vs = `vs prev ${trends.window}`;

  return (
    <section aria-label="Key numbers" className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5">
      <RankTile calibration={calibration} />
      <TrendTile
        label="Win rate"
        trend={trends.win_rate}
        format={(v) => `${Math.round(v)}%`}
        deltaFormat={(v) => `${Math.round(v)} pts`}
        suffix={vs}
        info={`Last ${trends.window} Ranked + All Pick games against the ${trends.window} before. The line is a rolling ${trends.rolling}-game win rate.`}
      />
      <TrendTile
        label="KDA"
        trend={trends.kda}
        format={(v) => v.toFixed(2)}
        deltaFormat={(v) => v.toFixed(2)}
        suffix={vs}
      />
      <TrendTile
        label="GPM"
        trend={trends.gpm}
        format={(v) => Math.round(v).toLocaleString("en-US")}
        deltaFormat={(v) => Math.round(v).toLocaleString("en-US")}
        suffix={vs}
      />
      <TrendTile
        label="Matches · 30d"
        trend={trends.matches_30d}
        format={(v) => Math.round(v).toLocaleString("en-US")}
        deltaFormat={(v) => Math.round(v).toString()}
        suffix="vs prev 30d"
        info="Ranked + All Pick games in the last 30 days. The line is games per week over 8 weeks."
      />
    </section>
  );
}

function TrendTile({
  label,
  trend,
  format,
  deltaFormat,
  suffix,
  info,
}: {
  label: string;
  trend: KpiTrend;
  format: (v: number) => string;
  deltaFormat: (v: number) => string;
  suffix: string;
  info?: string;
}) {
  return (
    <KpiTile
      label={label}
      info={info}
      value={trend.value === null ? "—" : format(trend.value)}
      trend={trend.series}
      comparison={
        trend.delta !== null ? (
          <Delta value={trend.delta} format={deltaFormat} suffix={suffix} />
        ) : (
          <span className="text-xs text-ink-faint">no earlier games</span>
        )
      }
    />
  );
}

function RankTile({ calibration }: { calibration: CalibrationResponse | null }) {
  const rank = calibration?.established_rank;
  const momentum = calibration?.momentum;
  const hasMomentum = momentum && momentum.points.length > 0;

  return (
    <KpiTile
      label="Rank · est."
      info="Your medal as Dota reports it. The MMR is the middle of that medal's band, and the change is modeled from your last ranked games — estimates, not Valve's numbers."
      value={rank?.label ?? "—"}
      trend={hasMomentum ? momentum.points.map((p) => p.cumulative) : undefined}
      comparison={
        rank?.mmr ? (
          <span className="flex items-baseline gap-2">
            <span className="font-mono text-xs tabular-nums text-ink-muted">
              ~{rank.mmr.midpoint.toLocaleString("en-US")}
            </span>
            {hasMomentum ? (
              <Delta value={momentum.net} suffix={`est. · ${momentum.points.length} ranked`} />
            ) : null}
          </span>
        ) : (
          <span className="text-xs text-ink-faint">{calibration ? "no medal reported" : " "}</span>
        )
      }
    />
  );
}
