"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { DeltaRow } from "@/components/charts/DeltaRow";
import { formatValue, SeriesChart } from "@/components/charts/SeriesChart";
import { Card } from "@/components/ui/Card";
import { SectionError } from "@/components/ui/SectionError";
import { ApiError, getCoachingProgress } from "@/lib/api";
import type {
  Evidence,
  MetricUnit,
  ProgressResponse,
  ProgressStatus,
} from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * What changed since the last coaching session.
 *
 * Every verdict here — improved, declined, stable, a new issue, a resolved one
 * — is decided by the backend and rendered verbatim. The client does not
 * subtract two numbers and decide what the difference means; that is a claim
 * about a player, and it is made in one place so it cannot be made two
 * different ways.
 *
 * A first session shows the trend it has and says plainly that there is
 * nothing to compare with. "No change" and "nothing to compare" are different
 * answers and are never collapsed into each other.
 */
export function ProgressSummary({ snapshot }: { snapshot?: Evidence[] }) {
  const [data, setData] = useState<ProgressResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** Bumped by Retry to re-run the fetch below. */
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;

    getCoachingProgress()
      .then((response) => {
        if (!cancelled) setData(response);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        if (e instanceof ApiError && (e.isUnauthenticated || e.status === 409)) return;
        setError(
          e instanceof ApiError ? e.message : "Could not load your progress.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, [attempt]);

  if (error) {
    return (
      <SectionError
        section="Progress since last session"
        message={error}
        onRetry={() => {
          setError(null);
          setAttempt((n) => n + 1);
        }}
      />
    );
  }
  if (!data) {
    return (
      <div className="h-40 animate-pulse rounded-card bg-surface-2" aria-busy="true" />
    );
  }

  if (data.sessions === 0 && !snapshot?.some((e) => e.kind === "progress")) return null;

  const performance = data.series.find((s) => s.key === "role.performance");
  const rows = changeRows(snapshot, data);
  const perf = rows.find((r) => r.key === "role.performance");
  const moved = rows.filter((r) => r.key !== "role.performance");
  const flags = issueFlags(snapshot, data);
  const matches = sectionMatches(snapshot, data);

  if (!perf && moved.length === 0 && flags.length === 0) {
    // A first session has a trend of one and nothing to compare it with.
    // Saying so beats both silence and an implied "no change".
    if (data.sessions >= 1 && !data.comparison) {
      return (
        <p className="glass rounded-card px-4 py-3 text-xs text-ink-faint">
          Since your last session · first session, nothing to compare yet
        </p>
      );
    }
    if (!performance) return null;
  }

  return (
    <section className="flex flex-col gap-3" aria-label="Since your last session">
      <div className="flex items-baseline justify-between gap-3">
        <h2 className="flex items-baseline gap-2 font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          Since your last session
          {matches !== null ? (
            <span className="font-mono text-[0.625rem] normal-case tracking-normal text-ink-faint">
              {matches} {matches === 1 ? "match" : "matches"}
            </span>
          ) : null}
        </h2>
        <Link
          href="/coach/history"
          className="focus-neon rounded text-xs text-function transition-colors duration-200 ease-out hover:text-ink"
        >
          History →
        </Link>
      </div>

      <div className="grid gap-3 lg:grid-cols-2 lg:items-start">
        <Card className="flex flex-col gap-4">
          {perf ? (
            <PerformanceDelta row={perf} />
          ) : null}
          {performance ? <SeriesChart series={performance} /> : null}
        </Card>

        {moved.length > 0 || flags.length > 0 ? (
          <Card className="flex flex-col gap-2">
            <p className="text-[0.625rem] uppercase tracking-wider text-ink-faint">
              All {data.role_label ?? "role"} games
            </p>
            {moved.length > 0 ? (
              <ul className="flex flex-col divide-y divide-border/60">
                {moved.slice(0, 6).map((row) => (
                  <DeltaRow
                    key={row.key}
                    label={row.label}
                    before={row.before}
                    after={row.after}
                    change={row.change}
                    status={row.status}
                    format={(v) => formatValue(v, row.unit)}
                    formatDelta={(v) =>
                      row.unit === "percentile" || row.unit === "score"
                        ? `${Math.round(v)} pts`
                        : formatValue(v, row.unit)
                    }
                  />
                ))}
              </ul>
            ) : null}
            {flags.length > 0 ? (
              <ul className="flex flex-wrap gap-1.5 pt-1">
                {flags.map((flag) => (
                  <li
                    key={flag.key}
                    className={cn(
                      "rounded-full border px-2.5 py-0.5 text-[0.6875rem]",
                      flag.resolved
                        ? "border-string/40 bg-string/10 text-string"
                        : "border-error/40 bg-error/10 text-error",
                    )}
                  >
                    {flag.resolved ? "✓ Resolved: " : "New: "}
                    {flag.label}
                  </li>
                ))}
              </ul>
            ) : null}
          </Card>
        ) : null}
      </div>
    </section>
  );
}

interface ChangeRow {
  key: string;
  label: string;
  before: number;
  after: number;
  change: number | null;
  status: ProgressStatus;
  unit: MetricUnit;
}

/**
 * Before -> after rows. From the analysis snapshot when it carries them, so
 * these match the cards above; otherwise from the live session comparison.
 */
export function changeRows(
  snapshot: Evidence[] | undefined,
  data: ProgressResponse,
): ChangeRow[] {
  const fromSnapshot = (snapshot ?? [])
    .filter((e) => e.kind === "progress" && e.figure?.previous != null)
    .map((e) => ({
      key: e.id.replace(/^progress\./, ""),
      label: e.label.replace(/ since your last session$/i, ""),
      before: e.figure!.previous as number,
      after: e.figure!.value,
      change: e.figure!.percent_change,
      status: e.figure!.status ?? "stable",
      unit: e.figure!.unit ?? "ratio",
    }));
  if (fromSnapshot.length > 0) {
    return fromSnapshot.map((r) =>
      r.key === "performance" ? { ...r, key: "role.performance", label: "Performance" } : r,
    );
  }

  const comparison = data.comparison;
  if (!comparison) return [];
  const all = [
    ...(comparison.performance ? [comparison.performance] : []),
    ...comparison.metrics.filter((m) => m.key !== "role.performance"),
  ];
  return all
    .filter(
      (m) =>
        m.previous !== null &&
        m.current !== null &&
        (m.key === "role.performance" || notable(m.status)),
    )
    .filter((m) => m.status !== "new_issue" && m.status !== "resolved_issue")
    .map((m) => ({
      key: m.key,
      label: m.key === "role.performance" ? "Performance" : m.label,
      before: m.previous as number,
      after: m.current as number,
      change: m.percent_change,
      status: m.status,
      unit: m.unit,
    }));
}

function issueFlags(snapshot: Evidence[] | undefined, data: ProgressResponse) {
  const fromSnapshot = (snapshot ?? []).filter(
    (e) => e.id.startsWith("progress.resolved.") || e.id.startsWith("progress.new."),
  );
  if (snapshot && snapshot.some((e) => e.kind === "progress")) {
    return fromSnapshot.map((e) => ({
      key: e.id,
      label: e.id.split(".").slice(2).join(".").replace(/^pattern\./, "").replace(/_/g, " "),
      resolved: e.id.startsWith("progress.resolved."),
    }));
  }
  return (data.comparison?.metrics ?? [])
    .filter((m) => m.status === "new_issue" || m.status === "resolved_issue")
    .map((m) => ({ key: m.key, label: m.label, resolved: m.status === "resolved_issue" }));
}

/** One match count for the section. */
function sectionMatches(snapshot: Evidence[] | undefined, data: ProgressResponse): number | null {
  const progress = (snapshot ?? []).filter((e) => e.kind === "progress");
  if (progress.length > 0) return Math.max(...progress.map((e) => e.sample));
  return data.comparison?.matches ?? null;
}

/** The role score, then and now. */
function PerformanceDelta({ row }: { row: ChangeRow }) {
  return (
    <div className="flex items-baseline gap-3">
      <span className="font-mono text-2xl tabular-nums text-ink-faint">
        {Math.round(row.before)}
      </span>
      <span aria-hidden className="text-ink-faint">→</span>
      <span className={cn("font-display text-3xl tabular-nums", tone(row.status, "text"))}>
        {Math.round(row.after)}
      </span>
      <span className="text-xs text-ink-muted">
        performance
        <span className={cn("ml-2 font-mono", tone(row.status, "text"))}>
          {row.status === "improved" || row.status === "declined"
            ? `${row.after > row.before ? "▲ +" : "▼ −"}${Math.abs(
                Math.round(row.after - row.before),
              )}`
            : "unchanged"}
        </span>
      </span>
    </div>
  );
}

/** Whether a status is worth a row. */
function notable(status: ProgressStatus): boolean {
  return (
    status === "improved" ||
    status === "declined" ||
    status === "new_issue" ||
    status === "resolved_issue"
  );
}

/**
 * Colour per verdict. Never the only signal — every row carries its
 * `status_label` in words beside the dot.
 */
function tone(status: ProgressStatus, kind: "text" | "bg"): string {
  const good = kind === "text" ? "text-string" : "bg-mark-win";
  const bad = kind === "text" ? "text-error" : "bg-mark-loss";
  const neutral = kind === "text" ? "text-ink-muted" : "bg-mark-track";

  switch (status) {
    case "improved":
    case "resolved_issue":
      return good;
    case "declined":
    case "new_issue":
      return bad;
    default:
      return neutral;
  }
}
