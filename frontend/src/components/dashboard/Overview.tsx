"use client";

import { useCallback, useEffect, useState } from "react";

import { CoachRecommendations } from "@/components/dashboard/CoachRecommendations";
import { LastGamesStrip, PerformanceTrend, SkillRadar } from "@/components/dashboard/OverviewCharts";
import { OverviewKpis } from "@/components/dashboard/OverviewKpis";
import { RecentMatches } from "@/components/dashboard/RecentMatches";
import { RolePerformance } from "@/components/dashboard/RolePerformance";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { InfoTip } from "@/components/ui/InfoTip";
import { StreamingText } from "@/components/ui/StreamingText";
import { ScopeLine } from "@/components/ui/ScopeLine";
import { ApiError, getBenchmark, getCalibration, getMatches, getStats, syncMatches } from "@/lib/api";
import type {
  BenchmarkResponse,
  CalibrationResponse,
  MatchView,
  StatsResponse,
  SyncReport,
} from "@/lib/types";
import { useSession } from "@/lib/session-context";
import { cn } from "@/lib/utils";

/** Enough recent matches for the trend line and form strip. Aggregates come
 *  from the backend, so this is a display sample, not a statistical one.
 *
 *  Drawn from the **competitive** scope: the strip and the sparkline describe
 *  the same games as the numbers beside them, so a Turbo win can never appear
 *  as form on a screen whose win rate excludes it. */
const SAMPLE = 20;

