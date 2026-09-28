"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { EvidenceList } from "@/components/coach/EvidenceList";
import { InsightCard } from "@/components/coach/InsightCard";
import { TrainingPlan } from "@/components/coach/TrainingPlan";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { analyzeMatch, ApiError, getMatchAnalysis } from "@/lib/api";
import { formatGeneratedAt } from "@/lib/coach";
import type { CoachResponse, Evidence } from "@/lib/types";
import { useGeneration } from "@/lib/useGeneration";

/**
 * The evidence id the backend uses to say a match has no measured timeline.
 *
 * Surfaced at the top of this section rather than left in the evidence list at
 * the bottom. A reader who does not know the deep reading was unavailable will
 * read a thin analysis as a complete one, which is the single most misleading
 * thing this page could do.
 */
const NO_TIMELINE = "match.timeline.unavailable";

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

  if (error) return <Alert title="Analysis unavailable">{error}</Alert>;
  if (!data) {
    return (
      <div className="flex flex-col gap-3" aria-busy="true">
        <p className="text-sm text-ink-muted">Reading this match…</p>
        <div className="h-32 animate-pulse rounded-card bg-surface-2" />
      </div>
    );
  }

  return <Loaded id={id} data={data} />;
}

/**
 * The section once the evidence is in hand.
 *
 * Split from the loader so the generation state is seeded from a response that
 * definitely exists, rather than from a nullable one — the alternative is a hook
 * that has to model "no data yet" a second time.
 */
function Loaded({ id, data }: { id: string; data: CoachResponse }) {
  const { current, busy, error, paywalled, generate } = useGeneration(
    data,
    () => analyzeMatch(id),
  );

  const { analysis } = current;
  const missing = current.evidence.find((item) => item.id === NO_TIMELINE);

  return (
    <div className="flex flex-col gap-5">
      {current.note ? <Alert tone="info">{current.note}</Alert> : null}

      {/* The generation failed and nothing survived verification. Said plainly:
          a partial answer must never be dressed up as a complete one. */}
      {error ? <Alert title="The coach could not answer">{error}</Alert> : null}

      {paywalled ? (
        <Alert tone="info" title="Your free trial has ended">
          Everything measured below is unchanged. To have the coach read this
          match again,{" "}
          <Link href="/billing" className="text-function hover:underline">
            subscribe
          </Link>
          .
        </Alert>
      ) : null}

      {/* What could not be concluded, and why, before anything that was. */}
      {missing ? <MissingTimeline evidence={missing} /> : null}

      {busy ? (
        <Card className="flex flex-col gap-2" glow="function">
          <p className="text-sm text-ink" aria-busy="true">
            Analysing your match…
          </p>
          <p className="text-xs leading-relaxed text-ink-faint">
            The coach is reading the timeline below. This usually takes under a
            minute.
          </p>
        </Card>
      ) : null}

      {analysis ? (
        <>
          <section className="flex flex-col gap-4">
            <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
              What went wrong?
            </h2>

            {analysis.summary ? (
              <Card className="flex flex-col gap-2" glow="keyword">
                <p className="leading-relaxed text-ink">{analysis.summary}</p>
                <p className="font-mono text-[0.625rem] text-ink-faint">
                  {formatGeneratedAt(analysis.generated_at)} · {analysis.model}
                </p>
              </Card>
            ) : (
              <Card className="flex flex-col gap-2">
                <p className="text-sm leading-relaxed text-ink-muted">
                  The coach&apos;s headline could not be verified against this
                  match&apos;s data, so it was discarded. Everything below was.
                </p>
                <p className="font-mono text-[0.625rem] text-ink-faint">
                  {formatGeneratedAt(analysis.generated_at)} · {analysis.model}
                </p>
              </Card>
            )}

            {current.stale ? (
              <Alert tone="info">
                Your record has changed since this was written, so it was read
                against slightly different averages than the ones shown now.
              </Alert>
            ) : null}

            {/* Numbered, because the backend ranks them most-costly-first and
                that ranking is part of the answer. */}
            {analysis.insights.map((insight, index) => (
              <InsightCard
                key={`${insight.kind}-${index}`}
                insight={insight}
                evidence={analysis.evidence}
                rank={index + 1}
              />
            ))}
          </section>

          {/* The one thing to work on, last and on its own — a reader who stops
              here has still got the point of the page. */}
          {analysis.plan.length > 0 ? (
            <>
              <hr className="border-glass-edge" />
              <TrainingPlan
                plan={analysis.plan}
                evidence={analysis.evidence}
                heading="Your main training focus"
              />
            </>
          ) : null}
        </>
      ) : (
        !busy && (
          <section className="flex flex-col gap-4">
            <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
              Coaching
            </h2>
            <Card>
              <p className="text-sm leading-relaxed text-ink-muted">
                No analysis of this match yet. It will be read against your own
                averages in this role, not against a generic standard, and only
                from the evidence below.
              </p>
            </Card>
          </section>
        )
      )}

      {current.llm_available && current.evidence.length > 0 ? (
        <div className="flex flex-col gap-2">
          <Button onClick={generate} disabled={busy} className="self-start">
            {busy
              ? "Analysing…"
              : analysis
                ? "Read this match again"
                : "Analyse this match"}
          </Button>
          {analysis && !current.stale ? (
            // The backend answers an identical question from storage, so say
            // so rather than implying a fresh opinion is one click away.
            <p className="text-xs text-ink-faint">
              Nothing behind this analysis has changed, so asking again returns
              the same answer.
            </p>
          ) : null}
        </div>
      ) : null}

      <EvidenceList evidence={current.evidence} />
    </div>
  );
}

/**
 * What this analysis cannot conclude, and why.
 *
 * The backend composes the sentence, including which of the four reasons applies
 * — no credentials, provider down, provider does not have the match, or Valve
 * never parsed the replay. Rendering its own wording keeps one explanation in
 * one place, and keeps this component from having to distinguish cases it cannot
 * see.
 */
function MissingTimeline({ evidence }: { evidence: Evidence }) {
  return (
    <Alert tone="info" title="No second-by-second data for this match">
      {evidence.statement}
    </Alert>
  );
}
