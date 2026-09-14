"use client";

import Link from "next/link";
import { useState } from "react";

import { EvidenceList } from "@/components/coach/EvidenceList";
import { InsightCard } from "@/components/coach/InsightCard";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ApiError } from "@/lib/api";
import { formatGeneratedAt } from "@/lib/coach";
import type { CoachResponse } from "@/lib/types";

/**
 * A coaching analysis, with the control that generates one.
 *
 * Shared by the coach page and the match page, because the contract is the
 * same in both places: evidence is always shown, the analysis is shown when
 * one exists, and generating is an explicit act the user takes.
 */
export function Analysis({
  data,
  onGenerate,
  generateLabel,
  emptyHint,
}: {
  data: CoachResponse;
  onGenerate: () => Promise<CoachResponse>;
  generateLabel: string;
  emptyHint: string;
}) {
  const [current, setCurrent] = useState(data);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** A lapsed trial is a state, not a failure, and gets its own treatment. */
  const [paywalled, setPaywalled] = useState(false);

  async function generate() {
    setBusy(true);
    setError(null);
    setPaywalled(false);
    try {
      setCurrent(await onGenerate());
    } catch (e) {
      if (e instanceof ApiError && e.isPaymentRequired) {
        setPaywalled(true);
        return;
      }
      setError(
        e instanceof ApiError ? e.message : "Could not reach the coach.",
      );
    } finally {
      setBusy(false);
    }
  }

  const { analysis } = current;

  return (
    <div className="flex flex-col gap-6">
      {current.note ? <Alert tone="info">{current.note}</Alert> : null}
      {error ? <Alert>{error}</Alert> : null}

      {/* Everything measured on this page is still there; only the model call
          is gated, so this says what is missing and where to fix it. */}
      {paywalled ? (
        <Alert tone="info" title="Your free trial has ended">
          The measured evidence below is unchanged. To have the coach interpret
          it again,{" "}
          <Link href="/billing" className="text-function hover:underline">
            subscribe
          </Link>
          .
        </Alert>
      ) : null}

      {analysis ? (
        <section className="flex flex-col gap-4">
          <Card className="flex flex-col gap-2" glow="keyword">
            <p className="leading-relaxed text-ink">{analysis.summary}</p>
            <p className="font-mono text-[0.625rem] text-ink-faint">
              {formatGeneratedAt(analysis.generated_at)} · {analysis.model}
            </p>
          </Card>

          {current.stale ? (
            <Alert tone="info">
              You have synced matches since this was written, so it does not
              include your most recent games.
            </Alert>
          ) : null}

          {analysis.insights.map((insight, index) => (
            <InsightCard
              key={`${insight.kind}-${index}`}
              insight={insight}
              evidence={analysis.evidence}
            />
          ))}
        </section>
      ) : (
        <Card className="flex flex-col gap-3">
          <p className="text-sm leading-relaxed text-ink-muted">{emptyHint}</p>
        </Card>
      )}

      {current.llm_available && current.evidence.length > 0 ? (
        <div className="flex flex-col gap-2">
          <Button onClick={generate} disabled={busy} className="self-start">
            {busy ? "Thinking…" : analysis ? "Refresh the analysis" : generateLabel}
          </Button>
          {analysis && !current.stale ? (
            // The backend answers an identical question from storage, so say
            // so rather than implying a fresh opinion is one click away.
            <p className="text-xs text-ink-faint">
              Nothing has changed since this was written, so refreshing returns
              the same answer. Sync new matches to get a new one.
            </p>
          ) : null}
        </div>
      ) : null}

      <EvidenceList evidence={current.evidence} />
    </div>
  );
}