export function Overview({ loginError }: { loginError?: string }) {
  const { session, setSession } = useSession();
  const [matches, setMatches] = useState<MatchView[] | null>(null);
  const [stats, setStats] = useState<StatsResponse | null>(null);
  /** Optional panels: each is null when its endpoint is unavailable (plan,
   *  provider, no sync yet) and its card simply is not drawn. */
  const [calibration, setCalibration] = useState<CalibrationResponse | null>(null);
  const [benchmark, setBenchmark] = useState<BenchmarkResponse | null>(null);
  const [matchesError, setMatchesError] = useState<string | null>(null);

  const [syncing, setSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [lastSync, setLastSync] = useState<SyncReport | null>(null);

  const load = useCallback(async () => {
    try {
      // Aggregates and the recent list in parallel: the numbers come from the
      // backend, the list only feeds the trend and form visuals — and both
      // read the competitive population, so they describe the same games.
      const [page, computed] = await Promise.all([
        getMatches(1, SAMPLE),
        getStats(),
      ]);
      setMatches(page.matches);
      setStats(computed);
      setMatchesError(null);
      // After the essentials, and never able to fail the page.
      getCalibration().then(setCalibration, () => setCalibration(null));
      getBenchmark(undefined, "all").then(setBenchmark, () => setBenchmark(null));
    } catch (error) {
      setMatchesError(
        error instanceof ApiError
          ? error.message
          : "Could not load your match history.",
      );
    }
  }, []);

  useEffect(() => {
    if (session.kind === "signed-in") void load();
  }, [session.kind, load]);

  async function handleSync() {
    setSyncing(true);
    setSyncError(null);
    setLastSync(null);

    try {
      const result = await syncMatches();
      setLastSync(result.sync);

      setSession((current) =>
        current.kind === "signed-in"
          ? {
              kind: "signed-in",
              me: { ...current.me, dota_player: result.dota_player },
            }
          : current,
      );
      await load();
    } catch (error) {
      setSyncError(
        error instanceof ApiError
          ? error.message
          : "Could not sync your matches right now.",
      );
    } finally {
      setSyncing(false);
    }
  }

  if (session.kind === "loading") return <OverviewSkeleton />;
  if (session.kind === "anonymous") return <SignedOut loginError={loginError} />;
  if (session.kind === "error") {
    // The shell shows the error with a retry.
    return null;
  }

  const { me } = session;
  const list = matches ?? [];
  const overall = stats?.overall;
  /** Eligible matches — the population every number on this screen describes. */
  const hasMatches = (overall?.matches ?? 0) > 0;
  /** Every stored match, whatever mode. A player can have plenty of these and
   *  no eligible ones at all, which is a different problem with a different
   *  answer, so the two empty states are kept apart. */
  const stored = stats?.eligibility.total_matches ?? 0;

  return (
    <div className="flex flex-col gap-6 pb-4">
      {/* Greeting and the one action this screen owns. Stacked under the thumb
          on a phone; a single header row once there is width for it. */}
      <section className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between lg:gap-6">
        <div className="flex flex-col gap-2">
          {/* `lg:text-base` would be a colour, not a size — see Button.tsx. */}
          <p className="text-sm text-ink-muted lg:text-[1rem]">
            <StreamingText
              text={`Welcome back, ${me.user.persona_name ?? "player"}.`}
            />
          </p>
          <p className="text-xs text-ink-faint">
            {hasMatches && stats
              ? "Your performance in Ranked and public All Pick."
              : "Sync your matches to see your numbers."}
          </p>
        </div>

        <Button
          onClick={handleSync}
          disabled={syncing}
          className="w-full lg:w-auto lg:shrink-0"
        >
          <Icon
            name="refresh"
            className={cn("size-5", syncing && "animate-spin")}
          />
          {syncing ? "Syncing matches…" : "Sync Matches"}
        </Button>
      </section>

      <div aria-live="polite" className="flex flex-col gap-3 empty:hidden">
        {syncError ? <Alert title="Sync failed">{syncError}</Alert> : null}
        {lastSync ? (
          <Alert tone="success" title="Sync complete">
            {lastSync.new_matches} new{" "}
            {lastSync.new_matches === 1 ? "match" : "matches"},{" "}
            {lastSync.duplicates_skipped} already stored.
          </Alert>
        ) : null}
      </div>

      {matchesError ? <Alert>{matchesError}</Alert> : null}

      {/* The dashboard's first question is "what should I work on", not "what
          are my numbers". Statistics follow underneath. */}
      {hasMatches ? <CoachRecommendations /> : null}

      {stats && stored > 0 ? <Scope stats={stats} /> : null}

      {stored === 0 && !matchesError ? (
        <Card>
          <p className="text-sm text-ink-muted">
            No matches stored yet. Tap{" "}
            <strong className="font-semibold text-keyword">Sync Matches</strong>{" "}
            to pull your recent games.
          </p>
        </Card>
      ) : null}

      {/* Stored games, none of them eligible. Never solved by syncing more of
          the same, so it does not say "sync". */}
      {stored > 0 && !hasMatches && !matchesError ? (
        <Card>
          <p className="text-sm text-ink-muted">
            None of your {stored} stored {stored === 1 ? "match is" : "matches are"} Ranked
            or public All Pick, so there is nothing to analyse yet.
          </p>
        </Card>
      ) : null}

      {hasMatches && stats ? (
        <>
          <OverviewKpis trends={stats.trends} calibration={calibration} />

          {/* `min-w-0` on every grid child: a grid item defaults to
              `min-width: auto` and one wide chart would push the page sideways. */}
          <div className="grid gap-4 lg:grid-cols-3">
            {benchmark ? <SkillRadar benchmark={benchmark} /> : null}
            <PerformanceTrend matches={list} trends={stats.trends} />
          </div>

          {list.length > 0 ? <LastGamesStrip matches={list} /> : null}

          <div className="grid gap-4 lg:grid-cols-2">
            {list.length > 0 ? <RecentMatches matches={list} /> : null}
            <RolePerformance analysis={stats.role_analysis} />
          </div>
        </>
      ) : null}
    </div>
  );
}

/** What every number here is about, in one line; the detail sits in the (i). */
function Scope({ stats }: { stats: StatsResponse }) {
  const { scope, eligibility } = stats;
  const excluded = eligibility.excluded.filter((g) => g.matches > 0);
  return (
    <div className="flex items-center gap-1.5">
      <ScopeLine matches={scope.analyzed_matches} />
      <InfoTip label="About these matches">
        <span className="flex flex-col gap-1.5">
          <span>
            {scope.confidence_label} sample. {scope.confidence_caveat}
          </span>
          {excluded.map((g) => (
            <span key={g.reason}>
              {g.matches} × {g.label} excluded
            </span>
          ))}
        </span>
      </InfoTip>
    </div>
  );
}

function OverviewSkeleton() {
  return (
    <div className="flex flex-col gap-6" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading your overview…</span>
      <div className="h-10 w-2/3 animate-pulse rounded-lg bg-surface-2" />
      <div className="h-11 animate-pulse rounded-xl bg-surface-2 lg:w-48" />
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4 lg:gap-4">
        {[0, 1, 2, 3].map((i) => (
          <div key={i} className="glass h-20 animate-pulse rounded-card" />
        ))}
      </div>
      <div className="grid gap-4 lg:grid-cols-3">
        <div className="glass h-40 animate-pulse rounded-card" />
        <div className="glass hidden h-40 animate-pulse rounded-card lg:col-span-2 lg:block" />
      </div>
    </div>
  );
}
