"use client";

import Link from "next/link";
import { useState } from "react";

import { RadarCompare } from "@/components/charts/RadarCompare";
import { TrendLine, type TrendPoint } from "@/components/charts/TrendLine";
import { ChartCard } from "@/components/ui/ChartCard";
import { ScopeLine } from "@/components/ui/ScopeLine";
import { Segmented } from "@/components/ui/Segmented";
import { heroPortraitUrl } from "@/lib/heroes";
import { skillPoints } from "@/lib/overview";
import type { BenchmarkResponse, MatchView, OverviewTrends } from "@/lib/types";
import { cn, shortDate } from "@/lib/utils";

/** Skill radar: you against your rank's median, one benchmark metric per axis. */
export function SkillRadar({ benchmark }: { benchmark: BenchmarkResponse }) {
  const points = skillPoints(benchmark.results);
  const bracket = benchmark.context.bracket.label;

  return (
    <ChartCard
      title="Skills"
      info={`Each axis is your percentile among ${bracket} players on ${benchmark.hero_name}: Farming is GPM, Fighting hero damage, Supporting assists, Pushing tower damage, Survival deaths (fewer is better). The dashed ring is the ${bracket} median. Vision has no data source yet.`}
      action={
        <Link href="/benchmark" className="focus-neon rounded text-xs text-function hover:text-ink">
          Benchmark →
        </Link>
      }
    >
      <ScopeLine
        hero={benchmark.hero_name}
        role={benchmark.context.role_label}
        matches={benchmark.sample}
        className="-mt-2"
      />
      {points.length >= 3 ? (
        <RadarCompare points={points} rankLabel={bracket} />
      ) : (
        <p className="text-sm text-ink-faint">
          Needs more games on {benchmark.hero_name} to place you on enough skills.
        </p>
      )}
    </ChartCard>
  );
}

type TrendTab = "kda" | "gpm" | "win_rate" | "rating";

const TABS: { value: TrendTab; label: string }[] = [
  { value: "kda", label: "KDA" },
  { value: "gpm", label: "GPM" },
  { value: "win_rate", label: "Win rate" },
  { value: "rating", label: "Rating" },
];

/**
 * Performance over the latest games, one metric at a time. Per-game values
 * are the stored ones; the win rate line is the server's rolling series.
 */
export function PerformanceTrend({
  matches,
  trends,
}: {
  /** Newest first, eligible games only. */
  matches: MatchView[];
  trends: OverviewTrends;
}) {
  const [tab, setTab] = useState<TrendTab>("kda");
  const games = [...matches].reverse();
  const tip = (m: MatchView) => `${m.hero_name} · ${shortDate(m.started_at)} · ${m.won ? "Won" : "Lost"}`;

  let points: TrendPoint[];
  let format: (v: number) => string;
  let reference: { value: number; label: string } | undefined;

  if (tab === "win_rate") {
    // The rolling series ends at the newest game; label each point with the
    // game it ends on.
    const series = trends.win_rate.series;
    const tail = games.slice(-series.length);
    points = series.map((value, i) => ({
      value,
      label: tail[i] ? `${trends.rolling} games to ${tip(tail[i])}` : `${trends.rolling}-game win rate`,
    }));
    format = (v) => `${Math.round(v)}%`;
    reference = { value: 50, label: "50%" };
  } else {
    const pick = (m: MatchView) =>
      tab === "kda" ? m.kda : tab === "gpm" ? m.gpm : m.rating;
    points = games.flatMap((m) => {
      const value = pick(m);
      return value === null || value === undefined ? [] : [{ value, label: tip(m) }];
    });
    format =
      tab === "gpm"
        ? (v) => Math.round(v).toLocaleString("en-US")
        : tab === "rating"
          ? (v) => v.toFixed(0)
          : (v) => v.toFixed(2);
  }

  return (
    <ChartCard
      title="Performance trend"
      info="Your last games, oldest on the left. Rating compares each game with your usual game on that hero. The win rate line is a rolling average, so each point covers several games."
      action={<Segmented label="Metric" value={tab} onChange={setTab} options={TABS} />}
      className="lg:col-span-2"
    >
      <TrendLine
        key={tab}
        points={points}
        label={TABS.find((t) => t.value === tab)!.label}
        format={format}
        reference={reference}
        height={320}
      />
    </ChartCard>
  );
}

/** The last 20 games as win/loss bars, each with its hero underneath. */
export function LastGamesStrip({ matches }: { matches: MatchView[] }) {
  const games = [...matches].reverse();
  const wins = games.filter((m) => m.won).length;

  return (
    <ChartCard
      title={`Last ${games.length}`}
      action={
        <span className="font-mono text-xs tabular-nums text-ink-muted">
          {wins}W – {games.length - wins}L
        </span>
      }
    >
      <ol
        className="grid gap-0.5"
        style={{ gridTemplateColumns: `repeat(${games.length}, minmax(0, 1fr))` }}
        aria-label={`Last ${games.length} games, oldest first: ${wins} won, ${games.length - wins} lost`}
      >
        {games.map((m) => (
          <li key={m.id} className="min-w-0">
            <Link
              href={`/matches/${m.id}`}
              title={`${m.hero_name} · ${m.won ? "Won" : "Lost"} · ${m.kills}/${m.deaths}/${m.assists} · ${shortDate(m.started_at)}`}
              className="focus-neon flex flex-col items-stretch gap-1 rounded"
            >
              <span className="flex h-8 items-end">
                <span
                  className={cn(
                    "w-full rounded-sm",
                    m.won ? "h-8 bg-mark-win" : "h-4 bg-mark-loss",
                  )}
                />
              </span>
              {/* Decorative: the link's title names the hero. Hidden on a
                  phone, where 20 portraits would be a few pixels each. */}
              <Portrait heroId={m.hero_id} />
            </Link>
          </li>
        ))}
      </ol>
    </ChartCard>
  );
}

function Portrait({ heroId }: { heroId: number }) {
  const src = heroPortraitUrl(heroId);
  if (!src) return <span className="hidden aspect-[16/9] w-full rounded-[2px] bg-surface-2 sm:block" />;
  return (
    // eslint-disable-next-line @next/next/no-img-element
    <img
      src={src}
      alt=""
      loading="lazy"
      className="hidden aspect-[16/9] w-full rounded-[2px] object-cover sm:block"
    />
  );
}
