"use client";

import Link from "next/link";

import { GHOST_LINK } from "@/components/profile/AccountCards";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { Icon } from "@/components/ui/Icon";
import { oldestFirst, topHeroes } from "@/lib/profile";
import { formatPercent } from "@/lib/stats";
import type { MatchView, StatsResponse } from "@/lib/types";
import { cn, formatMatchDate } from "@/lib/utils";

/** Shared look for every linked tile in the snapshot. */
const TILE =
  "focus-neon group flex min-w-0 flex-col gap-2 rounded-xl border border-glass-edge bg-surface-2/40 p-4 transition-colors duration-200 hover:border-function/60";

/**
 * Who this player is at a glance: the four facts the rest of the app spends a
 * page each on, each one a door into that page.
 *
 * Every figure is the backend's. `trends.win_rate` is already 0–100 over the
 * last window; `overall.win_rate` is a 0–1 share over every eligible match.
 */
export function PlayerSnapshot({
  stats,
  recent,
  coachingRoleLabel,
  error,
  onSync,
  syncing,
}: {
  stats: StatsResponse | null;
  /** Newest first, as the match list serves it. */
  recent: MatchView[] | null;
  /** The role the player chose to be coached on, when they have. */
  coachingRoleLabel: string | null;
  error: string | null;
  onSync: () => void;
  syncing: boolean;
}) {
  return (
    <section aria-labelledby="snapshot-heading" className="flex flex-col gap-3">
      <h2 id="snapshot-heading" className="font-display text-sm tracking-wide text-ink-muted">
        Player snapshot
      </h2>
      <Body
        stats={stats}
        recent={recent}
        coachingRoleLabel={coachingRoleLabel}
        error={error}
        onSync={onSync}
        syncing={syncing}
      />
    </section>
  );
}

