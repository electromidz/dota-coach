"use client";

import { useEffect, useState } from "react";
import Link from "next/link";

import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { ApiError, getPlayerModel } from "@/lib/api";
import { formatPercent } from "@/lib/stats";
import type {
  ModelConfidence,
  PatternStatus,
  PlayerModelResponse,
  PlayerTrait,
  RecurringPattern,
} from "@/lib/types";
import { cn } from "@/lib/utils";

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
  }, []);

  if (error) return <Alert>{error}</Alert>;
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
        <span
          className={cn(
            "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
            CONFIDENCE_CLASS[model.confidence],
          )}
        >
          {model.confidence_label}
        </span>
      </header>

      <Card className="flex flex-col gap-3">
        <p className="text-xs leading-relaxed text-ink-muted">
          {model.confidence_caveat}
        </p>

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

      <TraitList
        title="Strengths"
        traits={model.strengths}
        empty="Nothing stands out above your peers yet."
      />
      <TraitList
        title="Weaknesses"
        traits={model.weaknesses}
        empty="Nothing stands out below your peers yet."
      />

      <div className="flex flex-col gap-3">
        <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
          Recurring patterns
        </h3>

        {model.patterns.length > 0 ? (
          model.patterns.map((pattern) => (
            <PatternCard key={pattern.id} pattern={pattern} />
          ))
        ) : (
          <Card>
            <p className="text-sm leading-relaxed text-ink-muted">
              No recurring pattern has enough evidence behind it. A pattern
              needs to be measurable in {thresholds.min_measured} matches and
              happen in at least {formatPercent(thresholds.min_rate)} of them —
              one bad game is never a habit.
            </p>
          </Card>
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
            <h4 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
              Not enough data to check
            </h4>
            <p className="text-xs leading-relaxed text-ink-faint">
              These are silent because they could not be measured, which is not
              the same as passing them.
            </p>
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
  return (
    <Card className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-2">
        <span
          className={cn(
            "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
            STATUS_CLASS[pattern.status],
          )}
        >
          {pattern.status_label}
        </span>
        <h4 className="font-display text-sm text-ink">{pattern.label}</h4>
        <span className="font-mono text-xs tabular-nums text-ink-faint">
          {pattern.occurrences}/{pattern.measured}
        </span>
      </div>

      <p className="text-sm leading-relaxed text-ink-muted">
        {pattern.statement}
      </p>
      <p className="text-xs leading-relaxed text-ink-faint">
        {pattern.description}
      </p>

      {pattern.examples.length > 0 ? (
        <p className="flex flex-wrap items-center gap-2 text-xs text-ink-faint">
          <span>Recent examples:</span>
          {pattern.examples.map((id, index) => (
            <Link
              key={id}
              href={`/matches/${id}`}
              className="focus-neon rounded text-function transition-colors duration-200 hover:text-ink"
            >
              match {index + 1}
            </Link>
          ))}
        </p>
      ) : null}
    </Card>
  );
}

function TraitList({
  title,
  traits,
  empty,
}: {
  title: string;
  traits: PlayerTrait[];
  empty: string;
}) {
  return (
    <Card className="flex flex-col gap-3">
      <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
        {title}
      </h3>

      {traits.length > 0 ? (
        <ul className="m-0 flex list-none flex-col gap-2 p-0">
          {traits.map((trait) => (
            <li key={trait.key} className="flex flex-col gap-0.5">
              <span className="text-xs uppercase tracking-wider text-ink-muted">
                {trait.label}
              </span>
              <span className="text-sm leading-relaxed text-ink">
                {trait.statement}
              </span>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-sm text-ink-faint">{empty}</p>
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
