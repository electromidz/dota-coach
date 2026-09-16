"use client";

import { useEffect, useState } from "react";

import { ComparisonContext } from "@/components/benchmark/ComparisonContext";
import { BulletRow } from "@/components/charts/BulletRow";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { ApiError, getBenchmark } from "@/lib/api";
import type { BenchmarkResponse } from "@/lib/types";

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
export function RoleBenchmark({ roleLabel }: { roleLabel: string }) {
  const [data, setData] = useState<BenchmarkResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

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
  }, []);

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
    <section className="flex flex-col gap-4">
      <header className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          Against the top 20%
        </h2>
        {data.results.length > 0 ? (
          <span className="font-mono text-[0.6875rem] tabular-nums text-ink-faint">
            {data.hero_name} · {data.sample}{" "}
            {data.sample === 1 ? "game" : "games"} as {roleLabel}
          </span>
        ) : null}
      </header>

      {data.note ? <Alert tone="info">{data.note}</Alert> : null}

      {data.results.length > 0 ? (
        <Card className="flex flex-col gap-6">
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
