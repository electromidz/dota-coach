"use client";

import { useCallback, useEffect, useState } from "react";

import { Paywall } from "@/components/billing/Paywall";
import { Analysis } from "@/components/coach/Analysis";
import { CoachChat } from "@/components/coach/CoachChat";
import { CoachingFocusHeader } from "@/components/coach/CoachingFocusHeader";
import { PlayerModelPanel } from "@/components/coach/PlayerModelPanel";
import { RoleBenchmark } from "@/components/coach/RoleBenchmark";
import { RoleSelection } from "@/components/coach/RoleSelection";
import { TrainingFocusCard } from "@/components/coach/TrainingFocusCard";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { SectionBoundary, SectionError } from "@/components/ui/SectionError";
import {
  analyzeCoach,
  ApiError,
  getCoach,
  getRoleSelection,
  getTrainingFocus,
} from "@/lib/api";
import { useSession } from "@/lib/session-context";
import type {
  CoachResponse,
  RoleSelectionResponse,
  TrainingFocusResponse,
} from "@/lib/types";

/**
 * The coach.
 *
 * Coaching is role-first: before anything is analysed, the player says which
 * role they want to improve. Until that choice exists this screen *is* the
 * question, because coaching an unstated role would mean picking one for them.
 */
export function Coach() {
  const { session } = useSession();
  /** `null` until loaded, and also when no role has been chosen yet. */
  const [data, setData] = useState<CoachResponse | null>(null);
  const [selection, setSelection] = useState<RoleSelectionResponse | null>(null);
  /** Kept apart so one failing read cannot take the other half of the page. */
  const [coachError, setCoachError] = useState<string | null>(null);
  const [rolesError, setRolesError] = useState<string | null>(null);
  const [coachLoaded, setCoachLoaded] = useState(false);
  const [paywalled, setPaywalled] = useState(false);
  /** Set when the player asks to revisit a choice they have already made. */
  const [changingRole, setChangingRole] = useState(false);
  /** Fetched once for the summary chip and the focus card. `null` after a
   *  failure: the card then fetches on its own and reports its own error. */
  const [focus, setFocus] = useState<TrainingFocusResponse | null | undefined>();

  const handle = useCallback((e: unknown, fallback: string): string | null => {
    if (e instanceof ApiError && e.isPaymentRequired) {
      setPaywalled(true);
      return null;
    }
    return e instanceof ApiError ? e.message : fallback;
  }, []);

  const loadCoach = useCallback(async () => {
    setCoachError(null);
    try {
      // Coaching refuses to answer before a role is chosen, which is the
      // state this screen exists to resolve rather than an error.
      setData(
        await getCoach().catch((e) => {
          if (e instanceof ApiError && e.status === 409) return null;
          throw e;
        }),
      );
    } catch (e) {
      setCoachError(handle(e, "Could not load your coach."));
    } finally {
      setCoachLoaded(true);
    }
  }, [handle]);

  const loadRoles = useCallback(async () => {
    setRolesError(null);
    try {
      setSelection(await getRoleSelection());
    } catch (e) {
      setRolesError(handle(e, "Could not load your coaching role."));
    }
  }, [handle]);

  useEffect(() => {
    // A failed profile read is reported once by the shell; this page
    // still loads its own data.
    if (session.kind === "loading" || session.kind === "anonymous") return;
    // Independent reads: neither costs a model call, and either failing must
    // leave the other standing.
    void loadCoach();
    void loadRoles();
    getTrainingFocus()
      .then(setFocus)
      .catch(() => setFocus(null));
  }, [session.kind, loadCoach, loadRoles]);

  if (session.kind === "loading") return <CoachSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;

  if (paywalled) {
    return <Paywall message="Coaching needs an active trial or subscription." />;
  }
  if (!selection && !rolesError) return <CoachSkeleton />;

  const profile = selection?.profile ?? null;

  // No role chosen, or the player is reconsidering: the question is the screen.
  if (selection && (!profile || changingRole)) {
    return (
      <div className="flex flex-col gap-6 pb-4">
        {changingRole ? (
          <button
            type="button"
            onClick={() => setChangingRole(false)}
            className="focus-neon w-fit cursor-pointer rounded text-xs text-function transition-colors duration-200 ease-out hover:text-ink"
          >
            ← Keep coaching {profile?.selected_role_label}
          </button>
        ) : null}

        <RoleSelection
          data={selection}
          onSelected={(next) => {
            setSelection(next);
            setChangingRole(false);
            void loadCoach();
          }}
        />
      </div>
    );
  }

  const roleLabel = profile?.selected_role_label ?? "role";

  // A role with no eligible matches behind it: a legitimate choice the coach
  // states plainly rather than filling with advice it cannot support.
  if (data && data.evidence.length === 0 && profile && selection) {
    return (
      <div className="flex flex-col gap-6 pb-4">
        <CoachingFocusHeader
          profile={profile}
          analysis={selection.analysis}
          scope={selection.scope}
          onChangeRole={() => setChangingRole(true)}
        />
        <Alert tone="info">
          No eligible {roleLabel} matches yet. Play Ranked or All Pick games in
          this role, or pick a different one.
        </Alert>
      </div>
    );
  }

  const performance = selection?.analysis.roles.find(
    (role) => role.role === profile?.selected_role,
  );
  const isAdmin = session.kind === "signed-in" && session.me.user.is_admin;
  const focusCard = focus === undefined ? null : (
    <SectionBoundary section="Training focus">
      <TrainingFocusCard data={focus ?? undefined} />
    </SectionBoundary>
  );

  return (
    <div className="flex flex-col gap-8 pb-4">
      {rolesError ? (
        <SectionError
          section="Coaching role"
          message={rolesError}
          onRetry={() => void loadRoles()}
        />
      ) : profile && selection ? (
        <SectionBoundary section="Coaching role">
          <CoachingFocusHeader
            profile={profile}
            analysis={selection.analysis}
            scope={selection.scope}
            onChangeRole={() => setChangingRole(true)}
          />
        </SectionBoundary>
      ) : null}

      {coachError ? (
        <>
          <SectionError
            section="Coach analysis"
            message={coachError}
            onRetry={() => void loadCoach()}
          />
          {focusCard}
          <SectionBoundary section="Role benchmark">
            <RoleBenchmark roleLabel={roleLabel} />
          </SectionBoundary>
        </>
      ) : data ? (
        <SectionBoundary section="Coach analysis">
          <Analysis
            data={data}
            onGenerate={analyzeCoach}
            onReload={getCoach}
            generateLabel={`Analyse my ${roleLabel} games`}
            emptyHint="No analysis yet."
            performance={performance}
            focusTitle={focus?.focus?.title ?? null}
            isAdmin={isAdmin}
          >
            {(snapshot) => (
              <>
                {focusCard}
                <SectionBoundary section="Role benchmark">
                  <RoleBenchmark roleLabel={roleLabel} snapshot={snapshot} />
                </SectionBoundary>
              </>
            )}
          </Analysis>
        </SectionBoundary>
      ) : !coachLoaded ? (
        <ReportSkeleton />
      ) : null}

      <SectionBoundary section="Ask your coach">
        <CoachChat roleLabel={roleLabel} />
      </SectionBoundary>

      {/* The long-term model is not role-scoped, and says so. */}
      <section className="flex flex-col gap-4">
        <p className="text-xs uppercase tracking-wider text-ink-faint">
          Across every role
        </p>
        <SectionBoundary section="Player model">
          <PlayerModelPanel />
        </SectionBoundary>
      </section>
    </div>
  );
}

