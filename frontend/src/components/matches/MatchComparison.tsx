"use client";

import { useEffect, useState } from "react";

import { ComparisonContext } from "@/components/benchmark/ComparisonContext";
import { PercentileRow } from "@/components/charts/PercentileRow";
import { PercentileTrend } from "@/components/charts/PercentileTrend";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { ApiError, getMatchComparison } from "@/lib/api";
import type { Highlight, MatchComparisonResponse } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * This match against players in the same rank bracket on the same hero.
 *
 * Reads top to bottom as three questions: was this game good, am I getting
 * better on this hero, and what do I do about it. Every number arrives
 * computed — percentiles, the median summary, the strengths and weaknesses and
 * the suggestion are all deterministic server-side work, and nothing here
 * derives a figure from another one.
 *
 * Loads separately from the match itself so a benchmark provider outage costs
 * a section rather than the page.
 */
export function MatchComparison({ id }: { id: string }) {
  const [data, setData] = useState<MatchComparisonResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    getMatchComparison(id)
      .then((response) => {
        if (!cancelled) setData(response);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        if (e instanceof ApiError && e.isUnauthenticated) return;
        setError(
          e instanceof ApiError ? e.message : "Could not load this comparison.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, [id]);

  if (error) return <Alert>{error}</Alert>;
  if (!data) {
    return (
      <div
        className="h-56 animate-pulse rounded-card bg-surface-2"
        aria-busy="true"
      />
    );
  }

  const { standing, bracket } = data;

  return (
    <section className="flex flex-col gap-3">
      <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
        Against your rank
      </h2>

      {data.note ? <Alert tone="info">{data.note}</Alert> : null}

      <div className="grid gap-4 lg:grid-cols-2 lg:items-start">
        <Card className="flex flex-col gap-4">
          <Headline
            standing={standing.this_match}
            bracketLabel={bracket.label}
            heroName={data.hero_name}
            comparable={data.comparable}
          />

          {standing.this_match !== null ? (
            <>
              <StandingBar standing={standing.this_match} />
              <p className="text-xs leading-relaxed text-ink-faint">
                Median of {standing.metrics_counted}{" "}
                {standing.metrics_counted === 1 ? "metric" : "metrics"} below.
                {standing.hero_average !== null ? (
                  <>
                    {" "}
                    Your average on {data.hero_name} sits at p
                    {Math.round(standing.hero_average)}.
                  </>
                ) : null}
              </p>
            </>
          ) : null}
        </Card>

        {data.trend.length > 0 ? (
          <Card className="flex flex-col gap-3">
            <div className="flex items-baseline justify-between gap-3">
              <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
                Form on {data.hero_name}
              </h3>
              <Delta value={data.delta_vs_previous} />
            </div>
            <PercentileTrend points={data.trend} />
          </Card>
        ) : null}
      </div>

      {data.pros.length > 0 || data.cons.length > 0 ? (
        <div className="grid gap-4 sm:grid-cols-2 sm:items-start">
          <HighlightList
            title="Went well"
            icon="check"
            tone="good"
            items={data.pros}
            empty="Nothing stood out above your bracket this game."
          />
          <HighlightList
            title="Held you back"
            icon="alert"
            tone="bad"
            items={data.cons}
            empty="Nothing fell below your bracket this game."
          />
        </div>
      ) : null}

      {data.suggestion ? (
        <Card glow="keyword" className="flex items-start gap-3">
          <Icon name="spark" className="mt-0.5 size-5 shrink-0 text-keyword" />
          <div className="flex min-w-0 flex-col gap-1">
            <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
              Work on this
            </h3>
            <p className="text-sm leading-relaxed text-ink">
              {data.suggestion.text}
            </p>
          </div>
        </Card>
      ) : null}

      {data.metrics.length > 0 ? (
        <Card className="flex flex-col gap-5">
          <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
            <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
              Every metric, same scale
            </h3>
            <MarkLegend />
          </div>

          {data.metrics.map((row) => (
            <PercentileRow key={row.metric} row={row} />
          ))}

          <ComparisonContext
            context={data.context}
            className="border-t border-border pt-4"
          />
        </Card>
      ) : null}
    </section>
  );
}

/** The one sentence the section exists to deliver. */
function Headline({
  standing,
  bracketLabel,
  heroName,
  comparable,
}: {
  standing: number | null;
  bracketLabel: string;
  heroName: string;
  comparable: boolean;
}) {
  if (standing === null) {
    return (
      <p className="text-sm leading-relaxed text-ink-muted">
        {comparable
          ? `Not enough data to place this game against ${bracketLabel} players on ${heroName}.`
          : `This game was not compared against ${bracketLabel} players — see the note above.`}
      </p>
    );
  }

  const rounded = Math.round(standing);

  return (
    <div className="flex items-baseline gap-3">
      <span
        className={cn(
          "font-display text-4xl tabular-nums",
          rounded >= 70
            ? "text-string"
            : rounded <= 30
              ? "text-error"
              : "text-number",
        )}
      >
        {rounded}%
      </span>
      <p className="min-w-0 text-sm leading-snug text-ink-muted">
        of {bracketLabel} players on{" "}
        <span className="text-ink">{heroName}</span> did worse than this game.
      </p>
    </div>
  );
}

/** The standing on the same 0-100 track the metric rows use. */
function StandingBar({ standing }: { standing: number }) {
  return (
    <div className="relative h-6" aria-hidden>
      <div className="absolute inset-x-0 top-1/2 h-2.5 -translate-y-1/2 overflow-hidden rounded-full bg-mark-track">
        <div className="absolute inset-y-0 left-0 w-[30%] bg-mark-loss/25" />
        <div className="absolute inset-y-0 right-0 w-[30%] bg-mark-win/25" />
      </div>
      <span
        className="absolute top-1/2 h-4 w-px -translate-y-1/2 bg-ink-faint/60"
        style={{ left: "50%" }}
      />
      <span
        className={cn(
          "absolute top-1/2 size-4 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-surface-2",
          standing >= 70
            ? "bg-mark-win"
            : standing <= 30
              ? "bg-mark-loss"
              : "bg-mark-line",
        )}
        style={{ left: `${Math.max(2, Math.min(98, standing))}%` }}
      />
    </div>
  );
}

/**
 * Change against the previous game on this hero, in percentile points.
 *
 * Signed text as well as colour — the sign is the information, and a green pill
 * alone does not survive a colour-vision deficiency or a greyscale print.
 */
function Delta({ value }: { value: number | null }) {
  if (value === null) {
    return (
      <span className="text-[0.6875rem] text-ink-faint">
        no earlier game to compare
      </span>
    );
  }

  const rounded = Math.round(value);
  if (rounded === 0) {
    return (
      <span className="rounded bg-mark-track px-1.5 py-0.5 font-mono text-[0.625rem] tabular-nums text-ink-muted">
        level with your last game
      </span>
    );
  }

  return (
    <span
      className={cn(
        "rounded px-1.5 py-0.5 font-mono text-[0.625rem] tabular-nums",
        rounded > 0 ? "bg-mark-win/15 text-string" : "bg-mark-loss/15 text-error",
      )}
    >
      {rounded > 0 ? `+${rounded}` : rounded} vs your last game
    </span>
  );
}

function HighlightList({
  title,
  icon,
  tone,
  items,
  empty,
}: {
  title: string;
  icon: "check" | "alert";
  tone: "good" | "bad";
  items: Highlight[];
  empty: string;
}) {
  return (
    <Card className="flex flex-col gap-3">
      <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
        {title}
      </h3>

      {items.length === 0 ? (
        <p className="text-sm text-ink-faint">{empty}</p>
      ) : (
        <ul className="flex flex-col gap-3">
          {items.map((item) => (
            <li key={item.metric} className="flex items-start gap-2.5">
              <Icon
                name={icon}
                className={cn(
                  "mt-0.5 size-4 shrink-0",
                  tone === "good" ? "text-string" : "text-error",
                )}
              />
              <div className="flex min-w-0 flex-col gap-0.5">
                <span className="flex items-baseline gap-2 text-sm text-ink">
                  {item.label}
                  <span className="font-mono text-[0.625rem] tabular-nums text-ink-faint">
                    p{Math.round(item.percentile)}
                  </span>
                </span>
                <span className="text-[0.6875rem] leading-relaxed text-ink-faint">
                  {item.detail}
                </span>
              </div>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

/** Names the two marks, so shape is never the only thing carrying identity. */
function MarkLegend() {
  return (
    <span className="flex items-center gap-3 text-[0.6875rem] text-ink-faint">
      <span className="inline-flex items-center gap-1.5">
        <span aria-hidden className="size-2.5 rounded-full bg-mark-line" />
        this match
      </span>
      <span className="inline-flex items-center gap-1.5">
        <span
          aria-hidden
          className="size-2.5 rounded-full border-2 border-ink-muted bg-surface-2"
        />
        your average
      </span>
    </span>
  );
}
