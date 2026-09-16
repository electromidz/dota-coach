import { RoleScoreBreakdown } from "@/components/coach/RoleScoreBreakdown";
import { Card } from "@/components/ui/Card";
import { ConfidenceBadge } from "@/components/dashboard/ScopeBanner";
import { formatPercent } from "@/lib/stats";
import type {
  AnalysisScopeInfo,
  CoachingProfile,
  RoleAnalysis,
} from "@/lib/types";

/**
 * What is being coached, and on what evidence.
 *
 * The one thing a user must never have to guess at: which role this screen is
 * about, and which games that means. It also states plainly when the player
 * chose against the advice — not as a warning, but because a coach that quietly
 * forgot the disagreement would be hiding the most interesting thing it knows.
 */
export function CoachingFocusHeader({
  profile,
  analysis,
  scope,
  onChangeRole,
}: {
  profile: CoachingProfile;
  analysis: RoleAnalysis;
  scope: AnalysisScopeInfo;
  onChangeRole: () => void;
}) {
  const performance = analysis.roles.find(
    (role) => role.role === profile.selected_role,
  );

  return (
    <Card className="flex flex-col gap-3">
      <div className="flex flex-wrap items-start justify-between gap-x-4 gap-y-2">
        <div className="flex flex-col gap-1">
          <p className="text-xs uppercase tracking-wider text-ink-faint">
            Coaching focus
          </p>
          <p className="font-mono text-2xl text-keyword">
            {profile.selected_role_label}
          </p>
        </div>

        <button
          type="button"
          onClick={onChangeRole}
          className="focus-neon cursor-pointer rounded text-xs text-function transition-colors duration-200 ease-out hover:text-ink"
        >
          Change role
        </button>
      </div>

      <p className="text-sm leading-relaxed text-ink-muted">
        {performance ? (
          <>
            {performance.matches}{" "}
            {performance.matches === 1 ? "game" : "games"} as{" "}
            {profile.selected_role_label} in Ranked and public All Pick ·{" "}
            {formatPercent(performance.win_rate)} won ·{" "}
            {Math.round(performance.performance)}/100
          </>
        ) : (
          <>
            No eligible {profile.selected_role_label} games yet. Your other roles
            still count towards your overall numbers, but they are not evidence
            about this one.
          </>
        )}
      </p>

      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1">
        <p className="text-xs text-ink-faint">
          Read from your last {scope.analyzed_matches} eligible{" "}
          {scope.analyzed_matches === 1 ? "match" : "matches"}.
        </p>
        {performance ? (
          <ConfidenceBadge
            confidence={performance.confidence}
            label={
              performance.confidence.charAt(0).toUpperCase() +
              performance.confidence.slice(1)
            }
          />
        ) : null}
      </div>

      {performance ? (
        <RoleScoreBreakdown
          performance={performance}
          className="border-t border-border pt-3"
        />
      ) : null}

      {profile.overrode_recommendation && profile.recommended_role_label ? (
        <p className="border-t border-border pt-3 text-xs leading-relaxed text-ink-faint">
          Your measured performance was stronger as{" "}
          {profile.recommended_role_label}, and you chose{" "}
          {profile.selected_role_label}. That is the role being coached — the
          recommendation was only advice.
        </p>
      ) : null}
    </Card>
  );
}
