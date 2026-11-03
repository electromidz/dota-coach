"use client";

import { useEffect, useState } from "react";

import { ComparisonContext } from "@/components/benchmark/ComparisonContext";
import { PeerBullet, PeerLegend } from "@/components/coach/PeerBullet";
import { Card } from "@/components/ui/Card";
import { InfoTip } from "@/components/ui/InfoTip";
import { SectionError } from "@/components/ui/SectionError";
import { ApiError, getBenchmark } from "@/lib/api";
import { hasPeerFigure } from "@/lib/coach";
import type { BenchmarkResponse, Evidence } from "@/lib/types";

/**
 * Where the player stands against the top 20%, in the role being coached.
 *
 * The comparison is deliberately narrow on our side — the eligible matches in
 * the chosen role, on that role's most-played hero — and unavoidably wide on
 * the peer side, which the context block states rather than glosses. Nothing
 * here is generated: the values, the median, the top-20% line and the gap all
 * come from the benchmark engine, and when the provider has nothing to say the
 * panel says that instead of estimating.
 */
export function RoleBenchmark({
  roleLabel,
  snapshot,
}: {
  roleLabel: string;
  /** The analysis's evidence. Its benchmark figures win over a live read, so
   *  this panel shows the same numbers as the cards. */
  snapshot?: Evidence[];
}) {
  const [data, setData] = useState<BenchmarkResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** Bumped by Retry to re-run the fetch below. */
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;

    // No role argument: the endpoint follows the coaching profile, so this
    // panel cannot drift from the role the rest of the page is about.
    getBenchmark()
      .then((response) => {
        if (!cancelled) setData(response);
      })
      .catch((e) => {
        if (cancelled) return;
        if (e instanceof ApiError && e.isUnauthenticated) return;
        setError(
          e instanceof ApiError ? e.message : "Could not load your benchmark.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, [attempt]);

  if (error) {
    return (
      <SectionError
        section="Role benchmark"
        message={error}
        onRetry={() => {
          setError(null);
          setAttempt((n) => n + 1);
        }}
      />
    );
  }
  if (!data) {
    return (
      <div
        className="h-56 animate-pulse rounded-card bg-surface-2"
        aria-busy="true"
      />
    );
  }

  const fromSnapshot = (snapshot ?? []).filter(hasPeerFigure);
  const items: Evidence[] =
    fromSnapshot.length > 0
      ? fromSnapshot
      : data.results.map((r) => ({
          id: `benchmark.${r.metric}`,
          kind: "benchmark" as const,
          label: r.label,
          statement: r.note ?? "",
          sample: r.player_sample,
          confidence: r.confidence,
          figure: {
            metric: r.metric,
            hero_id: data.hero_id,
            value: r.player_value,
            higher_is_better: r.higher_is_better,
            unit: null,
            median: r.peer_median,
            top_20: r.top_20_value,
            percentile: r.percentile,
            previous: null,
            percent_change: null,
            status: null,
          },
        }));
  const sample = items[0]?.sample ?? data.sample;

  return (
    <section className="flex flex-col gap-3" aria-label="Against the top 20%">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="flex items-center gap-2 font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          Against the top 20%
          <InfoTip label="What this compares against">
            <ComparisonContext context={data.context} />
          </InfoTip>
        </h2>
        {items.length > 0 ? (
          <span className="font-mono text-[0.6875rem] tabular-nums text-ink-faint">
            {data.hero_name} · {sample} {sample === 1 ? "game" : "games"} as {roleLabel}
          </span>
        ) : null}
      </header>

      {data.note && items.length === 0 ? (
        <p className="truncate text-xs text-ink-faint">{data.note}</p>
      ) : null}

      {items.length > 0 ? (
        <Card className="flex flex-col gap-4">
          <PeerLegend />
          <ul className="grid grid-cols-1 gap-x-8 gap-y-3 md:grid-cols-2">
            {items.map((item) => (
              <PeerBullet key={item.id} item={item} />
            ))}
          </ul>
        </Card>
      ) : null}
    </section>
  );
}
