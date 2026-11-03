"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { CoachInsightCard } from "@/components/coach/CoachInsightCard";
import { CoachSummary } from "@/components/coach/CoachSummary";
import { GoalPlan } from "@/components/coach/GoalPlan";
import { MeasuredData } from "@/components/coach/MeasuredData";
import { ProgressSummary } from "@/components/coach/ProgressSummary";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { InfoTip } from "@/components/ui/InfoTip";
import { SectionBoundary } from "@/components/ui/SectionError";
import { formatGeneratedAt } from "@/lib/coach";
import type { CoachResponse, Evidence, RolePerformance } from "@/lib/types";
import { useGeneration } from "@/lib/useGeneration";
import { fixOrdinals } from "@/lib/utils";

/**
 * Everything on the coach page that is read from the analysis, in one
 * snapshot: the summary row, the strength and weakness cards, what changed,
 * the plan, and the data behind it. `children` sits after the plan.
 */
export function Analysis({
  data,
  onGenerate,
  onReload,
  generateLabel,
  emptyHint,
  performance,
  focusTitle,
  isAdmin = false,
  children,
}: {
  data: CoachResponse;
  onGenerate: () => Promise<CoachResponse>;
  /** Re-reads without generating; used to pick up a background refresh. */
  onReload?: () => Promise<CoachResponse>;
  generateLabel: string;
  emptyHint: string;
  performance?: RolePerformance;
  focusTitle?: string | null;
  /** The model name is shown to admins only. */
  isAdmin?: boolean;
  /** Rendered after the plan, given the same snapshot as everything else. */
  children?: (snapshot: Evidence[] | undefined) => React.ReactNode;
}) {
  const { current, busy, error, paywalled, generate, replace } = useGeneration(
    data,
    onGenerate,
  );

  const { analysis } = current;
  const refreshing = useRefreshPoll(current, onReload, replace);
  const newMatches = current.new_matches_since_analysis ?? 0;
  /** New matches, or the measured data changed under the analysis (`stale`). */
  const behind = newMatches > 0 || current.stale;
  /** One snapshot for the whole section: the evidence the analysis was
   *  written from. Live evidence only when there is no analysis yet. */
  const shown = measuredEvidence(current);
  const cards = (analysis?.insights ?? []).filter(
    (i) => i.kind === "strength" || i.kind === "weakness" || i.kind === "warning",
  );
  const otherInsights = (analysis?.insights ?? []).filter((i) => !cards.includes(i));

  return (
    <div className="flex flex-col gap-8">
      <SectionBoundary section="Summary">
        <CoachSummary
          performance={performance}
          insights={analysis?.insights ?? []}
          focusTitle={focusTitle}
        />
      </SectionBoundary>

      {current.note ? <Alert tone="info">{current.note}</Alert> : null}
      {error ? <Alert>{error}</Alert> : null}
      {paywalled ? (
        <Alert tone="info" title="Your free trial has ended">
          <Link href="/billing" className="text-function hover:underline">
            Subscribe
          </Link>{" "}
          to have the coach read your games again.
        </Alert>
      ) : null}

      {refreshing ? (
        <div role="status" className="glass flex items-center gap-3 rounded-card p-4 text-sm text-ink">
          <span aria-hidden className="size-2 animate-pulse rounded-full bg-warn" />
          Updating with {newMatches} new {newMatches === 1 ? "match" : "matches"}…
        </div>
      ) : behind && analysis && current.llm_available ? (
        <div className="glass flex flex-wrap items-center justify-between gap-3 rounded-card p-4">
          <span className="text-sm text-ink">
            {newMatches > 0 ? (
              <>
                <span className="font-mono tabular-nums text-number">{newMatches}</span> new{" "}
                {newMatches === 1 ? "match" : "matches"} since this analysis
              </>
            ) : current.new_matches_since_analysis == null ? (
              "New matches since this analysis"
            ) : (
              "Data updated since this analysis"
            )}
          </span>
          <Button onClick={generate} disabled={busy}>
            {busy ? "Thinking…" : "Refresh"}
          </Button>
        </div>
      ) : null}

      {analysis ? (
        <section className="flex flex-col gap-3" aria-label="Coach analysis">
          <div className="flex items-start gap-2">
            <p className="line-clamp-2 min-w-0 flex-1 text-sm leading-relaxed text-ink">
              {analysis.summary ? fixOrdinals(analysis.summary) : "Strengths and weaknesses"}
            </p>
            <InfoTip label="About this analysis" align="end">
              {analysis.summary ? <span>{fixOrdinals(analysis.summary)}</span> : null}
              <span className="font-mono text-[0.6875rem] text-ink-faint">
                {formatGeneratedAt(analysis.generated_at)}
                {isAdmin ? ` · ${analysis.model}` : ""}
              </span>
            </InfoTip>
          </div>

          {cards.length > 0 || otherInsights.length > 0 ? (
            <div className="grid gap-3 md:grid-cols-2">
              {[...cards, ...otherInsights].map((insight, index) => (
                <CoachInsightCard
                  key={`${insight.kind}-${index}`}
                  insight={insight}
                  evidence={analysis.evidence}
                />
              ))}
            </div>
          ) : null}
        </section>
      ) : (
        <div className="glass flex flex-wrap items-center justify-between gap-3 rounded-card p-4">
          <span className="text-sm text-ink-muted">{emptyHint}</span>
          {current.llm_available && current.evidence.length > 0 ? (
            <Button onClick={generate} disabled={busy}>
              {busy ? "Thinking…" : generateLabel}
            </Button>
          ) : null}
        </div>
      )}

      <SectionBoundary section="Progress since last session">
        <ProgressSummary snapshot={analysis ? shown : undefined} />
      </SectionBoundary>

      {analysis ? (
        <SectionBoundary section="Training plan">
          <GoalPlan
            plan={analysis.plan}
            goals={current.goals ?? []}
            previous={current.previous_goals ?? []}
          />
        </SectionBoundary>
      ) : null}

      {children?.(analysis ? shown : undefined)}

      <MeasuredData evidence={shown} />
    </div>
  );
}

export function measuredEvidence(response: CoachResponse): Evidence[] {
  return response.analysis?.evidence ?? response.evidence;
}

/** How often, and how many times, to look for a background refresh. */
const POLL_MS = 10_000;
const POLL_TRIES = 12;

/**
 * While the backend regenerates in the background, re-read until a newer
 * analysis appears or the tries run out. The current analysis stays on screen
 * the whole time.
 */
function useRefreshPoll(
  current: CoachResponse,
  reload: (() => Promise<CoachResponse>) | undefined,
  replace: (next: CoachResponse) => void,
): boolean {
  const [waiting, setWaiting] = useState(Boolean(current.refreshing && reload));
  const since = current.analysis?.generated_at;

  useEffect(() => {
    if (!waiting || !reload) return;
    let tries = 0;
    const timer = setInterval(async () => {
      tries += 1;
      try {
        const next = await reload();
        if (next.analysis?.generated_at !== since) {
          replace(next);
          setWaiting(false);
        }
      } catch {
        // A failed poll is not worth an error card; the analysis on screen is valid.
      }
      if (tries >= POLL_TRIES) setWaiting(false);
    }, POLL_MS);
    return () => clearInterval(timer);
  }, [waiting, reload, replace, since]);

  return waiting;
}
