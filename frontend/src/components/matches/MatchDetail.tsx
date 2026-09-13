"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { Icon } from "@/components/ui/Icon";
import { SteamLoginButton } from "@/components/ui/SteamLoginButton";
import { TiltCard } from "@/components/ui/TiltCard";
import { ApiError, getMatch } from "@/lib/api";
import type { Match } from "@/lib/types";
import { cn, formatDuration } from "@/lib/utils";

type State =
  | { kind: "loading" }
  | { kind: "ready"; match: Match }
  | { kind: "anonymous" }
  | { kind: "error"; message: string };

/**
 * Phase 3 shows the stored facts only. Deterministic metrics and the AI report
 * arrive in Phase 4 and slot in below the performance grid.
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

  if (state.kind === "error") {
    return (
      <div className="flex flex-col gap-4">
        <BackLink />
        {/* A match owned by someone else is reported as not found, by design. */}
        <Alert title="Match unavailable">{state.message}</Alert>
      </div>
    );
  }

  const { match } = state;
  const won = match.won;

  return (
    <div className="flex flex-col gap-5 pb-8">
      <BackLink />

      {/* A phone reads this top to bottom. A desktop puts the identity card
          beside the numbers, so the result and the figures explaining it are
          in one glance rather than one scroll. */}
      <div className="grid gap-5 lg:grid-cols-3 lg:items-start">
        <TiltCard>
          <Card glow={won ? "string" : "error"} className="flex flex-col gap-4">
            <div className="flex items-center gap-3">
              <div className="pop-3d">
                <HeroPortrait
                  heroId={match.hero_id}
                  heroName={match.hero_name}
                  size="lg"
                />
              </div>

              {/* The name wraps rather than truncating: this page is *about*
                  this hero, and "Outworld Destroyer" clipped to "Outworld…"
                  loses the one thing the heading exists to say. That only
                  works because the result badge is not competing for the same
                  row — on a 360px screen the two left roughly 80px for the
                  name, and a single long word simply spilled over the badge. */}
              <div className="min-w-0 flex-1">
                <h1 className="font-display text-xl tracking-wide">
                  {match.hero_name}
                </h1>
                <p className="mt-0.5 text-sm text-ink-muted">
                  <span className="text-operator">{match.role}</span>
                  {" · "}
                  <span className="font-mono tabular-nums">
                    {formatDuration(match.duration_seconds)}
                  </span>
                </p>
              </div>
            </div>

            {/* Result sits with the score it explains. */}
            <div className="flex items-baseline justify-between gap-3 border-t border-glass-edge pt-4">
              <span className="flex min-w-0 items-baseline gap-3">
                <span className="font-mono text-3xl tabular-nums text-number">
                  {match.kills}/{match.deaths}/{match.assists}
                </span>
                <span className="text-sm text-ink-faint">
                  {match.kda?.toFixed(1) ?? "—"} KDA
                </span>
              </span>

              <span
                className={cn(
                  "shrink-0 rounded-lg border px-3 py-1.5 font-display text-sm tracking-wide",
                  won
                    ? "border-string/50 bg-mark-win/15 text-string"
                    : "border-error/50 bg-mark-loss/15 text-error",
                )}
              >
                {won ? "Win" : "Loss"}
              </span>
            </div>

            <p className="text-xs text-ink-faint">
              {new Date(match.started_at).toLocaleString()}
            </p>
          </Card>
        </TiltCard>

        <section className="flex min-w-0 flex-col gap-3 lg:col-span-2">
          <h2 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
            Performance
          </h2>

          <Card>
            {/* `lg`, not `sm`: the shell is phone-width until then, and three
                columns of numbers do not fit in it. */}
            <dl className="grid grid-cols-2 gap-x-4 gap-y-5 lg:grid-cols-3">
              <Stat label="GPM" value={match.gpm} />
              <Stat label="XPM" value={match.xpm} />
              <Stat label="Last hits" value={match.last_hits} />
              <Stat label="Denies" value={match.denies} />
              <Stat label="Net worth" value={match.net_worth} />
              <Stat label="Hero damage" value={match.hero_damage} />
              <Stat label="Tower damage" value={match.tower_damage} />
              <Stat label="Hero healing" value={match.hero_healing} />
              <Stat label="Match ID" value={match.match_id} small />
            </dl>
          </Card>

          {!match.detail_synced ? (
            <Alert tone="info">
              Only the summary was available for this match, so some figures are
              missing. A later sync will fill them in.
            </Alert>
          ) : null}
        </section>
      </div>

      <p className="text-xs leading-relaxed text-ink-faint">
        Role is an estimate: Dota does not publish positions, so it is derived
        from lane data when the replay was parsed and from farm priority
        otherwise.
      </p>
    </div>
  );
}

function Stat({
  label,
  value,
  small = false,
}: {
  label: string;
  value: number | null;
  small?: boolean;
}) {
  return (
    <div className="flex flex-col gap-1">
      <dt className="text-xs uppercase tracking-wider text-ink-faint">{label}</dt>
      <dd
        className={cn(
          "font-mono tabular-nums",
          small ? "text-sm text-ink-muted" : "text-lg text-number",
        )}
      >
        {value === null ? (
          <span className="text-ink-faint" title="Not available">
            —
          </span>
        ) : (
          value.toLocaleString()
        )}
      </dd>
    </div>
  );
}

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
