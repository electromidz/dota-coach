"use client";

import { useEffect, useState } from "react";

import { HeroPool } from "@/components/heroes/HeroPool";
import { RecommendationCard } from "@/components/heroes/RecommendationCard";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { ApiError, getHeroIntelligence } from "@/lib/api";
import { formatScore, splitByLevel } from "@/lib/hero-intel";
import { useSession } from "@/lib/session-context";
import { formatPercent } from "@/lib/stats";
import type { HeroIntelligenceResponse } from "@/lib/types";

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

  useEffect(() => {
    if (session.kind !== "signed-in") return;

    getHeroIntelligence()
      .then((response) => {
        setData(response);
        setError(null);
      })
      .catch((e) =>
        setError(
          e instanceof ApiError
            ? e.message
            : "Could not load your hero intelligence.",
        ),
      );
  }, [session.kind]);

  if (session.kind === "loading") return <HeroSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;
  if (session.kind === "error") {
    return <Alert title="Cannot reach the service">{session.message}</Alert>;
  }

  if (error) return <Alert>{error}</Alert>;
  if (!data) return <HeroSkeleton />;

  const { leading, rest } = splitByLevel(data.recommendations);

  return (
    <div className="flex flex-col gap-8 pb-4">
      {data.note ? <Alert tone="info">{data.note}</Alert> : null}
      {!data.meta.available && data.meta.note ? (
        <Alert tone="info">{data.meta.note}</Alert>
      ) : null}

      {data.recommendations.length > 0 ? (
        <section className="flex flex-col gap-4">
          <header className="flex flex-wrap items-baseline justify-between gap-2">
            <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
              Recommended for you
            </h2>
            {data.meta.bracket_label ? (
              <p className="text-xs text-ink-faint">
                Scored against {data.meta.bracket_label}
              </p>
            ) : null}
          </header>

          {leading.map((fit) => (
            <RecommendationCard key={fit.hero_id} fit={fit} />
          ))}

          {rest.length > 0 ? (
            <details className="group">
              <summary className="focus-neon min-h-11 cursor-pointer list-none text-xs uppercase tracking-wider text-ink-faint transition-colors duration-200 hover:text-ink">
                {rest.length} more, not right now
              </summary>
              <div className="mt-4 flex flex-col gap-4">
                {rest.map((fit) => (
                  <RecommendationCard key={fit.hero_id} fit={fit} />
                ))}
              </div>
            </details>
          ) : null}
        </section>
      ) : null}

      {data.pool.length > 0 ? (
        <HeroPool
          pool={data.pool}
          summary={data.summary}
          recentWindow={data.recent_window}
        />
      ) : null}

      {data.meta_leaders.length > 0 ? (
        <section className="flex flex-col gap-4">
          <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
            Current meta
          </h2>

          <Card className="flex flex-col gap-3">
            <ul className="m-0 flex list-none flex-col gap-3 p-0">
              {data.meta_leaders.map((hero) => (
                <li key={hero.hero_id} className="flex items-center gap-3">
                  <HeroPortrait
                    heroId={hero.hero_id}
                    heroName={hero.hero_name}
                    size="sm"
                  />
                  <span className="min-w-0 flex-1 truncate text-sm text-ink">
                    {hero.hero_name}
                  </span>
                  <span className="shrink-0 font-mono text-xs tabular-nums text-ink-muted">
                    {formatPercent(hero.win_rate)} win ·{" "}
                    {formatPercent(hero.pick_rate)} pick
                  </span>
                  <span className="w-8 shrink-0 text-right font-mono text-sm tabular-nums text-number">
                    {formatScore(hero.meta_strength)}
                  </span>
                </li>
              ))}
            </ul>
          </Card>

          {/* The honesty note the meta section rests on. */}
          <Card className="flex flex-col gap-2">
            <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
              What meta strength means
            </h3>
            <p className="text-xs leading-relaxed text-ink-muted">
              A 0-100 score combining win rate against the rest of the roster,
              how contested the hero is, and its recent direction — deliberately
              not the win rate on its own. Heroes with too few recorded games
              are pulled toward the middle rather than topping the list on a
              small sample.
            </p>
            <p className="text-xs leading-relaxed text-ink-faint">
              {data.meta.source ? `Source: ${data.meta.source}. ` : ""}
              Segmented by: {data.meta.segmented_by.join(", ") || "nothing — these are all brackets combined"}.
              {data.meta.note ? ` ${data.meta.note}` : ""}
            </p>
          </Card>
        </section>
      ) : null}
    </div>
  );
}

function HeroSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading hero intelligence…</span>
      <div className="h-44 animate-pulse rounded-card bg-surface-2" />
      <div className="h-44 animate-pulse rounded-card bg-surface-2" />
      <div className="h-64 animate-pulse rounded-card bg-surface-2" />
    </div>
  );
}
