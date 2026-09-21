"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { formatValue, SeriesChart } from "@/components/charts/SeriesChart";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { ApiError, getCoachingProgress } from "@/lib/api";
import type { MetricProgress, ProgressResponse, ProgressStatus } from "@/lib/types";
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
export function ProgressSummary() {
  const [data, setData] = useState<ProgressResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

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
  }, []);

  if (error) return <Alert>{error}</Alert>;
  if (!data) {
    return (
      <div className="h-40 animate-pulse rounded-card bg-surface-2" aria-busy="true" />
    );
  }

  // Nothing recorded yet. Not worth a card — the coach page has plenty to say
  // before a player has two sessions.
  if (data.sessions === 0) return null;

  const performance = data.series.find((s) => s.key === "role.performance");
  const comparison = data.comparison;
  const movements = (comparison?.metrics ?? []).filter((m) => notable(m.status));

  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-baseline justify-between gap-3">
        <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          Since your last session
        </h2>
        <Link
          href="/coach/history"
          className="focus-neon rounded text-xs text-function transition-colors duration-200 ease-out hover:text-ink"
        >
          History →
        </Link>
      </div>

      {data.note ? <Alert tone="info">{data.note}</Alert> : null}

      <div className="grid gap-4 lg:grid-cols-2 lg:items-start">
        <Card className="flex flex-col gap-4">
          {comparison?.headline ? (
            <p className="text-sm leading-relaxed text-ink">
              {comparison.headline}
            </p>
          ) : comparison ? (
            <p className="text-sm leading-relaxed text-ink-muted">
              Nothing moved far enough to call a change since session{" "}
              {comparison.previous_sequence}.
            </p>
          ) : null}

          {comparison?.performance ? (
            <PerformanceDelta progress={comparison.performance} />
          ) : null}

          {performance ? (
            <div className="flex flex-col gap-1">
              <p className="text-xs uppercase tracking-wider text-ink-faint">
                Performance across sessions
              </p>
              <SeriesChart series={performance} />
            </div>
          ) : null}
        </Card>

        {movements.length > 0 ? (
          <Card className="flex flex-col gap-3">
            <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
              What moved
            </h3>
            <ul className="flex flex-col gap-2.5">
              {movements.slice(0, 6).map((metric) => (
                <MovementRow key={metric.key} metric={metric} />
              ))}
            </ul>
          </Card>
        ) : null}
      </div>
    </section>
  );
}

/** The role score, then and now. */
function PerformanceDelta({ progress }: { progress: MetricProgress }) {
  const { previous, current, direction_delta: delta } = progress;
  if (previous === null || current === null) return null;

  return (
    <div className="flex items-baseline gap-3">
      <span className="font-mono text-2xl tabular-nums text-ink-faint">
        {Math.round(previous)}
      </span>
      <Icon name="chevron-right" className="size-4 shrink-0 text-ink-faint" />
      <span
        className={cn(
          "font-display text-3xl tabular-nums",
          tone(progress.status, "text"),
        )}
      >
        {Math.round(current)}
      </span>
      <span className="text-sm text-ink-muted">
        performance
        {delta !== null && progress.status !== "stable" ? (
          <span className={cn("ml-2 font-mono text-xs", tone(progress.status, "text"))}>
            {delta > 0 ? "+" : ""}
            {Math.round(delta)}
          </span>
        ) : null}
      </span>
    </div>
  );
}

function MovementRow({ metric }: { metric: MetricProgress }) {
  return (
    <li className="flex items-start gap-2.5">
      <span
        aria-hidden
        className={cn(
          "mt-1.5 size-2 shrink-0 rounded-full",
          tone(metric.status, "bg"),
        )}
      />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="flex flex-wrap items-baseline justify-between gap-x-2 text-sm text-ink">
          {metric.label}
          <span className={cn("text-[0.6875rem]", tone(metric.status, "text"))}>
            {metric.status_label}
          </span>
        </span>

        {metric.previous !== null && metric.current !== null ? (
          <span className="font-mono text-[0.6875rem] tabular-nums text-ink-faint">
            {formatValue(metric.previous, metric.unit)} →{" "}
            {formatValue(metric.current, metric.unit)}
          </span>
        ) : null}

        {metric.note ? (
          <span className="text-[0.6875rem] leading-relaxed text-ink-faint">
            {metric.note}
          </span>
        ) : null}
      </div>
    </li>
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
