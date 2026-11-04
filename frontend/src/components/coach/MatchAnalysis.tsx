"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { FindingCard, MeasuredTable } from "@/components/coach/FindingCard";
import { TrainingPlan } from "@/components/coach/TrainingPlan";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { InfoTip } from "@/components/ui/InfoTip";
import { analyzeMatch, ApiError, getMatchAnalysis, requestReplayParse } from "@/lib/api";
import { formatGeneratedAt } from "@/lib/coach";
import { useIsAdmin } from "@/lib/session-context";
import type { CoachResponse, Evidence } from "@/lib/types";
import { useGeneration } from "@/lib/useGeneration";
import { cn, fixOrdinals } from "@/lib/utils";

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
  const isAdmin = useIsAdmin();
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
      {missing ? <MissingTimeline id={id} evidence={missing} /> : null}

      {busy ? (
        <Card className="flex flex-col gap-2" glow="function">
          <p className="text-sm text-ink" aria-busy="true">
            Analysing your match… usually under a minute.
          </p>
        </Card>
      ) : null}

      {analysis ? (
        <>
          <section className="flex flex-col gap-4">
            <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
              Coach findings
            </h2>

            <Card className="flex items-start gap-2" glow="keyword">
              <p className="line-clamp-2 min-w-0 flex-1 text-sm leading-relaxed text-ink">
                {analysis.summary
                  ? fixOrdinals(analysis.summary)
                  : "The headline could not be verified against this match's data, so it was discarded."}
              </p>
              <InfoTip label="About this analysis">
                {analysis.summary ? <>{fixOrdinals(analysis.summary)} </> : null}
                Generated {formatGeneratedAt(analysis.generated_at)}
                {isAdmin ? <> · {analysis.model}</> : null}.
              </InfoTip>
            </Card>

            {current.stale ? (
              <div className="glass flex flex-wrap items-center justify-between gap-3 rounded-card px-4 py-3">
                <span className="text-sm text-ink">Your baseline changed since this analysis</span>
                {current.llm_available ? (
                  <Button onClick={generate} disabled={busy}>
                    {busy ? "Reading…" : "Re-read"}
                  </Button>
                ) : null}
              </div>
            ) : null}

            {/* At most three, already ordered by severity on the server. */}
            <div className="grid gap-4 lg:grid-cols-3 lg:items-start">
              {analysis.insights.slice(0, 3).map((insight, index) => (
                <FindingCard
                  key={`${insight.kind}-${index}`}
                  insight={insight}
                  evidence={analysis.evidence}
                />
              ))}
            </div>
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
              <p className="text-sm text-ink-muted">
                No analysis yet. It reads only the measured data below, against
                your own average here.
              </p>
            </Card>
          </section>
        )
      )}

      {/* Only before the first reading: afterwards, a re-read is offered by
          the banner when, and only when, the baseline has moved. */}
      {!analysis && current.llm_available && current.evidence.length > 0 ? (
        <Button onClick={generate} disabled={busy} className="self-start">
          {busy ? "Analysing…" : "Analyse this match"}
        </Button>
      ) : null}

      {current.evidence.length > 0 ? (
        <details className="glass group rounded-card px-4 py-3">
          <summary className="focus-neon cursor-pointer list-none rounded text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint hover:text-ink">
            What the coach measured
            <span className="ml-2 font-mono normal-case tracking-normal">
              ({current.evidence.length})
            </span>
          </summary>
          <div className="mt-3">
            <MeasuredTable evidence={current.evidence} />
          </div>
        </details>
      ) : null}
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
function MissingTimeline({ id, evidence }: { id: string; evidence: Evidence }) {
  const [state, setState] = useState<"idle" | "sending" | "sent" | "failed">("idle");
  const [message, setMessage] = useState<string | null>(null);

  async function request() {
    setState("sending");
    try {
      const response = await requestReplayParse(id);
      setMessage(response.note);
      setState("sent");
    } catch (e) {
      setMessage(e instanceof ApiError ? e.message : "Could not request a parse.");
      setState("failed");
    }
  }

  return (
    <div className="glass flex flex-wrap items-center gap-3 rounded-card px-4 py-3">
      <span className="flex items-center gap-1.5 text-sm text-ink">
        No timeline data
        <InfoTip label="Why there is no timeline">{fixOrdinals(evidence.statement)}</InfoTip>
      </span>
      <span className="ml-auto flex flex-wrap items-center gap-2">
        <Button
          variant="ghost"
          onClick={request}
          disabled={state === "sending" || state === "sent"}
          className="px-3 text-xs"
        >
          {state === "sending" ? "Requesting…" : state === "sent" ? "Parse requested" : "Request replay parse"}
        </Button>
        <a
          href="https://www.opendota.com/faq"
          target="_blank"
          rel="noreferrer"
          className="focus-neon rounded text-xs text-function hover:text-ink"
        >
          How to make your profile public
        </a>
      </span>
      {message ? (
        <p className={cn("w-full text-xs", state === "failed" ? "text-error" : "text-ink-faint")}>
          {message}
        </p>
      ) : null}
    </div>
  );
}
