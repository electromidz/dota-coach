"use client";

import { useEffect, useState } from "react";
import Link from "next/link";

import { Card } from "@/components/ui/Card";
import { InfoTip } from "@/components/ui/InfoTip";
import { SectionError } from "@/components/ui/SectionError";
import { ApiError, getPlayerModel } from "@/lib/api";
import { formatPercent } from "@/lib/stats";
import type {
  ModelConfidence,
  PatternStatus,
  PlayerModelResponse,
  PlayerTrait,
  RecurringPattern,
} from "@/lib/types";
import { cn, fixOrdinals } from "@/lib/utils";

const CONFIDENCE_CLASS: Record<ModelConfidence, string> = {
  sparse: "border-border bg-surface-2 text-ink-muted",
  developing: "border-number/50 bg-number/10 text-number",
  established: "border-string/50 bg-string/10 text-string",
};

const STATUS_CLASS: Record<PatternStatus, string> = {
  active: "border-error/50 bg-error/10 text-error",
  improving: "border-number/50 bg-number/10 text-number",
  resolved: "border-string/50 bg-string/10 text-string",
};

/**
 * What the backend believes about this player, and how much that belief is
 * worth.
 *
 * Entirely deterministic: it renders with no model configured, and nothing
 * here was written by one. The confidence badge is not decoration — it is the
 * difference between "we have seen this forty times" and "this is a first
 * impression".
 */
