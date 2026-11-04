"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { Paywall } from "@/components/billing/Paywall";
import { ScoreRing } from "@/components/charts/ScoreRing";
import { MatchAnalysis } from "@/components/coach/MatchAnalysis";
import { ComparisonSkeleton, MatchComparison } from "@/components/matches/MatchComparison";
import { KpiStrip } from "@/components/matches/MatchKpis";
import { NetWorthTimeline } from "@/components/matches/NetWorthTimeline";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { InfoTip } from "@/components/ui/InfoTip";
import { Icon } from "@/components/ui/Icon";
import { SteamLoginButton } from "@/components/ui/SteamLoginButton";
import { ApiError, getMatch, getMatchComparison, setMatchRole } from "@/lib/api";
import type { CoachableRole, MatchComparisonResponse, MatchView } from "@/lib/types";
import { cn, formatDuration, formatMatchDate } from "@/lib/utils";

type State =
  | { kind: "loading" }
  | { kind: "ready"; match: MatchView }
  | { kind: "anonymous" }
  | { kind: "paywalled" }
  | { kind: "error"; message: string };

/**
 * The stored facts, then the coaching section.
 *
 * The analysis loads separately and never blocks the match itself: a coaching
 * outage must not stop a player reading their own game.
 */
export function MatchDetail({ id }: { id: string }) {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;

    getMatch(id)
      .then((response) => {
        if (!cancelled) setState({ kind: "ready", match: response.match });
      })
      .catch((error: unknown) => {
        if (cancelled) return;

        if (error instanceof ApiError && error.isUnauthenticated) {
          setState({ kind: "anonymous" });
          return;
        }
        if (error instanceof ApiError && error.isPaymentRequired) {
          setState({ kind: "paywalled" });
          return;
        }
        setState({
          kind: "error",
          message:
            error instanceof ApiError
              ? error.message
              : "Could not load this match.",
        });
      });

    return () => {
      cancelled = true;
    };
  }, [id]);

  if (state.kind === "loading") {
    return (
      <div className="flex flex-col gap-4" aria-busy="true">
        <span className="sr-only">Loading match…</span>
        <div className="glass h-40 animate-pulse rounded-card" />
        <div className="glass h-64 animate-pulse rounded-card" />
      </div>
    );
  }

  if (state.kind === "anonymous") {
    return (
      <div className="flex flex-col gap-4">
        <Alert tone="info">Sign in with Steam to view your matches.</Alert>
        <SteamLoginButton className="w-full" />
      </div>
    );
  }

  if (state.kind === "paywalled") {
    return (
      <div className="flex flex-col gap-4">
        <BackLink />
        <Paywall message="Match detail needs an active trial or subscription." />
      </div>
    );
  }

  if (state.kind === "error") {
    return (
      <div className="flex flex-col gap-4">
        <BackLink />
        {/* A match owned by someone else is reported as not found, by design. */}
        <Alert title="Match unavailable">{state.message}</Alert>
      </div>
    );
  }

  return (
    <Ready
      match={state.match}
      onChange={(next) => setState({ kind: "ready", match: next })}
    />
  );
}

/** The page once the match is in hand. */
function Ready({
  match,
  onChange,
}: {
  match: MatchView;
  onChange: (next: MatchView) => void;
}) {
  const comparison = useComparison(match.id);
  const data = comparison.kind === "ready" ? comparison.data : null;
  const won = match.won;

  return (
    <div className="flex flex-col gap-5 pb-8">
      <BackLink />

      <Card glow={won ? "string" : "error"} className="flex flex-col gap-4 sm:flex-row sm:items-center">
        <div className="flex min-w-0 flex-1 flex-col gap-3">
          <div className="flex items-center gap-3">
            <HeroPortrait heroId={match.hero_id} heroName={match.hero_name} size="lg" />
            {/* The name wraps rather than truncating: this page is about this
                hero, and a clipped name loses the one thing the heading says. */}
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <h1 className="font-display text-xl tracking-wide">{match.hero_name}</h1>
                <span
                  className={cn(
                    "shrink-0 rounded-full border px-2.5 py-0.5 font-display text-xs tracking-wide",
                    won
                      ? "border-string/50 bg-mark-win/15 text-string"
                      : "border-error/50 bg-mark-loss/15 text-error",
                  )}
                >
                  {won ? "Win" : "Loss"}
                </span>
              </div>
              <p className="mt-1 flex flex-wrap items-baseline gap-x-3 gap-y-1">
                <span className="font-mono text-2xl tabular-nums text-number">
                  {match.kills}/{match.deaths}/{match.assists}
                </span>
                <span className="text-sm text-ink-faint">{match.kda?.toFixed(1) ?? "—"} KDA</span>
                <span className="font-mono text-sm tabular-nums text-ink-muted">
                  {formatDuration(match.duration_seconds)}
                </span>
              </p>
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-x-3 gap-y-2 text-sm text-ink-muted">
            <RoleControl match={match} onChange={onChange} />
          </div>

          <p className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-ink-faint">
            <time dateTime={match.started_at}>{formatMatchDate(match.started_at)}</time>
            <span className="flex items-center gap-2 font-mono">
              {/* An id, not a quantity: never grouped with separators. */}
              <span>#{String(match.match_id)}</span>
              <a
                href={`https://www.opendota.com/matches/${match.match_id}`}
                target="_blank"
                rel="noreferrer"
                className="text-function hover:text-ink"
              >
                OpenDota
              </a>
              <a
                href={`https://stratz.com/matches/${match.match_id}`}
                target="_blank"
                rel="noreferrer"
                className="text-function hover:text-ink"
              >
                STRATZ
              </a>
            </span>
          </p>
        </div>

        <GameScore comparison={comparison} />
      </Card>

      {!match.detail_synced ? (
        <Alert tone="info">
          Only the summary was available for this match, so some figures are
          missing. A later sync will fill them in.
        </Alert>
      ) : null}

      <KpiStrip match={match} comparison={data} />

      {comparison.kind === "loading" ? <ComparisonSkeleton /> : null}
      {comparison.kind === "error" ? <Alert>{comparison.message}</Alert> : null}
      {data ? <MatchComparison
          data={data}
          durationSeconds={match.duration_seconds}
          positionLabel={match.position ? match.position_label : null}
        /> : null}

      <NetWorthTimeline id={match.id} />

      <MatchAnalysis id={match.id} />
    </div>
  );
}

