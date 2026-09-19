"use client";

import { useEffect, useState } from "react";

import { ComparisonContext } from "@/components/benchmark/ComparisonContext";
import { BulletRow } from "@/components/charts/BulletRow";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { ApiError, getBenchmark } from "@/lib/api";
import type { BenchmarkResponse } from "@/lib/types";

/**
 * Where this match's hero sits against peers — gold, XP, last hits, kills,
 * assists, deaths and damage all in one view, each already computed
 * server-side (`getBenchmark`) rather than scored here.
 *
 * Scoped to this match's hero but not its estimated role: `Match.role` is a
 * blunt lane-priority guess (`derive_role`) over a different, looser set of
 * labels than the five roles the benchmark engine coaches on, so forcing one
 * onto the other would silently mislabel the comparison. Omitting the role
 * argument falls back to the player's actual coaching role, the same
 * fallback `RoleBenchmark` already relies on.
 */
export function MatchBenchmark({
  heroId,
  heroName,
}: {
  heroId: number;
  heroName: string;
}) {
  const [data, setData] = useState<BenchmarkResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    getBenchmark(heroId)
      .then((response) => {
        if (!cancelled) setData(response);
      })
      .catch((e) => {
        if (cancelled) return;
        if (e instanceof ApiError && e.isUnauthenticated) return;
        setError(
          e instanceof ApiError ? e.message : "Could not load this benchmark.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, [heroId]);

  if (error) return <Alert>{error}</Alert>;
  if (!data) {
    return (
      <div
        className="h-56 animate-pulse rounded-card bg-surface-2"
        aria-busy="true"
      />
    );
  }

  return (
    <section className="flex flex-col gap-3">
      <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
        Benchmark
      </h2>

      {data.note ? <Alert tone="info">{data.note}</Alert> : null}

      {data.results.length > 0 ? (
        <Card className="flex flex-col gap-6">
          <p className="text-sm text-ink-faint">
            {heroName} ·{" "}
            <span className="font-mono tabular-nums text-ink">{data.sample}</span>{" "}
            {data.sample === 1 ? "match" : "matches"} of yours
          </p>

          {data.results.map((result) => (
            <BulletRow key={result.metric} result={result} />
          ))}

          <ComparisonContext
            context={data.context}
            className="border-t border-border pt-4"
          />
        </Card>
      ) : null}
    </section>
  );
}
