"use client";

import { useCallback, useEffect, useState } from "react";

import { Analysis } from "@/components/coach/Analysis";
import { CoachingFocusHeader } from "@/components/coach/CoachingFocusHeader";
import { PlayerModelPanel } from "@/components/coach/PlayerModelPanel";
import { RoleBenchmark } from "@/components/coach/RoleBenchmark";
import { RoleSelection } from "@/components/coach/RoleSelection";
import { TrainingFocusCard } from "@/components/coach/TrainingFocusCard";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { analyzeCoach, ApiError, getCoach, getRoleSelection } from "@/lib/api";
import { useSession } from "@/lib/session-context";
import type { CoachResponse, RoleSelectionResponse } from "@/lib/types";

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
  const [error, setError] = useState<string | null>(null);
  /** Set when the player asks to revisit a choice they have already made. */
  const [changingRole, setChangingRole] = useState(false);

  const load = useCallback(async () => {
    try {
      // Independent reads: neither costs a model call, and the role screen must
      // render even on a deployment with no model configured at all.
      const [coach, roles] = await Promise.all([
        // Coaching refuses to answer before a role is chosen, which is the
        // correct behaviour and not an error here — it is the state this
        // screen exists to resolve.
        getCoach().catch((e) => {
          if (e instanceof ApiError && e.status === 409) return null;
          throw e;
        }),
        getRoleSelection(),
      ]);
      setData(coach);
      setSelection(roles);
      setError(null);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not load your coach.");
    }
  }, []);

  useEffect(() => {
    if (session.kind === "signed-in") void load();
  }, [session.kind, load]);

  if (session.kind === "loading") return <CoachSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;
  if (session.kind === "error") {
    return <Alert title="Cannot reach the service">{session.message}</Alert>;
  }

  if (error) return <Alert>{error}</Alert>;
  if (!selection) return <CoachSkeleton />;

  const profile = selection.profile;

  // No role chosen, or the player is reconsidering: the question is the screen.
  if (!profile || changingRole) {
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
          }}
        />
      </div>
    );
  }

  if (!data) return <CoachSkeleton />;

  // A role with no eligible matches behind it. A legitimate choice — the
  // player may be learning the position — and a state the coach has to state
  // plainly rather than fill with advice it cannot support.
  if (data.evidence.length === 0) {
    return (
      <div className="flex flex-col gap-6 pb-4">
        <CoachingFocusHeader
          profile={profile}
          analysis={selection.analysis}
          scope={selection.scope}
          onChangeRole={() => setChangingRole(true)}
        />
        <Alert tone="info">
          No eligible {profile.selected_role_label} matches yet. Play some
          Ranked or public All Pick games in this role — or pick a different one
          — and the coach will have something to read.
        </Alert>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-10 pb-4">
      {/* What is being coached, and why that role. Always first: every screen
          below is read against it. */}
      <CoachingFocusHeader
        profile={profile}
        analysis={selection.analysis}
        scope={selection.scope}
        onChangeRole={() => setChangingRole(true)}
      />

      {/* Role-scoped: the focus, the benchmark, the evidence and the analysis
          below are all computed from this role's eligible matches and nothing
          else. */}
      <TrainingFocusCard />

      <RoleBenchmark roleLabel={profile.selected_role_label} />

      <Analysis
        data={data}
        onGenerate={analyzeCoach}
        generateLabel={`Analyse my ${profile.selected_role_label} games`}
        emptyHint="No analysis yet. The measured evidence below is ready; ask the coach to interpret it."
      />

      {/* The long-term model is deliberately *not* role-scoped: role affinity
          and "which roles do you actually play" are questions about the player
          across every role, and narrowing them would answer a different one.
          Labelled, so the change of scope is visible rather than inferred. */}
      <section className="flex flex-col gap-4">
        <p className="text-xs uppercase tracking-wider text-ink-faint">
          Across every role
        </p>
        <PlayerModelPanel />
      </section>
    </div>
  );
}

function CoachSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading your coach…</span>
      <div className="h-28 animate-pulse rounded-card bg-surface-2" />
      <div className="h-40 animate-pulse rounded-card bg-surface-2" />
      <div className="h-64 animate-pulse rounded-card bg-surface-2" />
    </div>
  );
}
