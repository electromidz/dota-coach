"use client";

import { useCallback, useEffect, useState } from "react";

import { ComparisonContext } from "@/components/benchmark/ComparisonContext";
import { Paywall } from "@/components/billing/Paywall";
import { BulletBar } from "@/components/charts/BulletBar";
import { formatMetricValue } from "@/components/charts/BulletRow";
import { Delta } from "@/components/charts/Delta";
import { DivergingBars } from "@/components/charts/DivergingBars";
import { KpiTile, PipMeter } from "@/components/charts/KpiTile";
import { LegendItem, RadarCompare } from "@/components/charts/RadarCompare";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { ChartCard } from "@/components/ui/ChartCard";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { Icon } from "@/components/ui/Icon";
import { InfoTip } from "@/components/ui/InfoTip";
import { Segmented } from "@/components/ui/Segmented";
import { ApiError, getBenchmark, getStats } from "@/lib/api";
import {
  averagePercentile,
  extremes,
  gapToTarget,
  shortLabel,
  targetByMetric,
  toPercentileVector,
} from "@/lib/benchmark-view";
import { CONFIDENCE_NOTE } from "@/lib/confidence";
import { useSession } from "@/lib/session-context";
import { formatPercent } from "@/lib/stats";
import type {
  BenchmarkResponse,
  CoachableRole,
  HeroStats,
  RankBracket,
} from "@/lib/types";
import { cn, ordinal } from "@/lib/utils";

/** Below this many matches the sample tile carries a warning badge. */
const LOW_SAMPLE_BADGE = 20;
/** Mirrors the server's floor for ranking a figure (`percentile::MIN_SAMPLE`). */
const MIN_SAMPLE = 5;

/**
 * Where the chosen bracket lives between page views.
 *
 * `sessionStorage`, not the database: which peer group someone is curious about
 * right now is a browsing state, and the product has no user-preference table
 * to put it in. It also dies with the tab, which is the right lifetime — the
 * bracket a player wants to see defaults back to the next rung up tomorrow.
 */
const BRACKET_KEY = "benchmark.bracket";

/**
 * What the picker is set to.
 *
 * `undefined` is not "no target" — it asks the server for the next bracket up,
 * which is what the page shows on arrival. `"none"` is the opt-out. Keeping the
 * two apart is what lets the default be server-chosen without the client having
 * to know the rank order.
 */
type BracketChoice = RankBracket | "none" | undefined;

function savedBracket(): BracketChoice {
  // Absent during server rendering, and can be disabled outright in the
  // browser. Not being able to remember the choice is not a reason to fail the
  // page, and the server's answer — the default target — is also the initial
  // markup, so there is nothing for hydration to disagree about.
  if (typeof window === "undefined") return undefined;

  try {
    return (sessionStorage.getItem(BRACKET_KEY) as BracketChoice) ?? undefined;
  } catch {
    return undefined;
  }
}