export function PlayerModelPanel() {
  const [data, setData] = useState<PlayerModelResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** Bumped by Retry to re-run the fetch below. */
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;

    getPlayerModel()
      .then((response) => {
        if (!cancelled) setData(response);
      })
      .catch((e) => {
        if (cancelled) return;
        if (e instanceof ApiError && e.isUnauthenticated) return;
        setError(
          e instanceof ApiError ? e.message : "Could not load your model.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, [attempt]);

  if (error) {
    return (
      <SectionError
        section="Player model"
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
      <div
        className="h-48 animate-pulse rounded-card bg-surface-2"
        aria-busy="true"
      />
    );
  }

  const { model, unmeasurable, thresholds } = data;
  const form = model.recent_form;

  return (
    <section className="flex flex-col gap-4">
      <header className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          What we know about you
        </h2>
        <span className="flex items-center gap-1.5">
          <span
            className={cn(
              "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
              CONFIDENCE_CLASS[model.confidence],
            )}
          >
            {model.confidence_label}
          </span>
          <InfoTip label="About this confidence" align="end">
            {model.confidence_caveat}
          </InfoTip>
        </span>
      </header>

      <Card className="flex flex-col gap-3">

        <dl className="grid grid-cols-2 gap-x-4 gap-y-4 lg:grid-cols-4">
          <Figure label="Matches read" value={model.matches_analyzed.toString()} />
          <Figure
            label={`Last ${form.matches}`}
            value={formatPercent(form.win_rate)}
          />
          <Figure
            label="Streak"
            value={
              form.streak === 0
                ? "—"
                : `${Math.abs(form.streak)} ${form.streak > 0 ? "W" : "L"}`
            }
          />
          <Figure
            label="Main role"
            value={model.preferred_roles[0]?.role ?? "—"}
          />
        </dl>

        {model.preferred_roles.length > 0 ? (
          <ul className="m-0 flex list-none flex-wrap gap-3 p-0">
            {model.preferred_roles.map((role) => (
              <li
                key={role.role}
                className="font-mono text-[0.6875rem] text-ink-faint"
              >
                {role.role} {formatPercent(role.share)} ·{" "}
                {formatPercent(role.win_rate)} won
              </li>
            ))}
          </ul>
        ) : null}
      </Card>

      <div className="grid gap-3 md:grid-cols-2">
        <TraitList title="Strengths" tone="good" traits={model.strengths} empty="None yet" />
        <TraitList title="Weaknesses" tone="bad" traits={model.weaknesses} empty="None yet" />
      </div>

      <div className="flex flex-col gap-3">
        <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
          Recurring patterns
        </h3>

        {model.patterns.length > 0 ? (
          model.patterns.map((pattern) => (
            <PatternCard key={pattern.id} pattern={pattern} />
          ))
        ) : (
          <p className="flex items-center gap-1.5 text-xs text-ink-faint">
            None with enough evidence
            <InfoTip label="What counts as a pattern">
              Measurable in {thresholds.min_measured} matches and happening in at
              least {formatPercent(thresholds.min_rate)} of them. One bad game is
              never a habit.
            </InfoTip>
          </p>
        )}

        {model.resolved_patterns.length > 0 ? (
          <details className="group">
            <summary className="focus-neon min-h-11 cursor-pointer list-none text-xs uppercase tracking-wider text-ink-faint transition-colors duration-200 hover:text-ink">
              {model.resolved_patterns.length} you have fixed
            </summary>
            <div className="mt-3 flex flex-col gap-3">
              {model.resolved_patterns.map((pattern) => (
                <PatternCard key={pattern.id} pattern={pattern} />
              ))}
            </div>
          </details>
        ) : null}

        {unmeasurable.length > 0 ? (
          <Card className="flex flex-col gap-2">
            <h4 className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
              Not enough data to check
              <InfoTip label="Why these are silent">
                These could not be measured, which is not the same as passing them.
              </InfoTip>
            </h4>
            <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
              {unmeasurable.map((detector) => (
                <li key={detector.id} className="text-xs text-ink-muted">
                  {detector.label}
                  <span className="ml-1.5 font-mono text-[0.6875rem] text-ink-faint">
                    {detector.measured}/{detector.required} matches
                  </span>
                </li>
              ))}
            </ul>
          </Card>
        ) : null}
      </div>
    </section>
  );
}

function PatternCard({ pattern }: { pattern: RecurringPattern }) {
  const rate = pattern.measured > 0 ? pattern.occurrences / pattern.measured : 0;
  return (
    <div className="glass flex flex-wrap items-center gap-x-3 gap-y-2 rounded-card px-4 py-3">
      <span
        className={cn(
          "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
          STATUS_CLASS[pattern.status],
        )}
      >
        {pattern.status_label}
      </span>
      <span className="min-w-0 flex-1 truncate text-sm text-ink">{pattern.label}</span>
      <span className="flex items-center gap-2">
        <span
          role="img"
          aria-label={`${pattern.occurrences} of ${pattern.measured} matches`}
          className="h-1.5 w-20 overflow-hidden rounded-full bg-mark-track"
        >
          <span
            className={cn(
              "block h-full rounded-full",
              pattern.status === "resolved" ? "bg-mark-win" : "bg-mark-loss",
            )}
            style={{ width: `${Math.round(rate * 100)}%` }}
          />
        </span>
        <span className="font-mono text-xs tabular-nums text-ink-faint">
          {pattern.occurrences}/{pattern.measured}
        </span>
        <InfoTip label={`About ${pattern.label}`} align="end">
          <span>{fixOrdinals(pattern.statement)}</span>
          <span className="text-ink-faint">{pattern.description}</span>
          {pattern.examples.length > 0 ? (
            <span className="flex flex-wrap gap-2">
              {pattern.examples.map((id, index) => (
                <Link key={id} href={`/matches/${id}`} className="text-function hover:text-ink">
                  match {index + 1}
                </Link>
              ))}
            </span>
          ) : null}
        </InfoTip>
      </span>
    </div>
  );
}

function TraitList({
  title,
  tone,
  traits,
  empty,
}: {
  title: string;
  tone: "good" | "bad";
  traits: PlayerTrait[];
  empty: string;
}) {
  return (
    <Card className="flex flex-col gap-3">
      <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
        {title}
      </h3>
      {traits.length > 0 ? (
        <ul className="m-0 flex list-none flex-wrap gap-1.5 p-0">
          {traits.map((trait) => (
            <li
              key={trait.key}
              className={cn(
                "flex items-center gap-1 rounded-full border px-2.5 py-0.5 text-[0.6875rem]",
                tone === "good"
                  ? "border-string/40 bg-string/10 text-string"
                  : "border-error/40 bg-error/10 text-error",
              )}
            >
              {trait.label}
              <InfoTip label={trait.label} align="end">
                {fixOrdinals(trait.statement)}
              </InfoTip>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-xs text-ink-faint">{empty}</p>
      )}
    </Card>
  );
}

function Figure({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-1">
      <dt className="text-xs uppercase tracking-wider text-ink-faint">
        {label}
      </dt>
      <dd className="font-mono text-lg tabular-nums text-number">{value}</dd>
    </div>
  );
}
