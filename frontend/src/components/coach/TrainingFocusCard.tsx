"use client";

import { useEffect, useState } from "react";

import { Sparkline } from "@/components/charts/Sparkline";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { ApiError, getTrainingFocus } from "@/lib/api";
import { formatFocusValue } from "@/lib/training";
import type { TrainingFocusResponse } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * The answer to "what should I do next?".
 *
 * The spec's dashboard order is focus, then why it matters, then progress —
 * and everything else after. This card is that order, and it leads the page
 * for the same reason: a screen of statistics is not coaching.
 *
 * Every number is deterministic. The model is not consulted to produce any of
 * it, which is why it renders on a deployment with no LLM at all.
 */
export function TrainingFocusCard({ compact = false }: { compact?: boolean }) {
  const [data, setData] = useState<TrainingFocusResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    getTrainingFocus()
      .then((response) => {
        if (!cancelled) setData(response);
      })
      .catch((e) => {
        if (cancelled) return;
        if (e instanceof ApiError && e.isUnauthenticated) return;
        setError(
          e instanceof ApiError ? e.message : "Could not load your focus.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, []);

  if (error) return <Alert>{error}</Alert>;
  if (!data) {
    return (
      <div
        className="h-44 animate-pulse rounded-card bg-surface-2"
        aria-busy="true"
      />
    );
  }

  const { focus, progress } = data;

  if (!focus) {
    return data.note ? <Alert tone="info">{data.note}</Alert> : null;
  }

  const measure = focus.measure;
  const values = progress?.points.map((point) => point.value) ?? [];
  const pct = Math.round((focus.progress ?? 0) * 100);

  return (
    <section className="flex flex-col gap-4">
      <header className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          Current training focus
        </h2>
        {focus.target_met ? (
          <span className="rounded-full border border-string/50 bg-string/10 px-2 py-0.5 text-[0.625rem] uppercase tracking-wider text-string">
            Target met
          </span>
        ) : null}
      </header>

      <Card className="flex flex-col gap-4" glow="keyword">
        <div className="flex flex-col gap-1.5">
          <h3 className="font-display text-lg text-ink">{focus.title}</h3>
          <p className="text-sm leading-relaxed text-ink-muted">{focus.why}</p>
        </div>

        {/* Baseline, now, target — the three numbers that make a goal
            checkable, on one line so the direction is obvious. */}
        <dl className="grid grid-cols-3 gap-3">
          <Figure
            label="When set"
            value={formatFocusValue(measure, focus.baseline_value)}
          />
          <Figure
            label="Now"
            value={formatFocusValue(measure, focus.current_value)}
            emphasis
          />
          <Figure
            label="Target"
            value={formatFocusValue(measure, focus.target_value)}
          />
        </dl>

        <figure className="m-0 flex flex-col gap-2">
          <figcaption className="flex items-baseline justify-between gap-3">
            <span className="text-xs uppercase tracking-wider text-ink-faint">
              Progress toward the target
            </span>
            <span className="font-mono text-sm tabular-nums text-ink">
              {focus.progress === null ? "—" : `${pct}%`}
            </span>
          </figcaption>

          <div
            role="meter"
            aria-valuenow={pct}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label="Progress toward the target"
            className="h-2.5 w-full overflow-hidden rounded-full bg-mark-track"
          >
            <div
              className="h-full rounded-r bg-mark-win transition-[width] duration-500 ease-out"
              style={{ width: `${pct}%` }}
            />
          </div>
        </figure>

        {values.length > 1 ? (
          <figure className="m-0 flex flex-col gap-2">
            <figcaption className="text-xs uppercase tracking-wider text-ink-faint">
              {progress?.label} · {progress?.window}-match averages
            </figcaption>
            <Sparkline
              values={values}
              label={progress?.label ?? focus.measure_label}
              formatValue={(value) => formatFocusValue(measure, value)}
            />
          </figure>
        ) : (
          <p className="text-xs text-ink-faint">
            Not enough matches yet to plot a trend for this.
          </p>
        )}

        {!compact ? <WhyThisOne data={data} /> : null}
      </Card>
    </section>
  );
}

/** The selection score, its parts, and what came second. */
function WhyThisOne({ data }: { data: TrainingFocusResponse }) {
  const focus = data.focus;
  if (!focus) return null;

  return (
    <details className="group">
      <summary className="focus-neon min-h-11 cursor-pointer list-none text-xs uppercase tracking-wider text-ink-faint transition-colors duration-200 hover:text-ink">
        Why this one?
      </summary>

      <div className="mt-3 flex flex-col gap-4">
        <ul className="m-0 flex list-none flex-col gap-2 p-0">
          {focus.score_parts.map((part) => (
            <li key={part.key} className="flex flex-col gap-1">
              <span className="flex items-baseline justify-between gap-3">
                <span className="text-xs text-ink-muted">
                  {part.label}
                  <span className="ml-1.5 font-mono text-[0.625rem] text-ink-faint">
                    {Math.round(part.weight * 100)}%
                  </span>
                </span>
                <span className="font-mono text-xs tabular-nums text-ink">
                  {Math.round(part.score)}
                </span>
              </span>
              <div className="h-1.5 w-full overflow-hidden rounded-full bg-mark-track">
                <div
                  className="h-full rounded-r bg-operator"
                  style={{
                    width: `${Math.max(0, Math.min(100, part.score))}%`,
                  }}
                />
              </div>
              <span className="text-[0.6875rem] leading-relaxed text-ink-faint">
                {part.detail}
              </span>
            </li>
          ))}
        </ul>

        {data.next_up.length > 0 ? (
          <div className="flex flex-col gap-1">
            <span className="text-xs uppercase tracking-wider text-ink-faint">
              Next in line
            </span>
            {data.next_up.map((candidate) => (
              <span key={candidate.key} className="text-xs text-ink-muted">
                {candidate.title}
                <span className="ml-1.5 font-mono text-[0.6875rem] text-ink-faint">
                  {Math.round(candidate.score)}
                </span>
              </span>
            ))}
          </div>
        ) : null}

        {data.history.filter((f) => f.status !== "active").length > 0 ? (
          <div className="flex flex-col gap-1">
            <span className="text-xs uppercase tracking-wider text-ink-faint">
              Previously
            </span>
            {data.history
              .filter((f) => f.status !== "active")
              .map((past) => (
                <span key={past.id ?? past.key} className="text-xs text-ink-muted">
                  {past.title}
                  <span
                    className={cn(
                      "ml-1.5 text-[0.6875rem]",
                      past.status === "achieved" ? "text-string" : "text-ink-faint",
                    )}
                  >
                    {past.status_label.toLowerCase()}
                  </span>
                </span>
              ))}
          </div>
        ) : null}
      </div>
    </details>
  );
}

function Figure({
  label,
  value,
  emphasis = false,
}: {
  label: string;
  value: string;
  emphasis?: boolean;
}) {
  return (
    <div className="flex flex-col gap-1">
      <dt className="text-xs uppercase tracking-wider text-ink-faint">
        {label}
      </dt>
      <dd
        className={cn(
          "font-mono tabular-nums",
          emphasis ? "text-lg text-number" : "text-sm text-ink-muted",
        )}
      >
        {value}
      </dd>
    </div>
  );
}