export function Benchmark() {
  const { session } = useSession();
  const [heroes, setHeroes] = useState<HeroStats[]>([]);
  const [data, setData] = useState<BenchmarkResponse | null>(null);
  const [selected, setSelected] = useState<number | undefined>();
  const [error, setError] = useState<string | null>(null);
  const [paywalled, setPaywalled] = useState(false);
  const [loading, setLoading] = useState(false);
  /** `undefined` follows the coaching role; `"all"` opts out of role scoping. */
  const [roleScope, setRoleScope] = useState<CoachableRole | "all" | undefined>();
  /** Which bracket to aim at. Restored on the first render rather than in an
   *  effect, so the picker never shows one bracket over another's numbers for
   *  a frame. */
  const [bracket, setBracket] = useState<BracketChoice>(savedBracket);

  const load = useCallback(
    async (
      heroId?: number,
      role?: CoachableRole | "all",
      target?: BracketChoice,
    ) => {
      setLoading(true);
      try {
        // Replaced wholesale rather than merged: a bracket switch changes every
        // peer number on the page, and a half-updated response would show one
        // bracket's medians under another's heading.
        setData(await getBenchmark(heroId, role, target));
        setError(null);
        setPaywalled(false);
      } catch (e) {
        if (e instanceof ApiError && e.isPaymentRequired) {
          setPaywalled(true);
        } else {
          setError(
            e instanceof ApiError
              ? e.message
              : "Could not load your benchmarks.",
          );
        }
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  useEffect(() => {
    // A failed profile read is reported once by the shell; this page
    // still loads its own data.
    if (session.kind === "loading" || session.kind === "anonymous") return;

    // The hero list drives the picker; the benchmark defaults to the
    // most-played, which is the only hero likely to clear the sample floor.
    getStats()
      .then((stats) => setHeroes(stats.heroes))
      .catch(() => setHeroes([]));

    // Read again rather than carried over from the initializer: the state
    // above already holds it, and passing it here keeps the restore to a
    // single request instead of a default one followed by a correction.
    void load(undefined, undefined, savedBracket());
  }, [session.kind, load]);

  if (session.kind === "loading") return <BenchmarkSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;

  if (paywalled) {
    return (
      <Paywall message="Benchmarking your figures against other players needs an active trial or subscription." />
    );
  }
  if (error) return <Alert>{error}</Alert>;
  if (!data) return <BenchmarkSkeleton />;

  async function pick(heroId: number) {
    setSelected(heroId);
    await load(heroId, roleScope, bracket);
  }

  async function setScope(role: CoachableRole | "all" | undefined) {
    setRoleScope(role);
    // The hero resets with the scope: the most-played hero in one role is
    // frequently not the most-played in another, and keeping a stale pick would
    // silently benchmark a hero the new scope barely contains.
    setSelected(undefined);
    await load(undefined, role, bracket);
  }

  /** The hero and role stay put: only the bracket being aimed at changes. */
  async function setTargetBracket(next: BracketChoice) {
    setBracket(next);
    try {
      if (next) sessionStorage.setItem(BRACKET_KEY, next);
      else sessionStorage.removeItem(BRACKET_KEY);
    } catch {
      // See `savedBracket` — remembering is a nicety, not a requirement.
    }
    await load(selected, roleScope, next);
  }

  const confidence = data.results[0]?.confidence;
  const caveat = confidence ? CONFIDENCE_NOTE[confidence] : "";
  /** The bracket the player's own figures are measured against. Never moves. */
  const resolved = data.context.bracket;
  const ownRank = data.brackets.find((option) => option.is_player_rank);
  const target = data.target;
  const activeHero = heroes.find(
    (hero) => hero.hero_id === (selected ?? data.hero_id),
  );

  /**
   * A bracket was named and nothing came back for it. Only worth saying when
   * the reader picked it — the server's own default quietly picking nothing is
   * not a failed request, and Immortal players would otherwise be told off for
   * having no rank above them.
   */
  const targetMissing =
    bracket !== undefined && bracket !== "none" && target === null;

  const points = toPercentileVector(data.results, target);
  const ranked = points.filter((point) => point.you !== null);
  const best = extremes(points);
  const gaps = gapToTarget(data.results, target);
  const average = averagePercentile(data.results);
  const aims = targetByMetric(target);
  const lowSample = data.sample < LOW_SAMPLE_BADGE;

  return (
    <div className={cn("flex flex-col gap-4 pb-4", loading && "opacity-60 transition-opacity")}>
      {heroes.length > 1 ? (
        <nav aria-label="Choose a hero" className="-mx-1 flex gap-2 overflow-x-auto px-1 pb-1">
          {heroes.map((hero) => {
            const active = (selected ?? data.hero_id) === hero.hero_id;
            return (
              <button
                key={hero.hero_id}
                type="button"
                onClick={() => pick(hero.hero_id)}
                aria-current={active ? "true" : undefined}
                aria-label={`${hero.hero_name}, ${hero.matches} matches`}
                title={hero.hero_name}
                className={cn(
                  "focus-neon relative shrink-0 cursor-pointer rounded-lg transition-opacity duration-200",
                  active ? "ring-2 ring-function" : "opacity-60 hover:opacity-100",
                )}
              >
                <HeroPortrait heroId={hero.hero_id} heroName={hero.hero_name} size="sm" />
                <span className="absolute bottom-0.5 right-0.5 rounded bg-base/85 px-1 font-mono text-[0.625rem] tabular-nums text-ink">
                  {hero.matches}
                </span>
              </button>
            );
          })}
        </nav>
      ) : null}

      <div className="flex flex-wrap items-center gap-2">
        {/* Which of the player's own matches are compared. */}
        {data.context.role || roleScope === "all" ? (
          <Segmented
            label="Comparison scope"
            value={roleScope === "all" ? "all" : "role"}
            onChange={(next) => void setScope(next === "all" ? "all" : undefined)}
            options={[
              { value: "role", label: data.context.role_label ?? "My role" },
              { value: "all", label: "All roles" },
            ]}
          />
        ) : null}

        {/* Which rank to aim at. */}
        <select
          aria-label="Aiming at"
          value={bracket ?? ""}
          onChange={(e) =>
            void setTargetBracket(
              e.target.value === "" ? undefined : (e.target.value as BracketChoice),
            )
          }
          className="focus-neon min-h-11 cursor-pointer rounded-xl border border-glass-edge bg-surface-2/60 px-3 text-xs text-ink outline-none"
        >
          <option value="">
            {target ? `Next rank up (${target.label})` : "Next rank up"}
          </option>
          {data.brackets.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
              {option.is_player_rank ? " — your rank" : ""}
            </option>
          ))}
          <option value="none">
            {ownRank ? `Your rank only (${ownRank.label})` : "Your rank only"}
          </option>
        </select>

        <span className="ml-auto flex items-center gap-1.5 text-xs text-ink-faint">
          vs {resolved.label}
          <InfoTip label="What this compares against" align="end">
            <ComparisonContext context={data.context} />
          </InfoTip>
        </span>
      </div>

      {targetMissing ? (
        <Alert tone="info" title="No data for that rank">
          Nothing published for{" "}
          {data.brackets.find((option) => option.value === bracket)?.label ??
            "that bracket"}
          ; nothing has been estimated.
        </Alert>
      ) : null}

      {data.note ? <Alert tone="info">{data.note}</Alert> : null}

      {data.results.length > 0 ? (
        <>
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
            {target ? (
              <KpiTile
                label={`Above ${target.label}`}
                value={
                  <>
                    {target.metrics_cleared}
                    <span className="text-ink-faint">/{target.metrics_compared}</span>
                  </>
                }
                comparison={
                  <PipMeter
                    filled={target.metrics_cleared}
                    total={target.metrics_compared}
                    label={`${target.metrics_cleared} of ${target.metrics_compared} metrics at or past the ${target.label} median`}
                  />
                }
                info={`Metrics where you already meet the ${target.label} median for ${data.hero_name}.`}
              />
            ) : (
              <KpiTile
                label={`Above ${resolved.label}`}
                value={
                  <>
                    {ranked.filter((point) => (point.you ?? 0) >= 50).length}
                    <span className="text-ink-faint">/{ranked.length}</span>
                  </>
                }
                comparison={
                  <PipMeter
                    filled={ranked.filter((point) => (point.you ?? 0) >= 50).length}
                    total={ranked.length}
                    label={`Metrics at or above the ${resolved.label} median`}
                  />
                }
              />
            )}

            <KpiTile
              label="Avg percentile"
              value={average !== null ? ordinal(average) : "—"}
              comparison={
                average !== null ? (
                  <Delta value={Math.round(average) - 50} suffix={`vs ${resolved.label} p50`} />
                ) : (
                  <span className="text-xs text-ink-faint">Insufficient sample size</span>
                )
              }
              info={`Mean of your percentiles across ranked metrics, within ${resolved.label}.`}
            />

            <KpiTile
              label="Sample"
              value={
                <>
                  {data.sample}
                  <span className="ml-1.5 font-sans text-sm text-ink-faint">
                    {data.sample === 1 ? "match" : "matches"}
                  </span>
                </>
              }
              badge={
                lowSample ? (
                  <span className="flex items-center gap-1 rounded-md bg-warn/15 px-1.5 py-0.5 text-[0.625rem] font-semibold uppercase tracking-wide text-warn">
                    <Icon name="alert" className="size-3" />
                    Low
                  </span>
                ) : null
              }
              comparison={
                <span className="text-xs text-ink-faint">vs {resolved.label}</span>
              }
              info={caveat || undefined}
            />

            <KpiTile
              label="Hero"
              value={<span className="text-xl sm:text-2xl">{data.hero_name}</span>}
              visual={
                <HeroPortrait heroId={data.hero_id} heroName={data.hero_name} size="sm" />
              }
              comparison={
                <span className="text-xs text-ink-faint">
                  {data.context.role_label ?? "All roles"}
                  {activeHero ? ` · ${formatPercent(activeHero.win_rate)} WR` : ""}
                </span>
              }
            />
          </div>

          <div className="grid grid-cols-1 gap-4 lg:grid-cols-12">
            <ChartCard
              title={`${data.hero_name} vs ${resolved.label}`}
              className="lg:col-span-7"
              info="Each axis is your percentile in your own bracket (0–100). Deaths are scored so outward is always better. Hover an axis for raw values."
            >
              {ranked.length >= 3 ? (
                <>
                  <RadarCompare
                    points={ranked}
                    rankLabel={resolved.label}
                    targetLabel={target?.label}
                  />
                  {best ? (
                    <p className="mx-auto flex flex-wrap justify-center gap-2 text-xs">
                      <span className="rounded-full bg-string/10 px-2.5 py-1 text-ink">
                        Strongest: {best.strongest.axis}{" "}
                        <span className="font-mono text-string">{ordinal(best.strongest.you ?? 0)}</span>
                      </span>
                      <span className="rounded-full bg-error/10 px-2.5 py-1 text-ink">
                        Weakest: {best.weakest.axis}{" "}
                        <span className="font-mono text-error">{ordinal(best.weakest.you ?? 0)}</span>
                      </span>
                    </p>
                  ) : null}
                </>
              ) : (
                <EmptyRadar sample={data.sample} />
              )}
            </ChartCard>

            <ChartCard
              title={target ? `Gap to ${target.label}` : "Gap to next rank"}
              className="lg:col-span-5"
              info={
                target
                  ? `How far each metric sits above (+) or below (−) the ${target.label} median, as a share of that median. Deaths count fewer as better.`
                  : "Pick a rank to aim at to see the gap."
              }
            >
              {gaps.length > 0 ? (
                <DivergingBars
                  label={`Gap to ${target?.label} median by metric`}
                  rows={gaps.map((gap) => {
                    const result = data.results.find((r) => r.metric === gap.metric);
                    const aim = aims.get(gap.metric);
                    return {
                      key: gap.metric,
                      label: gap.axis,
                      value: gap.percent,
                      detail:
                        result && aim?.peer_median != null
                          ? `You ${formatMetricValue(result.player_value)} · ${target?.label} ${formatMetricValue(aim.peer_median)}`
                          : undefined,
                    };
                  })}
                />
              ) : (
                <p className="py-8 text-center text-xs text-ink-faint">No target rank selected</p>
              )}
            </ChartCard>

            <ChartCard
              title="Percentile ladder"
              className="lg:col-span-12"
              info={caveat || undefined}
              action={
                <ul className="hidden gap-3 text-[0.6875rem] text-ink-faint sm:flex">
                  <LegendItem kind="you">You</LegendItem>
                  <LegendTick className="bg-rank">{resolved.label}</LegendTick>
                  {target ? (
                    <LegendTick className="bg-target">{target.label}</LegendTick>
                  ) : null}
                </ul>
              }
            >
              <ul className="grid grid-cols-1 gap-x-8 gap-y-3 md:grid-cols-2">
                {data.results.map((result) => {
                  const aim = aims.get(result.metric);
                  return (
                    <BulletBar
                      key={result.metric}
                      label={shortLabel(result.metric, result.label)}
                      fullLabel={result.label}
                      value={result.player_value}
                      percentile={result.percentile}
                      rankMedian={result.peer_median}
                      targetMedian={aim?.peer_median ?? null}
                      cleared={aim ? aim.cleared : null}
                      shortfall={aim?.gap_to_median ?? null}
                      rankLabel={resolved.label}
                      targetLabel={target?.label}
                      note={result.note}
                    />
                  );
                })}
              </ul>
            </ChartCard>
          </div>
        </>
      ) : null}
    </div>
  );
}

function LegendTick({
  className,
  children,
}: {
  className: string;
  children: React.ReactNode;
}) {
  return (
    <li className="flex items-center gap-1.5">
      <span aria-hidden className={cn("h-3 w-1 rounded", className)} />
      {children}
    </li>
  );
}

function EmptyRadar({ sample }: { sample: number }) {
  return (
    <div className="flex flex-col items-center gap-3 py-6">
      <div className="relative aspect-square w-full max-w-[16rem] rounded-full border border-dashed border-mark-track" />
      {sample < MIN_SAMPLE ? (
        <>
          <p className="text-sm text-ink">Insufficient sample size</p>
          <p className="text-xs text-ink-faint">
            {sample}/{MIN_SAMPLE} matches to rank
          </p>
        </>
      ) : (
        <p className="text-sm text-ink">Too few ranked metrics to plot</p>
      )}
    </div>
  );
}

function BenchmarkSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading benchmarks…</span>
      <div className="flex gap-2">
        {Array.from({ length: 6 }, (_, i) => (
          <div key={i} className="h-9 w-16 animate-pulse rounded-lg bg-surface-2" />
        ))}
      </div>
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {Array.from({ length: 4 }, (_, i) => (
          <div key={i} className="glass flex h-28 flex-col gap-3 rounded-card p-4">
            <div className="h-2.5 w-20 animate-pulse rounded bg-surface-2" />
            <div className="h-7 w-16 animate-pulse rounded bg-surface-2" />
            <div className="h-1.5 w-full animate-pulse rounded bg-surface-2" />
          </div>
        ))}
      </div>
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-12">
        <div className="glass flex items-center justify-center rounded-card p-6 lg:col-span-7">
          <div className="aspect-square w-full max-w-[22rem] animate-pulse rounded-full bg-surface-2/70 [clip-path:polygon(50%_0,85%_15%,100%_50%,85%_85%,50%_100%,15%_85%,0_50%,15%_15%)]" />
        </div>
        <div className="glass flex flex-col justify-center gap-3 rounded-card p-5 lg:col-span-5">
          {[60, 40, 75, 30, 55, 45, 65, 35].map((w, i) => (
            <div key={i} className="grid grid-cols-[5.5rem_1fr] items-center gap-3">
              <div className="h-2.5 w-14 animate-pulse rounded bg-surface-2" />
              <div className="h-3 animate-pulse rounded-sm bg-surface-2" style={{ width: `${w}%`, marginLeft: i % 2 ? "50%" : `${50 - w / 2}%` }} />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