function Body({
  stats,
  recent,
  coachingRoleLabel,
  error,
  onSync,
  syncing,
}: Parameters<typeof PlayerSnapshot>[0]) {
  if (error) {
    return (
      <Card>
        <p className="text-sm text-ink-muted">{error}</p>
      </Card>
    );
  }

  if (!stats || !recent) return <SnapshotSkeleton />;

  const stored = stats.eligibility.total_matches;

  // Stored games, none of them eligible. Syncing more of the same never fixes
  // this, so the answer is to say why rather than to offer a sync.
  if (stats.overall.matches === 0 && stored > 0) {
    const reasons = stats.eligibility.excluded.map((e) => e.label).join(", ");
    return (
      <Card className="flex flex-col items-start gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <p className="text-sm text-ink">Nothing to analyse yet</p>
          <p className="text-xs text-ink-faint">
            None of your {stored} stored {stored === 1 ? "match is" : "matches are"} Ranked or
            public All Pick{reasons ? ` (${reasons})` : ""}. Play a few and sync again.
          </p>
        </div>
        <Link href="/matches?mode=all" className={cn(GHOST_LINK, "w-full sm:w-auto")}>
          See stored matches
        </Link>
      </Card>
    );
  }

  if (stats.overall.matches === 0) {
    return (
      <Card className="flex flex-col items-start gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <p className="text-sm text-ink">No Ranked or All Pick matches yet</p>
          <p className="text-xs text-ink-faint">
            Sync to pull your recent games and build your snapshot.
          </p>
        </div>
        <Button onClick={onSync} disabled={syncing} className="w-full sm:w-auto">
          <Icon name="refresh" className={cn("size-5", syncing && "animate-spin")} />
          {syncing ? "Syncing matches…" : "Sync matches"}
        </Button>
      </Card>
    );
  }

  const { overall, trends } = stats;
  const mostPlayed = [...stats.role_analysis.roles].sort((a, b) => b.matches - a.matches)[0];
  const heroes = topHeroes(stats.heroes);
  const form = oldestFirst(recent.slice(0, 10));

  return (
    <Card className="grid grid-cols-1 gap-3 sm:grid-cols-2">
      <Link href="/matches" className={TILE}>
        <span className="text-xs text-ink-faint">Win rate</span>
        <span className="flex items-baseline gap-3">
          <span className="font-mono text-2xl tabular-nums text-number">
            {trends.win_rate.value === null ? "—" : `${Math.round(trends.win_rate.value)}%`}
          </span>
          {/* A window wider than the history would overstate the sample. */}
          <span className="text-xs text-ink-faint">
            last {Math.min(trends.window, overall.matches)}
          </span>
        </span>
        <span className="text-xs text-ink-muted">
          {formatPercent(overall.win_rate)} across all {overall.matches} eligible matches
        </span>
      </Link>

      <Link href="/coach" className={TILE}>
        <span className="text-xs text-ink-faint">
          {coachingRoleLabel ? "Coaching role" : "Main role"}
        </span>
        <span className="font-display text-lg tracking-wide text-ink">
          {coachingRoleLabel ?? mostPlayed?.role_label ?? "Not enough games"}
        </span>
        <span className="text-xs text-ink-muted">
          {mostPlayed
            ? `Most played: ${mostPlayed.role_label} · ${mostPlayed.matches} games`
            : "Roles appear once matches can be attributed to one."}
        </span>
      </Link>

      <Link href="/heroes" className={cn(TILE, "sm:col-span-2")}>
        <span className="text-xs text-ink-faint">Top heroes</span>
        {heroes.length > 0 ? (
          <ul className="grid grid-cols-1 gap-2 sm:grid-cols-3">
            {heroes.map((hero) => (
              <li key={hero.hero_id} className="flex min-w-0 items-center gap-3">
                <HeroPortrait heroId={hero.hero_id} heroName={hero.hero_name} size="sm" />
                <span className="min-w-0">
                  <span className="block truncate text-sm text-ink">{hero.hero_name}</span>
                  <span className="block font-mono text-xs tabular-nums text-ink-muted">
                    {hero.matches} {hero.matches === 1 ? "game" : "games"} ·{" "}
                    {formatPercent(hero.win_rate)} WR
                  </span>
                </span>
              </li>
            ))}
          </ul>
        ) : (
          <span className="text-sm text-ink-muted">No heroes yet.</span>
        )}
      </Link>

      <div className="flex min-w-0 flex-col gap-2 rounded-xl border border-glass-edge bg-surface-2/40 p-4 sm:col-span-2">
        <span className="flex items-center justify-between text-xs text-ink-faint">
          <span>Recent form · last {form.length}</span>
          <span>oldest → newest</span>
        </span>
        <ol className="flex flex-wrap gap-1.5" aria-label="Last matches, oldest first">
          {form.map((match) => (
            <li key={match.id}>
              <Link
                href={`/matches/${match.id}`}
                aria-label={`${match.won ? "Win" : "Loss"} on ${match.hero_name}, ${formatMatchDate(match.started_at)}`}
                title={`${match.won ? "Win" : "Loss"} · ${match.hero_name}`}
                className={cn(
                  "focus-neon flex size-8 items-center justify-center rounded-full text-[0.6875rem] font-semibold transition-transform duration-200 hover:scale-110",
                  match.won ? "bg-mark-win/15 text-string" : "bg-mark-loss/15 text-error",
                )}
              >
                {match.won ? "W" : "L"}
              </Link>
            </li>
          ))}
        </ol>
      </div>
    </Card>
  );
}

function SnapshotSkeleton() {
  return (
    <div aria-busy="true" className="glass grid grid-cols-1 gap-3 rounded-card p-5 sm:grid-cols-2 sm:p-6">
      <span className="sr-only">Loading your snapshot…</span>
      <div className="h-24 animate-pulse rounded-xl bg-surface-2/60" />
      <div className="h-24 animate-pulse rounded-xl bg-surface-2/60" />
      <div className="h-20 animate-pulse rounded-xl bg-surface-2/60 sm:col-span-2" />
      <div className="h-16 animate-pulse rounded-xl bg-surface-2/60 sm:col-span-2" />
    </div>
  );
}
