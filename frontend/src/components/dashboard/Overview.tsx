"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { BarList } from "@/components/charts/BarList";
import { FormStrip } from "@/components/charts/FormStrip";
import { Meter } from "@/components/charts/Meter";
import { Sparkline } from "@/components/charts/Sparkline";
import { TrainingFocusCard } from "@/components/coach/TrainingFocusCard";
import { StatTile } from "@/components/dashboard/StatTile";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { Icon } from "@/components/ui/Icon";
import { StreamingText } from "@/components/ui/StreamingText";
import { ApiError, getMatches, getStats, syncMatches } from "@/lib/api";
import {
  formatFixed,
  formatPercent,
  formatWhole,
  kdaSeries,
  recentForm,
} from "@/lib/stats";
import type { Match, StatsResponse, SyncReport } from "@/lib/types";
import { useSession } from "@/lib/session-context";
import { cn } from "@/lib/utils";

/** Enough recent matches for the trend line and form strip. Aggregates come
 *  from the backend, so this is a display sample, not a statistical one. */
const SAMPLE = 20;

export function Overview({ loginError }: { loginError?: string }) {
  const { session, setSession } = useSession();
  const [matches, setMatches] = useState<Match[] | null>(null);
  const [stats, setStats] = useState<StatsResponse | null>(null);
  const [matchesError, setMatchesError] = useState<string | null>(null);

  const [syncing, setSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [lastSync, setLastSync] = useState<SyncReport | null>(null);

  const load = useCallback(async () => {
    try {
      // Aggregates and the recent list in parallel: the numbers come from the
      // backend, the list only feeds the trend and form visuals.
      const [page, computed] = await Promise.all([
        getMatches(1, SAMPLE),
        getStats(),
      ]);
      setMatches(page.matches);
      setStats(computed);
      setMatchesError(null);
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
    return <Alert title="Cannot reach the service">{session.message}</Alert>;
  }

  const { me } = session;
  const list = matches ?? [];
  const overall = stats?.overall;
  const hasMatches = (overall?.matches ?? 0) > 0;

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
            {hasMatches && overall
              ? `Reading ${overall.matches} ${overall.matches === 1 ? "match" : "matches"}.`
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
      {hasMatches ? <TrainingFocusCard compact /> : null}

      {!hasMatches && !matchesError ? (
        <Card>
          <p className="text-sm leading-relaxed text-ink-muted">
            No matches stored yet. Tap{" "}
            <strong className="font-semibold text-keyword">Sync Matches</strong>{" "}
            to pull your recent games from OpenDota.
          </p>
        </Card>
      ) : null}

      {hasMatches && overall && stats ? (
        <>
          {/* Four headline numbers: two-up on a phone, one row on a desktop,
              which is where the eye compares them fastest. */}
          <section className="grid grid-cols-2 gap-3 lg:grid-cols-4 lg:gap-4">
            <StatTile
              label="Win rate"
              value={formatPercent(overall.win_rate)}
              icon="trophy"
              tone="string"
            />
            <StatTile
              label="Avg KDA"
              value={formatFixed(overall.avg_kda)}
              icon="spark"
            />
            <StatTile
              label="Avg GPM"
              value={formatWhole(overall.avg_gpm)}
              icon="coins"
            />
            <StatTile
              label="Deaths / 10min"
              value={formatFixed(overall.avg_deaths_per_10)}
              icon="skull"
              tone="error"
            />
          </section>

          {/* Form and trend read together: on a wide screen they sit side by
              side, the trend taking the extra width because a line needs it.

              Every card in these grids carries `min-w-0`. A grid item defaults
              to `min-width: auto`, which means "never shrink below your own
              content" — so one wide child (the hero strip) would push the
              whole grid past the viewport instead of scrolling inside itself.
              Column flex, which these cards used to sit in, has no such rule,
              which is why the overflow only appeared once they were gridded. */}
          <div className="grid gap-4 lg:grid-cols-3">
            <Card className="flex min-w-0 flex-col gap-5">
              <Meter
                value={overall.win_rate ?? 0}
                label="Win rate"
                valueText={`${overall.wins}W · ${overall.losses}L`}
              />

              <div className="flex flex-col gap-2">
                <h2 className="text-xs uppercase tracking-wider text-ink-faint">
                  Recent form
                </h2>
                <FormStrip results={recentForm(list, 12)} />
              </div>
            </Card>

            <Card className="flex min-w-0 flex-col gap-2 lg:col-span-2">
              <div className="flex items-baseline justify-between gap-3">
                <h2 className="text-xs uppercase tracking-wider text-ink-faint">
                  KDA trend
                </h2>
                <span className="text-[0.6875rem] text-ink-faint">
                  last {Math.min(list.length, 20)} matches
                </span>
              </div>
              <Sparkline values={kdaSeries(list, 20)} label="KDA per match" />
            </Card>
          </div>

          <div className="grid gap-4 lg:grid-cols-2">
            <Card className="flex min-w-0 flex-col gap-4">
              <h2 className="text-xs uppercase tracking-wider text-ink-faint">
                Roles played
              </h2>
              <BarList
                caption="Matches played per role"
                data={stats.roles.map((r) => ({
                  label: r.role,
                  value: r.matches,
                  meta: formatPercent(r.win_rate),
                }))}
              />
            </Card>

            <Card className="flex min-w-0 flex-col gap-4">
              <div className="flex items-baseline justify-between gap-3">
                <h2 className="text-xs uppercase tracking-wider text-ink-faint">
                  Most played
                </h2>
                <Link
                  href="/matches"
                  className="focus-neon cursor-pointer rounded text-xs text-function transition-colors duration-200 ease-out hover:text-ink"
                >
                  All matches
                </Link>
              </div>

              {/* A phone swipes through these; a desktop has room to lay all
                  five out at once, so it does rather than hiding four of them
                  behind a gesture that has no pointer equivalent.
                  `lg`, not `sm`: the column itself only widens at `lg`, and a
                  portrait is a fixed 7rem — five of them overflow anything
                  narrower. */}
              <ul className="-mx-1 flex gap-2 overflow-x-auto px-1 pb-1 lg:grid lg:grid-cols-5 lg:overflow-visible">
                {stats.heroes.slice(0, 5).map((hero) => (
                  <li
                    key={hero.hero_id}
                    className="flex shrink-0 flex-col gap-1.5 lg:min-w-0 lg:shrink"
                  >
                    <HeroPortrait
                      heroId={hero.hero_id}
                      heroName={hero.hero_name}
                      size="lg"
                    />
                    <span className="max-w-28 truncate text-[0.6875rem] text-ink-muted lg:max-w-full">
                      {hero.hero_name}
                    </span>
                    <span className="truncate font-mono text-[0.6875rem] tabular-nums text-ink-faint">
                      {hero.matches}× · {formatPercent(hero.win_rate)}
                    </span>
                  </li>
                ))}
              </ul>
            </Card>
          </div>

          {overall.parsed_matches < overall.matches ? (
            <p className="text-xs leading-relaxed text-ink-faint">
              {overall.parsed_matches} of {overall.matches} matches have a
              parsed replay. Timing metrics such as last hits at 10 minutes are
              only available for those.
            </p>
          ) : null}
        </>
      ) : null}
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
