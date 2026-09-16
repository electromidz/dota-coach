"use client";

import { useState } from "react";

import { RoleScoreBreakdown } from "@/components/coach/RoleScoreBreakdown";
import { ConfidenceBadge } from "@/components/dashboard/ScopeBanner";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { ApiError, selectCoachingRole } from "@/lib/api";
import { gamesUntilRecommendable } from "@/lib/roles";
import { formatPercent } from "@/lib/stats";
import type {
  CoachableRole,
  RolePerformance,
  RoleSelectionResponse,
} from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * "Which role do you want to improve?"
 *
 * The screen is built around one rule: the recommendation is advice and the
 * choice is the player's. So the recommended role is shown first, with the
 * measured reason it was picked — and every other role is a button of the same
 * weight right beside it, including roles with no matches behind them. Nothing
 * is disabled, nothing nags, and picking against the advice takes exactly one
 * tap, the same as accepting it.
 */
export function RoleSelection({
  data,
  onSelected,
}: {
  data: RoleSelectionResponse;
  /** Called with the server's answer once a role is stored. */
  onSelected: (next: RoleSelectionResponse) => void;
}) {
  const [pending, setPending] = useState<CoachableRole | null>(null);
  const [error, setError] = useState<string | null>(null);

  const { analysis, selectable_roles: selectable, profile } = data;
  const recommended = analysis.recommendation;
  const byRole = new Map(analysis.roles.map((r) => [r.role, r]));

  async function choose(role: CoachableRole) {
    setPending(role);
    setError(null);

    try {
      onSelected(await selectCoachingRole(role));
    } catch (e) {
      setError(
        e instanceof ApiError ? e.message : "Could not save that choice.",
      );
    } finally {
      setPending(null);
    }
  }

  return (
    <div className="flex flex-col gap-5">
      <header className="flex flex-col gap-2">
        <h1 className="text-lg text-ink">Which role do you want to improve?</h1>
        <p className="text-sm leading-relaxed text-ink-muted">
          Coaching is built around one role at a time. Everything after this —
          your numbers, your heroes, the benchmark and the training plan — is
          measured from your {" "}
          <strong className="font-semibold text-keyword">
            Ranked and public All Pick
          </strong>{" "}
          games in the role you pick.
        </p>
      </header>

      {error ? <Alert title="Could not save">{error}</Alert> : null}

      {recommended ? (
        <Card className="flex flex-col gap-3">
          <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
            <h2 className="text-xs uppercase tracking-wider text-ink-faint">
              Based on your last {analysis.analyzed_matches} eligible matches,
              we suggest
            </h2>
            <ConfidenceBadge
              confidence={recommended.confidence}
              label={analysis.confidence_label}
            />
          </div>

          <p className="font-mono text-2xl text-keyword">
            {recommended.role_label}
          </p>
          <p className="text-sm leading-relaxed text-ink-muted">
            {recommended.why}
          </p>

          {/* The reasoning, not just the verdict. A player overriding the
              advice should be able to see what they are overriding. */}
          {byRole.get(recommended.role) ? (
            <RoleScoreBreakdown
              performance={byRole.get(recommended.role)!}
              className="border-t border-border pt-3"
            />
          ) : null}

          <Button
            onClick={() => void choose(recommended.role)}
            disabled={pending !== null}
            className="w-full sm:w-auto"
          >
            <Icon
              name="check"
              className={cn("size-5", pending === recommended.role && "animate-pulse")}
            />
            {pending === recommended.role
              ? "Saving…"
              : `Coach me on ${recommended.role_label}`}
          </Button>

          <p className="text-xs leading-relaxed text-ink-faint">
            This is a suggestion, not a decision. Pick any role below and that is
            what gets coached.
          </p>
        </Card>
      ) : (
        <Alert tone="info" title="No recommendation yet">
          {analysis.note ??
            "There is not enough eligible match data to suggest a role. Pick the one you want to work on."}
        </Alert>
      )}

      <section className="flex flex-col gap-3">
        <h2 className="text-xs uppercase tracking-wider text-ink-faint">
          {recommended ? "Or choose your own" : "Choose a role"}
        </h2>

        <ul className="grid gap-2 sm:grid-cols-2">
          {selectable.map((option) => (
            <li key={option.role}>
              <RoleOption
                label={option.label}
                position={option.position}
                performance={byRole.get(option.role)}
                shortBy={gamesUntilRecommendable(
                  byRole.get(option.role)?.matches ?? 0,
                  analysis.min_recommendable_matches,
                )}
                recommended={recommended?.role === option.role}
                current={profile?.selected_role === option.role}
                pending={pending === option.role}
                disabled={pending !== null}
                onSelect={() => void choose(option.role)}
              />
            </li>
          ))}
        </ul>
      </section>

      {analysis.unclassified_matches > 0 ? (
        <p className="text-xs leading-relaxed text-ink-faint">
          {analysis.unclassified_matches} of your eligible matches could not be
          tied to a specific role — without a parsed replay the data only shows
          whether you played a core or a support. They count towards your overall
          numbers but not towards any single role.
        </p>
      ) : null}
    </div>
  );
}

/**
 * One selectable role.
 *
 * A role with no matches is still a button, and says so rather than being
 * greyed out: wanting to learn a position you do not play is a normal reason to
 * open a coaching app.
 */
function RoleOption({
  label,
  position,
  performance,
  shortBy,
  recommended,
  current,
  pending,
  disabled,
  onSelect,
}: {
  label: string;
  position: number;
  performance?: RolePerformance;
  /** Eligible games still needed before this role could be recommended. */
  shortBy: number;
  recommended: boolean;
  current: boolean;
  pending: boolean;
  disabled: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      disabled={disabled}
      aria-current={current ? "true" : undefined}
      className={cn(
        "focus-neon glass flex w-full cursor-pointer flex-col gap-1.5 rounded-card px-4 py-3 text-left transition-colors duration-200 ease-out",
        "hover:border-border disabled:cursor-not-allowed disabled:opacity-60",
        current && "neon-ring-string",
      )}
    >
      <span className="flex items-baseline justify-between gap-2">
        <span className="flex items-baseline gap-2">
          <span className="text-sm text-ink">{label}</span>
          <span className="font-mono text-[0.6875rem] text-ink-faint">
            pos {position}
          </span>
        </span>

        {recommended ? (
          <span className="text-[0.6875rem] uppercase tracking-wider text-string">
            Suggested
          </span>
        ) : current ? (
          <span className="text-[0.6875rem] uppercase tracking-wider text-string">
            Current
          </span>
        ) : null}
      </span>

      <span className="font-mono text-xs tabular-nums text-ink-muted">
        {pending ? (
          "Saving…"
        ) : performance ? (
          <>
            {Math.round(performance.performance)}/100 · {performance.matches}{" "}
            {performance.matches === 1 ? "game" : "games"} ·{" "}
            {formatPercent(performance.win_rate)}
          </>
        ) : (
          "No eligible matches yet"
        )}
      </span>

      {/* Why a role is missing from the advice, rather than leaving it to be
          read as a silent judgement. Still selectable — a thin sample is a
          reason not to *recommend* a role, never a reason to refuse it. */}
      {shortBy > 0 && !pending ? (
        <span className="text-[0.6875rem] text-ink-faint">
          {shortBy} more {shortBy === 1 ? "game" : "games"} before this can be
          recommended
        </span>
      ) : null}
    </button>
  );
}
