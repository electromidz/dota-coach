"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { Card } from "@/components/ui/Card";
import { SectionError } from "@/components/ui/SectionError";
import { ApiError, getTrainingFocus } from "@/lib/api";
import { CONFIDENCE_LABEL } from "@/lib/confidence";
import { impactOf, type Impact } from "@/lib/overview";
import { formatFocusValue } from "@/lib/training";
import type { TrainingFocus, TrainingFocusResponse } from "@/lib/types";
import { cn } from "@/lib/utils";

const IMPACT_CLASS: Record<Impact, string> = {
  high: "bg-warn/15 text-warn",
  medium: "bg-function/15 text-function",
  low: "bg-mark-track text-ink-muted",
};

/**
 * The top three things to work on: the current training focus first, then
 * the two the focus engine ranks next. Picked and scored on the server.
 */
export function CoachRecommendations() {
  const [data, setData] = useState<TrainingFocusResponse | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "none" | "error">("loading");
  const [error, setError] = useState<{ message: string; requestId: string | null } | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;
    getTrainingFocus()
      .then((r) => {
        if (cancelled) return;
        setData(r);
        setState("ready");
      })
      .catch((e) => {
        if (cancelled) return;
        // No role chosen yet, or the plan does not include coaching: the Coach
        // page explains both, so this section just stays out of the way.
        if (e instanceof ApiError && (e.status === 409 || e.isPaymentRequired || e.isUnauthenticated)) {
          setState("none");
          return;
        }
        setError({
          message: e instanceof ApiError ? e.message : "Could not load your coaching focus.",
          requestId: e instanceof ApiError ? e.requestId : null,
        });
        setState("error");
      });
    return () => {
      cancelled = true;
    };
  }, [attempt]);

  if (state === "loading") {
    return (
      <div className="grid gap-3 lg:grid-cols-3" aria-busy="true">
        {[0, 1, 2].map((i) => (
          <div key={i} className="glass h-32 animate-pulse rounded-card" />
        ))}
      </div>
    );
  }
  if (state === "error" && error) {
    return (
      <SectionError
        section="Coach recommendations"
        message={error.message}
        requestId={error.requestId}
        onRetry={() => {
          setState("loading");
          setAttempt((n) => n + 1);
        }}
      />
    );
  }

  const focuses = data?.focus ? [data.focus, ...data.next_up].slice(0, 3) : [];
  if (state === "none" || focuses.length === 0) return null;

  return (
    <section aria-label="Coach recommendations" className="grid gap-3 lg:grid-cols-3">
      {focuses.map((focus, i) => (
        <RecommendationCard key={focus.key} focus={focus} primary={i === 0} />
      ))}
    </section>
  );
}

function RecommendationCard({ focus, primary }: { focus: TrainingFocus; primary: boolean }) {
  const impact = impactOf(focus);
  return (
    <Card glow={primary ? "keyword" : undefined} className="flex min-w-0 flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <span className="text-[0.6875rem] font-semibold uppercase tracking-[0.16em] text-ink-faint">
          {primary ? "Current focus" : "Next up"}
        </span>
        {impact ? (
          <span className={cn("rounded-full px-2 py-0.5 text-[0.625rem] font-semibold uppercase tracking-wider", IMPACT_CLASS[impact])}>
            {impact} impact
          </span>
        ) : null}
      </div>

      <p className="line-clamp-2 font-semibold leading-snug text-ink">{focus.title}</p>

      <ul className="flex flex-wrap gap-1.5 text-[0.6875rem]">
        <li className="rounded-full bg-surface-2 px-2 py-0.5 text-ink-muted">{focus.measure_label}</li>
        <li className="rounded-full bg-surface-2 px-2 py-0.5 font-mono tabular-nums text-ink-muted">
          {formatFocusValue(focus.measure, focus.current_value)} → {formatFocusValue(focus.measure, focus.target_value)}
        </li>
        <li className="rounded-full bg-surface-2 px-2 py-0.5 text-ink-faint">{CONFIDENCE_LABEL[focus.confidence]}</li>
      </ul>

      <Link
        href="/coach"
        className="focus-neon mt-auto self-start rounded text-xs text-function hover:text-ink"
      >
        View drills →
      </Link>
    </Card>
  );
}