function CoachSkeleton() {
  return (
    <div className="flex flex-col gap-8" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading your coach…</span>
      <div className="h-12 animate-pulse rounded-card bg-surface-2" />
      <ReportSkeleton />
    </div>
  );
}

/** Shaped like the report: four summary tiles, two cards, a plan row. */
function ReportSkeleton() {
  return (
    <div className="flex flex-col gap-8" aria-busy="true">
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {["gauge", "bar", "stack", "chips"].map((kind) => (
          <div key={kind} className="glass flex h-32 flex-col gap-3 rounded-card p-4">
            <div className="h-2.5 w-20 animate-pulse rounded bg-surface-2" />
            {kind === "gauge" ? (
              <div className="mx-auto h-14 w-28 animate-pulse rounded-t-full bg-surface-2" />
            ) : kind === "chips" ? (
              <div className="flex flex-wrap gap-1.5">
                {[16, 20, 14, 18].map((w) => (
                  <div key={w} className="h-6 animate-pulse rounded-full bg-surface-2" style={{ width: `${w * 4}px` }} />
                ))}
              </div>
            ) : (
              <>
                <div className="h-6 w-16 animate-pulse rounded bg-surface-2" />
                <div className="h-2 w-full animate-pulse rounded-full bg-surface-2" />
              </>
            )}
          </div>
        ))}
      </div>
      <div className="grid gap-3 md:grid-cols-2">
        {[0, 1].map((i) => (
          <div key={i} className="glass flex flex-col gap-3 rounded-card p-4">
            <div className="h-4 w-2/3 animate-pulse rounded bg-surface-2" />
            <div className="h-3 w-full animate-pulse rounded bg-surface-2" />
            {[70, 55].map((w) => (
              <div key={w} className="grid grid-cols-[5.5rem_1fr] items-center gap-3">
                <div className="h-2.5 animate-pulse rounded bg-surface-2" />
                <div className="h-2 animate-pulse rounded-full bg-surface-2" style={{ width: `${w}%` }} />
              </div>
            ))}
          </div>
        ))}
      </div>
      <div className="grid gap-3 md:grid-cols-3">
        {[0, 1, 2].map((i) => (
          <div key={i} className="glass h-36 animate-pulse rounded-card" />
        ))}
      </div>
    </div>
  );
}
