"use client";

import { useEffect, useState } from "react";

import { Analysis } from "@/components/coach/Analysis";
import { Alert } from "@/components/ui/Alert";
import { analyzeMatch, ApiError, getMatchAnalysis } from "@/lib/api";
import type { CoachResponse } from "@/lib/types";

/**
 * The coaching section of a match page.
 *
 * Loads the stored analysis and the measured evidence, which costs nothing.
 * Generating is a separate, explicit click — opening a match must never spend
 * a model call.
 */
export function MatchAnalysis({ id }: { id: string }) {
  const [data, setData] = useState<CoachResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    getMatchAnalysis(id)
      .then((response) => {
        if (!cancelled) setData(response);
      })
      .catch((e) => {
        if (cancelled) return;
        // A signed-out or missing match is already handled by the page above
        // this one; anything else is reported here rather than swallowed.
        if (e instanceof ApiError && e.isUnauthenticated) return;
        setError(
          e instanceof ApiError ? e.message : "Could not load the analysis.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, [id]);

  if (error) return <Alert>{error}</Alert>;
  if (!data) {
    return (
      <div
        className="h-32 animate-pulse rounded-card bg-surface-2"
        aria-busy="true"
      />
    );
  }

  return (
    <section className="flex flex-col gap-4">
      <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
        Coaching
      </h2>

      <Analysis
        data={data}
        onGenerate={() => analyzeMatch(id)}
        generateLabel="Analyse this match"
        emptyHint="No analysis of this match yet. It will be read against your own averages, not against a generic standard."
      />
    </section>
  );
}
