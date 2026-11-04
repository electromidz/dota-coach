"use client";

import { ComparisonContext } from "@/components/benchmark/ComparisonContext";
import { PercentileRow } from "@/components/charts/PercentileRow";
import { PercentileTrend } from "@/components/charts/PercentileTrend";
import { RadarCompare } from "@/components/charts/RadarCompare";
import { ItemTimeline } from "@/components/matches/ItemTimeline";
import { LaningPanel } from "@/components/matches/MatchKpis";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { InfoTip } from "@/components/ui/InfoTip";
import { shortLabel } from "@/lib/benchmark-view";
import type { PercentilePoint } from "@/lib/benchmark-view";
import type { MatchComparisonResponse } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * This match against players in the same rank bracket on the same hero, as
 * charts. Every number arrives computed server-side; nothing here derives a
 * figure from another one.
 */
export function MatchComparison({
  data,
  durationSeconds,
}: {
  data: MatchComparisonResponse;
  durationSeconds: number;
}) {
  const { bracket } = data;
  const points = radarPoints(data);

  return (
    <section className="flex flex-col gap-4" aria-label="Against your rank">
      {data.note ? <Alert tone="info">{data.note}</Alert> : null}

      {data.metrics.length > 0 ? (
        <div className="grid gap-4 lg:grid-cols-5 lg:items-start">
          <Card className={cn("flex flex-col gap-5", points.length >= 3 ? "lg:col-span-3" : "lg:col-span-5")}>
            <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
              <h3 className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
                Every metric, same scale
                <InfoTip label="About this chart">
                  Each metric as a percentile among {bracket.label} players on{" "}
                  {data.hero_name}, so different units share one axis. Further
                  right is better, deaths included.
                </InfoTip>
              </h3>
              <MarkLegend />
            </div>

            {data.metrics.map((row) => (
              <PercentileRow key={row.metric} row={row} />
            ))}

            <ComparisonContext context={data.context} className="border-t border-border pt-4" />
          </Card>

          {points.length >= 3 ? (
            <Card className="flex flex-col gap-3 lg:col-span-2">
              <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
                Shape of the game
              </h3>
              <RadarCompare
                points={points}
                rankLabel={bracket.label}
                targetLabel="Your average"
                youLegend="This match"
                targetLegend={`Your ${data.hero_name} average`}
              />
            </Card>
          ) : null}
        </div>
      ) : null}

      <div className="grid gap-4 lg:grid-cols-3 lg:items-start">
        {data.trend.length > 0 ? (
          <Card className="flex flex-col gap-3">
            <div className="flex items-baseline justify-between gap-3">
              <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
                Form on {data.hero_name}
              </h3>
              <Delta value={data.delta_vs_previous} />
            </div>
            <PercentileTrend points={data.trend} />
          </Card>
        ) : null}

        {data.laning.length > 0 ? (
          <Card className="flex flex-col gap-3">
            <h3 className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
              Laning
              <InfoTip label="About laning">
                At ten minutes, against your own average on this hero and
                position. No peer figure is published for these, so none is
                shown.
              </InfoTip>
            </h3>
            <LaningPanel readings={data.laning} />
            <p className="flex items-center gap-1.5 text-[0.6875rem] text-ink-faint">
              <span aria-hidden className="h-2.5 w-0.5 rounded bg-rank" />
              your average
            </p>
          </Card>
        ) : null}

        {data.item_timings.length > 0 ? (
          <Card className="flex flex-col gap-3">
            <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
              Item timings
            </h3>
            <ItemTimeline
              timings={data.item_timings}
              durationSeconds={durationSeconds}
              heroName={data.hero_name}
            />
          </Card>
        ) : null}
      </div>
    </section>
  );
}

/** The comparison as radar axes: this match, the bracket median, your average. */
export function radarPoints(data: MatchComparisonResponse): PercentilePoint[] {
  return data.metrics
    .filter((m) => m.this_match?.percentile !== null && m.this_match?.percentile !== undefined)
    .map((m) => ({
      axis: shortLabel(m.metric, m.label),
      label: m.label,
      metric: m.metric,
      higherIsBetter: m.higher_is_better,
      you: m.this_match!.percentile,
      rankMedian: 50,
      targetMedian: m.hero_average?.percentile ?? null,
      raw: {
        you: m.this_match!.value,
        rankMedian: m.peer_median,
        targetMedian: m.hero_average?.value ?? null,
      },
    }));
}

/** Placeholder shaped like the section. */
export function ComparisonSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true">
      <div className="grid gap-4 lg:grid-cols-5">
        <div className="h-96 animate-pulse rounded-card bg-surface-2 lg:col-span-3" />
        <div className="aspect-square animate-pulse rounded-card bg-surface-2 lg:col-span-2 lg:aspect-auto lg:h-96" />
      </div>
      <div className="grid gap-4 lg:grid-cols-3">
        <div className="h-40 animate-pulse rounded-card bg-surface-2" />
        <div className="h-40 animate-pulse rounded-card bg-surface-2" />
        <div className="h-40 animate-pulse rounded-card bg-surface-2" />
      </div>
    </div>
  );
}

/**
 * Change against the previous game on this hero, in percentile points. Signed
 * text as well as colour, so the sign survives a colour-vision deficiency.
 */
function Delta({ value }: { value: number | null }) {
  if (value === null) {
    return <span className="text-[0.6875rem] text-ink-faint">first game</span>;
  }
  const rounded = Math.round(value);
  if (rounded === 0) {
    return (
      <span className="rounded bg-mark-track px-1.5 py-0.5 font-mono text-[0.625rem] tabular-nums text-ink-muted">
        level with last game
      </span>
    );
  }
  return (
    <span
      className={cn(
        "rounded px-1.5 py-0.5 font-mono text-[0.625rem] tabular-nums",
        rounded > 0 ? "bg-mark-win/15 text-string" : "bg-mark-loss/15 text-error",
      )}
    >
      {rounded > 0 ? `+${rounded}` : rounded} vs your last game
    </span>
  );
}

/** Names the marks, so shape is never the only thing carrying identity. */
function MarkLegend() {
  return (
    <span className="flex flex-wrap items-center gap-3 text-[0.6875rem] text-ink-faint">
      <span className="inline-flex items-center gap-1.5">
        <span aria-hidden className="size-2.5 rounded-full bg-mark-line" />
        this match
      </span>
      <span className="inline-flex items-center gap-1.5">
        <span aria-hidden className="size-2.5 rounded-full border-2 border-ink-muted bg-surface-2" />
        your average
      </span>
      <span className="inline-flex items-center gap-1.5">
        <span aria-hidden className="h-2.5 w-px bg-rank" />
        p50
      </span>
      <span className="inline-flex items-center gap-1.5">
        <span aria-hidden className="h-2.5 w-0.5 rounded bg-target/80" />
        p80
      </span>
    </span>
  );
}
