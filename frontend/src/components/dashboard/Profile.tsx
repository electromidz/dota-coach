"use client";

import { useCallback, useEffect, useState } from "react";

import { ProfileCard } from "@/components/dashboard/ProfileCard";
import { MatchesStoredCard, SubscriptionCard } from "@/components/profile/AccountCards";
import { DataAccountCard } from "@/components/profile/DataAccountCard";
import { PlayerSnapshot } from "@/components/profile/PlayerSnapshot";
import { PreferencesCard } from "@/components/profile/PreferencesCard";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import {
  ApiError,
  getBilling,
  getMatches,
  getMe,
  getRoleSelection,
  getStats,
  syncMatches,
} from "@/lib/api";
import { useSession } from "@/lib/session-context";
import type {
  BillingResponse,
  MatchView,
  RoleSelectionResponse,
  StatsResponse,
  SyncReport,
} from "@/lib/types";

function message(error: unknown, fallback: string): string {
  return error instanceof ApiError ? error.message : fallback;
}

/**
 * The account page: who you are, how you play, what is stored, and what you
 * can do about it.
 *
 * Every panel loads on its own and fails on its own — billing being down must
 * not hide the match snapshot, and the reverse. All of these endpoints are
 * free-tier reads, so an expired trial still sees the whole page.
 */
export function Profile() {
  const { session, setSession } = useSession();

  const [stats, setStats] = useState<StatsResponse | null>(null);
  const [recent, setRecent] = useState<MatchView[] | null>(null);
  const [snapshotError, setSnapshotError] = useState<string | null>(null);
  /** `undefined` while loading, `null` when nothing is stored. */
  const [range, setRange] = useState<{ oldest: string; newest: string } | null | undefined>(
    undefined,
  );
  const [billing, setBilling] = useState<BillingResponse | null>(null);
  const [billingError, setBillingError] = useState<string | null>(null);
  const [roles, setRoles] = useState<RoleSelectionResponse | null>(null);

  const [syncing, setSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [lastSync, setLastSync] = useState<SyncReport | null>(null);

  const loadMatchData = useCallback(() => {
    // The snapshot reads the competitive population, like the Overview; the
    // stored range covers every mode, like the stored count beside it.
    Promise.all([getStats(), getMatches(1, 10)])
      .then(([computed, page]) => {
        setStats(computed);
        setRecent(page.matches);
        setSnapshotError(null);
      })
      .catch((e) => setSnapshotError(message(e, "Could not load your snapshot.")));

    Promise.all([
      getMatches(1, 1, { mode: "all" }),
      getMatches(1, 1, { mode: "all", sort: "oldest" }),
    ])
      .then(([newest, oldest]) => {
        const last = newest.matches[0];
        const first = oldest.matches[0];
        setRange(last && first ? { oldest: first.started_at, newest: last.started_at } : null);
      })
      .catch(() => setRange(null));
  }, []);

  useEffect(() => {
    if (session.kind !== "signed-in") return;

    loadMatchData();
    getBilling()
      .then(setBilling)
      .catch((e) => setBillingError(message(e, "Could not load your subscription.")));
    getRoleSelection()
      .then(setRoles)
      .catch(() => setRoles(null));
  }, [session.kind, loadMatchData]);

  async function handleSync() {
    setSyncing(true);
    setSyncError(null);
    setLastSync(null);

    try {
      const result = await syncMatches();
      setLastSync(result.sync);
      // The stored count lives on `me`; re-read it rather than adding to it.
      const me = await getMe().catch(() => null);
      setSession((current) =>
        current.kind === "signed-in"
          ? {
              kind: "signed-in",
              me: me ?? { ...current.me, dota_player: result.dota_player },
            }
          : current,
      );
      loadMatchData();
    } catch (e) {
      setSyncError(message(e, "Could not sync your matches right now."));
    } finally {
      setSyncing(false);
    }
  }

  if (session.kind === "loading") return <ProfileSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;
  if (session.kind === "error") {
    // The shell shows the error with a retry.
    return null;
  }

  const { me } = session;

  return (
    <div className="flex flex-col gap-5 pb-4 lg:grid lg:grid-cols-3 lg:items-start lg:gap-6">
      {/* `contents` on a phone lets these children interleave with the side
          column by `order`, so the account cards land before "Data & account";
          from `lg` they are one real column again. */}
      <div className="contents lg:col-span-2 lg:flex lg:min-w-0 lg:flex-col lg:gap-6">
        <div className="order-1 flex min-w-0 flex-col gap-3 lg:order-none">
          <ProfileCard
            user={me.user}
            dotaPlayer={me.dota_player}
            onSync={handleSync}
            syncing={syncing}
          />
          <div aria-live="polite" className="flex flex-col gap-3 empty:hidden">
            {syncError ? <Alert title="Sync failed">{syncError}</Alert> : null}
            {lastSync ? (
              <Alert tone="success" title="Sync complete">
                {lastSync.new_matches} new {lastSync.new_matches === 1 ? "match" : "matches"},{" "}
                {lastSync.duplicates_skipped} already stored.
              </Alert>
            ) : null}
          </div>
        </div>

        <div className="order-2 min-w-0 lg:order-none">
          <PlayerSnapshot
            stats={stats}
            recent={recent}
            coachingRoleLabel={roles?.profile?.selected_role_label ?? null}
            error={snapshotError}
            onSync={handleSync}
            syncing={syncing}
          />
        </div>

        <div className="order-4 min-w-0 lg:order-none">
          <DataAccountCard accountId={me.dota_player.dota_account_id} />
        </div>
      </div>

      <aside
        aria-label="Account"
        className="order-3 flex min-w-0 flex-col gap-5 lg:order-none lg:gap-4"
      >
        <MatchesStoredCard count={me.matches_stored} range={range} />
        <SubscriptionCard billing={billing} error={billingError} />
        <PreferencesCard roles={roles} onRolesChange={setRoles} />
      </aside>

      <p className="order-5 text-xs leading-relaxed text-ink-faint lg:order-none lg:col-span-3">
        Roles and scores shown in this app are estimates derived from public
        match data. They are not an official rating and do not promise MMR
        gains.
      </p>
    </div>
  );
}

function ProfileSkeleton() {
  return (
    <div
      className="flex flex-col gap-5 lg:grid lg:grid-cols-3 lg:items-start lg:gap-6"
      aria-busy="true"
    >
      <span className="sr-only">Loading profile…</span>
      <div className="flex flex-col gap-5 lg:col-span-2">
        <div className="glass h-52 animate-pulse rounded-card" />
        <div className="glass h-72 animate-pulse rounded-card" />
      </div>
      <div className="flex flex-col gap-4">
        <div className="glass h-32 animate-pulse rounded-card" />
        <div className="glass h-36 animate-pulse rounded-card" />
        <div className="glass h-64 animate-pulse rounded-card" />
      </div>
    </div>
  );
}
