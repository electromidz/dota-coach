"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { formatValue } from "@/components/charts/SeriesChart";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { Icon } from "@/components/ui/Icon";
import { SteamLoginButton } from "@/components/ui/SteamLoginButton";
import { ApiError, getCoachingSession } from "@/lib/api";
import type { CoachingSession, PlayerTrait } from "@/lib/types";

type State =
  | { kind: "loading" }
  | { kind: "ready"; session: CoachingSession }
  | { kind: "anonymous" }
  | { kind: "error"; message: string };

/**
 * One coaching session, exactly as it was recorded.
 *
 * Every figure on this page is the figure that was measured when the session
 * was written, not the player's current one. That is the whole point of
 * storing it: a session showing 54 must keep showing 54 after the player
 * reaches 61, or the improvement has nowhere to be visible from.
 */
export function SessionDetail({ id }: { id: string }) {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;

    getCoachingSession(id)
      .then((response) => {
        if (!cancelled) setState({ kind: "ready", session: response.session });
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        if (error instanceof ApiError && error.isUnauthenticated) {
          setState({ kind: "anonymous" });
          return;
        }
        setState({
          kind: "error",
          message:
            error instanceof ApiError
              ? error.message
              : "Could not load this session.",
        });
      });

    return () => {
      cancelled = true;
    };
  }, [id]);

  if (state.kind === "loading") {
    return (
      <div className="flex flex-col gap-4" aria-busy="true">
        <span className="sr-only">Loading session…</span>
        <div className="glass h-32 animate-pulse rounded-card" />
        <div className="glass h-64 animate-pulse rounded-card" />
      </div>
    );
  }

  if (state.kind === "anonymous") {
    return (
      <div className="flex flex-col gap-4">
        <Alert tone="info">Sign in with Steam to view your coaching sessions.</Alert>
        <SteamLoginButton className="w-full" />
      </div>
    );
  }

  if (state.kind === "error") {
    return (
      <div className="flex flex-col gap-4">
        <BackLink />
        {/* A session owned by someone else is reported as not found, by design. */}
        <Alert title="Session unavailable">{state.message}</Alert>
      </div>
    );
  }

  const { session } = state;

  return (
    <div className="flex flex-col gap-6 pb-8">
      <BackLink />

      <Card className="flex flex-col gap-4">
        <div className="flex items-baseline justify-between gap-3">
          <div className="flex min-w-0 flex-col gap-1">
            <h1 className="font-display text-xl tracking-wide">
              Session #{session.sequence}
            </h1>
            <p className="text-sm text-ink-muted">
              <span className="text-operator">{session.role_label}</span>
              {" · "}
              {session.analyzed_match_count}{" "}
              {session.analyzed_match_count === 1 ? "match" : "matches"}
            </p>
          </div>

          {session.performance !== null ? (
            <span className="font-display text-4xl tabular-nums text-number">
              {Math.round(session.performance)}
            </span>
          ) : null}
        </div>

        <p className="text-xs text-ink-faint">
          Recorded {new Date(session.created_at).toLocaleString()}. These are
          the figures as they stood then — they do not change as you play.
        </p>
      </Card>

      {session.metrics.length > 0 ? (
        <section className="flex flex-col gap-3">
          <h2 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
            Measured
          </h2>
          <Card>
            <dl className="grid grid-cols-2 gap-x-4 gap-y-5 lg:grid-cols-3">
              {session.metrics.map((metric) => (
                <div key={metric.key} className="flex flex-col gap-1">
                  <dt className="text-xs uppercase tracking-wider text-ink-faint">
                    {metric.label}
                  </dt>
                  <dd className="font-mono text-lg tabular-nums text-number">
                    {formatValue(metric.value, metric.unit)}
                  </dd>
                  <dd className="text-[0.625rem] text-ink-faint">
                    over {metric.sample}{" "}
                    {metric.sample === 1 ? "match" : "matches"}
                  </dd>
                </div>
              ))}
            </dl>
          </Card>
        </section>
      ) : null}

      {session.strengths.length > 0 || session.weaknesses.length > 0 ? (
        <div className="grid gap-4 sm:grid-cols-2 sm:items-start">
          <TraitList
            title="Strengths then"
            icon="check"
            tone="text-string"
            traits={session.strengths}
          />
          <TraitList
            title="Weaknesses then"
            icon="alert"
            tone="text-error"
            traits={session.weaknesses}
          />
        </div>
      ) : null}

      {session.heroes.length > 0 ? (
        <section className="flex flex-col gap-3">
          <h2 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
            Heroes then
          </h2>
          <Card>
            <ul className="flex flex-col gap-3">
              {session.heroes.map((hero) => (
                <li key={hero.hero_id} className="flex items-center gap-3">
                  <HeroPortrait
                    heroId={hero.hero_id}
                    heroName={hero.hero_name}
                    size="sm"
                  />
                  <span className="min-w-0 flex-1 truncate text-sm text-ink">
                    {hero.hero_name}
                  </span>
                  <span className="shrink-0 font-mono text-xs tabular-nums text-ink-faint">
                    {hero.matches} · {Math.round(hero.win_rate * 100)}%
                  </span>
                </li>
              ))}
            </ul>
          </Card>
        </section>
      ) : null}

      <p className="text-xs leading-relaxed text-ink-faint">
        Read from {session.analyzed_match_count} eligible{" "}
        {session.role_label} {session.analyzed_match_count === 1 ? "match" : "matches"}
        {" "}— Ranked and public All Pick only.
        {session.analysis_id
          ? " A coaching analysis was generated from this session."
          : ""}
      </p>
    </div>
  );
}

function TraitList({
  title,
  icon,
  tone,
  traits,
}: {
  title: string;
  icon: "check" | "alert";
  tone: string;
  traits: PlayerTrait[];
}) {
  return (
    <Card className="flex flex-col gap-3">
      <h2 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
        {title}
      </h2>

      {traits.length === 0 ? (
        <p className="text-sm text-ink-faint">Nothing stood out.</p>
      ) : (
        <ul className="flex flex-col gap-2.5">
          {traits.map((trait) => (
            <li key={trait.key} className="flex items-start gap-2.5">
              <Icon name={icon} className={`mt-0.5 size-4 shrink-0 ${tone}`} />
              <span className="text-[0.8125rem] leading-relaxed text-ink-muted">
                {trait.statement}
              </span>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

function BackLink() {
  return (
    <Link
      href="/coach/history"
      className="focus-neon inline-flex min-h-11 w-fit cursor-pointer items-center gap-1.5 rounded text-sm text-ink-muted transition-colors duration-200 ease-out hover:text-function"
    >
      <Icon name="chevron-left" className="size-5" />
      History
    </Link>
  );
}
