"use client";

import { useEffect, useState } from "react";

import Link from "next/link";

import { formatMetricValue } from "@/components/charts/BulletRow";
import { Sparkline } from "@/components/charts/Sparkline";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { ApiError, getTrainingFocus } from "@/lib/api";
import { CONFIDENCE_LABEL, CONFIDENCE_NOTE } from "@/lib/confidence";
import { formatFocusValue } from "@/lib/training";
import type { PreliminaryFocus, TrainingFocusResponse } from "@/lib/types";
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
  /** The backend has no role to train for yet. A prompt, not a failure. */
  const [needsRole, setNeedsRole] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    getTrainingFocus()
      .then((response) => {
        if (!cancelled) setData(response);
      })
      .catch((e) => {
        if (cancelled) return;
        if (e instanceof ApiError && e.isUnauthenticated) return;
        // A focus belongs to a role, so there is nothing to show until one is
        // chosen. The backend says which of the two reasons applies; either
        // way the answer is an invitation rather than an error.
        if (e instanceof ApiError && e.status === 409) {
          setNeedsRole(e.message);
          return;
        }
        setError(
          e instanceof ApiError ? e.message : "Could not load your focus.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, []);

  if (error) return <Alert>{error}</Alert>;

  if (needsRole) {
    return (
      <Card className="flex flex-col gap-3">
        <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          Start coaching
        </h2>
        <p className="text-sm leading-relaxed text-ink-muted">{needsRole}</p>
        <Link
          href="/coach"
          className="focus-neon w-fit cursor-pointer rounded text-sm text-function transition-colors duration-200 ease-out hover:text-ink"
        >
          Choose a role →
        </Link>
      </Card>
    );
  }

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
    // Nothing clears the bar for a goal. That is not the same as nothing being
    // known, and the backend says which of the two this is.
    if (data.preliminary) return <PreliminaryCard reading={data.preliminary} />;
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

/**
 * The early-signal variant: something measurable to look at, labelled as what
 * it is.
 *
 * Three things keep it from reading as a conclusion, and all three are
 * deliberate rather than decorative:
 *
 *   - a different heading and no `glow`, so it does not occupy the same visual
 *     slot as an actual focus;
 *   - the confidence badge and sample size sit *above* the number rather than
 *     in a footnote under it;
 *   - no progress meter and no target, because there is no goal here — a
 *     progress bar would imply the system had committed to this.
 *
 * The percentile renders only when the backend supplied one. When it did not,
 * the peer median is shown instead and plainly named as a comparison of
 * averages; nothing on this card is derived here.
 */
function PreliminaryCard({ reading }: { reading: PreliminaryFocus }) {
  const ranked = reading.percentile !== null;

  return (
    <section className="flex flex-col gap-4">
      <header className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          Potential training focus
        </h2>
        <span className="rounded-full border border-glass-edge bg-mark-track px-2 py-0.5 text-[0.625rem] uppercase tracking-wider text-ink-muted">
          {CONFIDENCE_LABEL[reading.confidence]}
        </span>
      </header>

      <Card className="flex flex-col gap-4">
        <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
          <h3 className="font-display text-lg text-ink">{reading.label}</h3>
          <span className="flex items-baseline gap-2">
            <span className="font-mono text-lg tabular-nums text-number">
              {formatMetricValue(reading.player_value)}
            </span>
            {ranked ? (
              <span className="rounded bg-mark-track px-1.5 py-0.5 font-mono text-[0.625rem] tabular-nums text-ink-muted">
                p{Math.round(reading.percentile as number)}
              </span>
            ) : (
              <span className="rounded bg-mark-track px-1.5 py-0.5 text-[0.625rem] text-ink-faint">
                unranked
              </span>
            )}
          </span>
        </div>

        <p className="text-sm leading-relaxed text-ink-muted">{reading.why}</p>

        {reading.peer_median !== null ? (
          <dl className="grid grid-cols-2 gap-3">
            <Figure
              label="You"
              value={formatMetricValue(reading.player_value)}
              emphasis
            />
            <Figure
              label="Peer median"
              value={formatMetricValue(reading.peer_median)}
            />
          </dl>
        ) : null}

        {/* The whole point of the card: say why this is not yet a conclusion,
            and what would make it one. */}
        <div className="flex flex-col gap-1.5 rounded-xl border border-glass-edge bg-mark-track/40 p-3">
          <p className="text-xs leading-relaxed text-ink-muted">
            This is an early signal, not a reliable conclusion — no target has
            been set against it and no progress is being tracked yet.
          </p>
          <p className="text-xs leading-relaxed text-ink-faint">
            {reading.to_confirm}
          </p>
          {CONFIDENCE_NOTE[reading.confidence] ? (
            <p className="text-xs leading-relaxed text-ink-faint">
              {CONFIDENCE_NOTE[reading.confidence]}
            </p>
          ) : null}
        </div>

        <p className="text-xs text-ink-faint">
          Keep playing in this role and it will either firm up into a focus or
          drop away.
        </p>
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
