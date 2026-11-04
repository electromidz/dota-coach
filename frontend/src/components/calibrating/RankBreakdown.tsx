"use client";

import Link from "next/link";

import { Donut, type DonutSlice } from "@/components/charts/Donut";
import { LegendItem, RadarCompare } from "@/components/charts/RadarCompare";
import { ChartCard } from "@/components/ui/ChartCard";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { ScopeLine } from "@/components/ui/ScopeLine";
import { toPercentileVector } from "@/lib/benchmark-view";
import type { CalibrationResponse, CoachableRole, MomentumPoint, RankDistribution } from "@/lib/types";
import { cn, shortDate } from "@/lib/utils";

/** Positions are ordered, so one hue stepped by position, strongest first;
 *  every slice is also named in the legend. */
const POSITION_SHADE: Record<CoachableRole, number> = {
  carry: 100,
  mid: 80,
  offlane: 62,
  soft_support: 46,
  hard_support: 32,
};

/** Against your bracket: the same percentiles Benchmark shows, as a radar. */
export function BracketRadar({ brackets }: { brackets: RankDistribution }) {
  const points = toPercentileVector(
    brackets.own_bracket_metrics.filter((m) => m.percentile !== null),
    null,
  );
  const consistency = brackets.consistency;

  return (
    <ChartCard
      title="Against your bracket"
      info="Your percentile on each metric among players of your own medal, on your most-played hero in this scope. The same numbers as Benchmark. Consistency is how steady your KDA is from game to game (100 = every game alike), not how good the games were."
      action={
        <Link href="/benchmark" className="focus-neon rounded text-xs text-function hover:text-ink">
          Full breakdown → Benchmark
        </Link>
      }
    >
      <ScopeLine hero={brackets.hero_name} role={brackets.role_label} matches={brackets.sample} className="-mt-2" />
      {points.length >= 3 ? (
        <>
          <RadarCompare points={points} rankLabel="Your medal" />
          <div className="flex flex-wrap justify-center gap-4 text-xs text-ink-muted">
            <LegendItem kind="you">You</LegendItem>
            <LegendItem kind="rank">Your medal&apos;s median</LegendItem>
          </div>
        </>
      ) : (
        <p className="text-sm text-ink-faint">Needs more games on {brackets.hero_name} to place you on enough metrics.</p>
      )}
      <p className="font-mono text-xs tabular-nums text-ink-muted">
        Consistency {consistency ? `${Math.round(consistency.percentage)} / 100 · ${consistency.matches} games` : "· needs 10 ranked games"}
      </p>
    </ChartCard>
  );
}

/** Roles donut, ranked win rate by hero, and rating against result. */
export function RankBreakdown({ data }: { data: CalibrationResponse }) {
  const slices: DonutSlice[] = data.role_preference.map((r) => ({
    key: r.position ?? "unclassified",
    label: r.position ? r.role : "Unclassified",
    value: r.matches,
    display: `${Math.round(r.pct)}%`,
    color: r.position
      ? `color-mix(in oklch, var(--color-you) ${POSITION_SHADE[r.position]}%, transparent)`
      : "var(--color-rank)",
  }));
  const ranked = data.role_preference.reduce((sum, r) => sum + r.matches, 0);

  return (
    <div className="grid gap-4 lg:grid-cols-3">
      <ChartCard title="Positions" info="Ranked games by position. Unclassified games had too little data to place.">
        {slices.length > 0 ? (
          <Donut slices={slices} label="Ranked games by position" center={{ value: String(ranked), caption: "ranked games" }} />
        ) : (
          <p className="text-sm text-ink-faint">No ranked games yet.</p>
        )}
      </ChartCard>

      <ChartCard title="Ranked win rate by hero" info="Your most-played heroes in ranked, with the 50% mark.">
        {data.hero_win_rates.length > 0 ? (
          <ul className="flex flex-col gap-2.5">
            {data.hero_win_rates.map((h) => (
              <li key={h.hero_id} className="flex items-center gap-2.5">
                <HeroPortrait heroId={h.hero_id} heroName={h.hero_name} size="sm" className="!h-6 !w-11 shrink-0" />
                <span className="flex min-w-0 flex-1 flex-col gap-1">
                  <span className="flex items-baseline justify-between gap-2 text-xs">
                    <span className="truncate text-ink-muted">{h.hero_name}</span>
                    <span className="shrink-0 font-mono tabular-nums text-ink-faint">
                      {h.wins}–{h.matches - h.wins} · {Math.round(h.win_rate)}%
                    </span>
                  </span>
                  <span className="relative block h-1.5 rounded-full bg-mark-track">
                    <span
                      className={cn("block h-full rounded-full", h.win_rate >= 50 ? "bg-mark-win" : "bg-mark-loss")}
                      style={{ width: `${h.win_rate}%` }}
                    />
                    <span aria-hidden className="absolute -inset-y-0.5 left-1/2 w-0.5 rounded bg-ink-faint" />
                  </span>
                </span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-sm text-ink-faint">No ranked games yet.</p>
        )}
      </ChartCard>

      <ChartCard title="Rating vs result" info="Each dot is one of your recent ranked games, placed by its rating (1-10 against your usual game on that hero). Wins on the top lane, losses below.">
        <RatingStrip points={data.momentum.points} />
      </ChartCard>
    </div>
  );
}

/** Two lanes, won and lost, with each game at its rating on 1-10. */
function RatingStrip({ points }: { points: MomentumPoint[] }) {
  const rated = points.filter((p) => p.rating !== null);
  if (rated.length === 0) return <p className="text-sm text-ink-faint">No rated ranked games yet.</p>;

  const at = (rating: number) => `${((rating - 1) / 9) * 100}%`;
  return (
    <div className="flex flex-col gap-2" role="img" aria-label={`Rating against result for ${rated.length} ranked games`}>
      {[true, false].map((won) => (
        <div key={String(won)} className="flex items-center gap-2">
          <span className="w-9 shrink-0 text-[0.6875rem] text-ink-faint">{won ? "Won" : "Lost"}</span>
          <span className="relative block h-7 flex-1 rounded-md bg-surface-2/50">
            {rated
              .filter((p) => p.won === won)
              .map((p) => (
                <Link
                  key={p.id}
                  href={`/matches/${p.id}`}
                  title={`${p.hero_name} · rating ${p.rating!.toFixed(1)} · ${shortDate(p.started_at)}`}
                  className={cn(
                    "absolute top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-surface-2",
                    won ? "bg-mark-win" : "bg-mark-loss",
                  )}
                  style={{ left: at(p.rating!) }}
                />
              ))}
          </span>
        </div>
      ))}
      <div className="ml-11 flex justify-between font-mono text-[0.625rem] tabular-nums text-ink-faint">
        <span>1</span>
        <span>5.5</span>
        <span>10</span>
      </div>
    </div>
  );
}
