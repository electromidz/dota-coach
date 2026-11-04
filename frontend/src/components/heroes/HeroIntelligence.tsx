"use client";

import { useEffect, useState } from "react";

import { Paywall } from "@/components/billing/Paywall";
import { FitMetaChart } from "@/components/heroes/FitMetaChart";
import { MetaBars } from "@/components/heroes/MetaBars";
import { PoolGrid } from "@/components/heroes/PoolGrid";
import { RecommendedRow } from "@/components/heroes/RecommendedRow";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { InfoTip } from "@/components/ui/InfoTip";
import { Segmented } from "@/components/ui/Segmented";
import { ApiError, getHeroIntelligence } from "@/lib/api";
import { formatScore } from "@/lib/hero-intel";
import { useSession } from "@/lib/session-context";
import type { CoachableRole, HeroIntelligenceResponse } from "@/lib/types";

/**
 * Hero Intelligence.
 *
 * The order is the product's argument, not a layout preference: what to play,
 * why, then the repertoire it was judged against, then the meta it was scored
 * in. The meta comes last deliberately — it is an input to the answer, not the
 * answer.
 */
export function HeroIntelligence() {
  const { session } = useSession();
  const [data, setData] = useState<HeroIntelligenceResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [paywalled, setPaywalled] = useState(false);
  /** Absent follows the coaching role; "all" widens to every role. */
  const [role, setRole] = useState<CoachableRole | "all" | undefined>();
  /** Hero picked on the chart, ringed in the pool. */
  const [highlight, setHighlight] = useState<number | null>(null);

  useEffect(() => {
    // A failed profile read is reported once by the shell; this page
    // still loads its own data.
    if (session.kind === "loading" || session.kind === "anonymous") return;

    getHeroIntelligence(undefined, role)
      .then((response) => {
        setData(response);
        setError(null);
      })
      .catch((e) => {
        if (e instanceof ApiError && e.isPaymentRequired) {
          setPaywalled(true);
          return;
        }
        setError(
          e instanceof ApiError
            ? e.message
            : "Could not load your hero intelligence.",
        );
      });
  }, [session.kind, role]);

  if (session.kind === "loading") return <HeroSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;

  if (paywalled) {
    return (
      <Paywall message="Hero recommendations and your hero pool need an active trial or subscription." />
    );
  }
  if (error) return <Alert>{error}</Alert>;
  if (!data) return <HeroSkeleton />;

  const { scope } = data;
  const inPool = new Set(data.pool.map((h) => h.hero_id));
  const plottable = data.pool.filter(
    (h) => h.win_rate_delta !== null && h.meta_strength !== null,
  ).length;

  function select(heroId: number) {
    setHighlight(heroId);
    document
      .getElementById(`hero-${heroId}`)
      ?.scrollIntoView({ behavior: "smooth", block: "center" });
  }

  return (
    <div className="flex flex-col gap-8 pb-4">
      <div className="flex flex-wrap items-center gap-3">
        {scope.coaching_role ? (
          <Segmented
            label="Which games"
            value={scope.role ? "role" : "all"}
            onChange={(next) => {
              setData(null);
              setRole(next === "all" ? "all" : undefined);
            }}
            options={[
              { value: "role", label: scope.coaching_role_label ?? "My role" },
              { value: "all", label: "All roles" },
            ]}
          />
        ) : null}
        <p className="font-mono text-xs text-ink-faint">
          {scope.role_label ?? "All roles"} · {scope.population} · last {scope.window}
        </p>
      </div>

      {data.note ? <Alert tone="info">{data.note}</Alert> : null}
      {!data.meta.available && data.meta.note ? (
        <p className="truncate text-xs text-warn">{data.meta.note}</p>
      ) : null}

      {plottable > 0 ? (
        <section className="flex flex-col gap-3" aria-label="Fit vs meta">
          <h2 className="flex items-center gap-2 font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
            Fit vs meta
            <InfoTip label="About this chart">
              <span>
                Across: your win rate on the hero against your own average, after
                small samples are pulled toward that average. Up: how strong the hero
                is in the current meta. Size: games played.
              </span>
              {data.pool.length > plottable ? (
                <span className="text-ink-faint">
                  {data.pool.length - plottable} heroes with fewer than 3 games are not
                  plotted.
                </span>
              ) : null}
            </InfoTip>
          </h2>
          <FitMetaChart pool={data.pool} onSelect={select} />
        </section>
      ) : null}

      {data.recommendations.length > 0 ? (
        <section className="flex flex-col gap-3" aria-label="Recommended for you">
          <h2 className="flex items-center gap-2 font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
            Recommended for you
            <InfoTip label="How heroes are recommended">
              Heroes with 5 or more games, ranked by fit score
              {data.meta.bracket_label ? `, scored against ${data.meta.bracket_label}` : ""}.
            </InfoTip>
          </h2>
          <RecommendedRow fits={data.recommendations} pool={data.pool} />
        </section>
      ) : null}

      {data.worth_trying.length > 0 ? (
        <section className="flex flex-col gap-3" aria-label="Worth trying">
          <h2 className="flex items-center gap-2 font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
            Worth trying
            <InfoTip label="About worth trying">
              Strong in the meta, but fewer than 5 of your games on them.
            </InfoTip>
          </h2>
          <ul className="flex flex-wrap gap-2">
            {data.worth_trying.map((fit) => (
              <li
                key={fit.hero_id}
                className="glass flex items-center gap-2 rounded-xl py-1.5 pl-1.5 pr-2.5"
              >
                <HeroPortrait heroId={fit.hero_id} heroName={fit.hero_name} size="sm" />
                <span className="flex min-w-0 flex-col">
                  <span className="truncate text-xs text-ink">{fit.hero_name}</span>
                  <span className="text-[0.6875rem] text-ink-faint">
                    {fit.matches} {fit.matches === 1 ? "game" : "games"}
                  </span>
                </span>
                <span
                  className="rounded-md bg-you/15 px-1.5 py-0.5 font-mono text-[0.6875rem] tabular-nums text-ink"
                  title="Meta strength"
                >
                  {formatScore(fit.meta_strength)}
                </span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      {data.pool.length > 0 ? <PoolGrid pool={data.pool} highlight={highlight} /> : null}

      {data.meta_leaders.length > 0 ? (
        <MetaBars leaders={data.meta_leaders} meta={data.meta} inPool={inPool} />
      ) : null}
    </div>
  );
}

function HeroSkeleton() {
  return (
    <div className="flex flex-col gap-8" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading hero intelligence…</span>
      <div className="h-9 w-72 animate-pulse rounded-xl bg-surface-2" />
      <div className="relative h-72 w-full animate-pulse rounded-xl bg-surface-2/60 sm:h-80">
        <span className="absolute inset-y-0 left-1/2 w-px bg-border" />
        <span className="absolute inset-x-0 top-1/2 h-px bg-border" />
      </div>
      <div className="flex gap-3 overflow-hidden">
        {[0, 1, 2, 3].map((i) => (
          <div key={i} className="glass flex h-36 w-56 shrink-0 flex-col gap-3 rounded-card p-3">
            <div className="flex items-center gap-2">
              <div className="h-9 w-16 animate-pulse rounded-lg bg-surface-2" />
              <div className="h-3 flex-1 animate-pulse rounded bg-surface-2" />
              <div className="size-11 animate-pulse rounded-full bg-surface-2" />
            </div>
            <div className="h-2 w-full animate-pulse rounded-full bg-surface-2" />
          </div>
        ))}
      </div>
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
        {Array.from({ length: 10 }, (_, i) => (
          <div key={i} className="glass flex h-12 items-center gap-2 rounded-xl p-2">
            <div className="h-7 w-12 animate-pulse rounded bg-surface-2" />
            <div className="h-2.5 flex-1 animate-pulse rounded bg-surface-2" />
            <div className="size-8 animate-pulse rounded-full bg-surface-2" />
          </div>
        ))}
      </div>
    </div>
  );
}