type ComparisonState =
  | { kind: "loading" }
  | { kind: "ready"; data: MatchComparisonResponse }
  | { kind: "error"; message: string };

/**
 * One request for the comparison, shared by the header score, the KPI badges
 * and the charts, so no two of them can show different numbers.
 */
function useComparison(id: string): ComparisonState {
  const [state, setState] = useState<ComparisonState>({ kind: "loading" });
  useEffect(() => {
    let cancelled = false;
    getMatchComparison(id)
      .then((data) => {
        if (!cancelled) setState({ kind: "ready", data });
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setState({
          kind: "error",
          message: e instanceof ApiError ? e.message : "Could not load this comparison.",
        });
      });
    return () => {
      cancelled = true;
    };
  }, [id]);
  return state;
}

/** The game's median percentile against the bracket, with your average marked. */
function GameScore({ comparison }: { comparison: ComparisonState }) {
  if (comparison.kind === "loading") {
    return <div aria-hidden className="size-[104px] shrink-0 animate-pulse self-center rounded-full bg-surface-2" />;
  }
  if (comparison.kind !== "ready" || comparison.data.standing.this_match === null) return null;
  const { standing, bracket } = comparison.data;
  return (
    <div className="flex items-start gap-1 self-center">
      <ScoreRing
        score={standing.this_match!}
        marker={standing.hero_average}
        markerLabel="your avg"
        label={`Game score against ${bracket.label} players`}
        caption={`vs ${bracket.label}`}
      />
      <InfoTip label="About the game score">
        The median of this game&apos;s {standing.metrics_counted} metric
        percentiles among {bracket.label} players on this hero. The hollow dot
        is the same reading for your average, excluding this game.
      </InfoTip>
    </div>
  );
}

const SOURCE_TAG: Record<MatchView["position_source"], string> = {
  user: "set by you",
  lane: "from lane data",
  estimated: "estimated",
  unknown: "estimated",
};

/**
 * The position, how sure it is, and a way to correct it. A correction is
 * stored on the match and used by every page that reads positions.
 */
function RoleControl({
  match,
  onChange,
}: {
  match: MatchView;
  onChange: (next: MatchView) => void;
}) {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const value = match.position_source === "user" ? (match.position ?? "auto") : "auto";

  return (
    <span className="inline-flex flex-wrap items-center gap-1.5">
      <span className="text-operator">{match.position_label}</span>
      <span className="whitespace-nowrap rounded bg-surface-2 px-1.5 text-[0.625rem] uppercase tracking-wider text-ink-faint">
        {SOURCE_TAG[match.position_source]}
      </span>
      <select
        aria-label="Correct the position"
        value={value}
        disabled={saving}
        onChange={async (e) => {
          setSaving(true);
          setError(null);
          try {
            const response = await setMatchRole(
              match.id,
              e.target.value as CoachableRole | "auto",
            );
            onChange(response.match);
          } catch (err) {
            setError(err instanceof ApiError ? err.message : "Could not save the position.");
          } finally {
            setSaving(false);
          }
        }}
        className="focus-neon min-h-7 cursor-pointer rounded-md border border-glass-edge bg-surface-2/60 px-1.5 text-[0.6875rem] text-ink outline-none"
      >
        <option value="auto">Auto (estimate)</option>
        {POSITIONS.map((p) => (
          <option key={p.value} value={p.value}>
            {p.label}
          </option>
        ))}
      </select>
      <InfoTip label="How the position is decided">
        Dota does not publish positions. It is read from lane data when the replay
        was parsed, from farm priority otherwise. Correct it here and every page
        uses your choice.
      </InfoTip>
      {error ? <span className="text-xs text-error">{error}</span> : null}
    </span>
  );
}

const POSITIONS: { value: CoachableRole; label: string }[] = [
  { value: "carry", label: "Carry" },
  { value: "mid", label: "Mid" },
  { value: "offlane", label: "Offlane" },
  { value: "soft_support", label: "Soft Support" },
  { value: "hard_support", label: "Hard Support" },
];

function BackLink() {
  return (
    <Link
      href="/matches"
      className="focus-neon inline-flex min-h-11 w-fit cursor-pointer items-center gap-1.5 rounded text-sm text-ink-muted transition-colors duration-200 ease-out hover:text-function"
    >
      <Icon name="chevron-left" className="size-5" />
      Matches
    </Link>
  );
}
