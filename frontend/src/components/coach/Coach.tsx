"use client";

import { useEffect, useState } from "react";

import { Analysis } from "@/components/coach/Analysis";
import { PlayerModelPanel } from "@/components/coach/PlayerModelPanel";
import { TrainingFocusCard } from "@/components/coach/TrainingFocusCard";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { analyzeCoach, ApiError, getCoach } from "@/lib/api";
import { useSession } from "@/lib/session-context";
import type { CoachResponse } from "@/lib/types";

/** The career coach: everything measured, plus whatever was last interpreted. */
export function Coach() {
  const { session } = useSession();
  const [data, setData] = useState<CoachResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (session.kind !== "signed-in") return;

    getCoach()
      .then((response) => {
        setData(response);
        setError(null);
      })
      .catch((e) =>
        setError(
          e instanceof ApiError ? e.message : "Could not load your coach.",
        ),
      );
  }, [session.kind]);

  if (session.kind === "loading") return <CoachSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;
  if (session.kind === "error") {
    return <Alert title="Cannot reach the service">{session.message}</Alert>;
  }

  if (error) return <Alert>{error}</Alert>;
  if (!data) return <CoachSkeleton />;

  if (data.evidence.length === 0) {
    return (
      <Alert tone="info">
        Sync some matches first — there is nothing to coach on yet.
      </Alert>
    );
  }

  return (
    <div className="flex flex-col gap-10 pb-4">
      {/* The focus leads: the page answers "what should I work on" before it
          answers anything else. */}
      <TrainingFocusCard />

      <Analysis
        data={data}
        onGenerate={analyzeCoach}
        generateLabel="Analyse my performance"
        emptyHint="No analysis yet. The measured evidence below is ready; ask the coach to interpret it."
      />

      {/* Deterministic, and loaded separately: the model is worth reading
          whether or not a coaching model has ever run. */}
      <PlayerModelPanel />
    </div>
  );
}

function CoachSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading your coach…</span>
      <div className="h-28 animate-pulse rounded-card bg-surface-2" />
      <div className="h-40 animate-pulse rounded-card bg-surface-2" />
      <div className="h-64 animate-pulse rounded-card bg-surface-2" />
    </div>
  );
}
