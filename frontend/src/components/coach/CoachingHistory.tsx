"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { SeriesChart } from "@/components/charts/SeriesChart";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { SteamLoginButton } from "@/components/ui/SteamLoginButton";
import {
  ApiError,
  getCoachingProgress,
  getCoachingSessions,
} from "@/lib/api";
import { useSession } from "@/lib/session-context";
import type { ProgressResponse, SessionHistoryResponse } from "@/lib/types";
import { timeAgo } from "@/lib/utils";

/**
 * The record of what was measured, and when.
 *
 * Two questions on one screen, deliberately: "how have my numbers moved" and
 * "what did each session actually say". The trends answer the first; the list
 * answers the second, and each row opens the snapshot verbatim.
 *
 * Nothing here is recomputed. A session shows the figures it was written with,
 * not today's — rewriting session one with the numbers the player has now
 * would erase the improvement the page exists to show.
 */
export function CoachingHistory() {
  const { session } = useSession();
  const [history, setHistory] = useState<SessionHistoryResponse | null>(null);
  const [progress, setProgress] = useState<ProgressResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [needsRole, setNeedsRole] = useState(false);

  useEffect(() => {
    if (session.kind !== "signed-in") return;
    let cancelled = false;

    Promise.all([getCoachingSessions(), getCoachingProgress()])
      .then(([sessions, prog]) => {
        if (cancelled) return;
        setHistory(sessions);
        setProgress(prog);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        if (e instanceof ApiError && e.isUnauthenticated) return;
        // 409 is "you have not chosen a role", which is a prompt rather than
        // a failure — the player needs a link, not an error.
        if (e instanceof ApiError && e.status === 409) {
          setNeedsRole(true);
          return;
        }
        setError(e instanceof ApiError ? e.message : "Could not load your history.");
      });

    return () => {
      cancelled = true;
    };
  }, [session.kind]);

  if (session.kind === "loading") return <HistorySkeleton />;
  if (session.kind === "anonymous") {
    return (
      <div className="flex flex-col gap-4">
        <Alert tone="info">Sign in with Steam to see your coaching history.</Alert>
        <SteamLoginButton className="w-full" />
      </div>
    );
  }

  if (needsRole) {
    return (
      <Alert tone="info" title="No role chosen yet">
        Coaching is scoped to one role, and so is its history.{" "}
        <Link href="/coach" className="focus-neon rounded underline">
          Choose a role
        </Link>{" "}
        and a session is recorded once you have played enough new games in it.
      </Alert>
    );
  }

  if (error) return <Alert>{error}</Alert>;
  if (!history || !progress) return <HistorySkeleton />;

  if (history.total === 0) {
    return (
      <Alert tone="info" title="No sessions yet">
        A coaching session is recorded once you have ten new eligible{" "}
        {history.role_label} matches since the last one. Sync after a few games
        and the first will appear here.
      </Alert>
    );
  }

  return (
    <div className="flex flex-col gap-8 pb-4">
      <BackLink />

      {progress.series.length > 0 ? (
        <section className="flex flex-col gap-3">
          <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
            {history.role_label} over time
          </h2>
          <div className="grid gap-4 lg:grid-cols-2">
            {progress.series.slice(0, 6).map((series) => (
              <Card key={series.key} className="flex flex-col gap-2">
                <p className="text-xs uppercase tracking-wider text-ink-faint">
                  {series.label}
                </p>
                <SeriesChart series={series} />
              </Card>
            ))}
          </div>
        </section>
      ) : null}

      <section className="flex flex-col gap-3">
        <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          Sessions
        </h2>

        <ol className="flex flex-col gap-3">
          {history.sessions.map((entry) => (
            <li key={entry.id}>
              <Link
                href={`/coach/sessions/${entry.id}`}
                className="focus-neon block rounded-card transition-transform duration-200 ease-out hover:-translate-y-0.5"
              >
                <Card className="flex items-center justify-between gap-4">
                  <div className="flex min-w-0 flex-col gap-1">
                    <span className="font-display text-sm tracking-wide text-ink">
                      Session #{entry.sequence}
                    </span>
                    <span className="text-xs text-ink-faint">
                      {entry.analyzed_match_count}{" "}
                      {entry.analyzed_match_count === 1 ? "match" : "matches"} ·{" "}
                      {timeAgo(entry.created_at)}
                      {entry.has_analysis ? " · analysed" : ""}
                    </span>
                  </div>

                  <div className="flex shrink-0 items-center gap-3">
                    {entry.performance !== null ? (
                      <span className="font-mono text-2xl tabular-nums text-number">
                        {Math.round(entry.performance)}
                      </span>
                    ) : (
                      <span className="text-sm text-ink-faint" title="Not enough matches to score">
                        —
                      </span>
                    )}
                    <Icon name="chevron-right" className="size-4 text-ink-faint" />
                  </div>
                </Card>
              </Link>
            </li>
          ))}
        </ol>

        {history.total_pages > 1 ? (
          <p className="text-xs text-ink-faint">
            Showing {history.sessions.length} of {history.total} sessions.
          </p>
        ) : null}
      </section>
    </div>
  );
}

function BackLink() {
  return (
    <Link
      href="/coach"
      className="focus-neon inline-flex min-h-11 w-fit cursor-pointer items-center gap-1.5 rounded text-sm text-ink-muted transition-colors duration-200 ease-out hover:text-function"
    >
      <Icon name="chevron-left" className="size-5" />
      Coach
    </Link>
  );
}

function HistorySkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading your coaching history…</span>
      <div className="h-40 animate-pulse rounded-card bg-surface-2" />
      <div className="h-24 animate-pulse rounded-card bg-surface-2" />
      <div className="h-24 animate-pulse rounded-card bg-surface-2" />
    </div>
  );
}
