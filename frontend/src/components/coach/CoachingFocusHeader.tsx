import { ScopeLine } from "@/components/ui/ScopeLine";
import { InfoTip } from "@/components/ui/InfoTip";
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
  extra,
}: {
  profile: CoachingProfile;
  analysis: RoleAnalysis;
  scope: AnalysisScopeInfo;
  onChangeRole: () => void;
  /** Trailing content, e.g. the analysis's (i). */
  extra?: React.ReactNode;
}) {
  const performance = analysis.roles.find(
    (role) => role.role === profile.selected_role,
  );

  return (
    <div className="glass flex flex-wrap items-center gap-x-4 gap-y-2 rounded-card px-4 py-3">
      <span className="text-[0.6875rem] uppercase tracking-[0.16em] text-ink-faint">
        Coaching
      </span>
      <span className="font-display text-lg text-keyword">{profile.selected_role_label}</span>
      <span className="font-mono text-xs tabular-nums text-ink-muted">
        {performance ? (
          <>
            {performance.matches} {performance.matches === 1 ? "game" : "games"} ·{" "}
            {formatPercent(performance.win_rate)} won
          </>
        ) : (
          "No eligible games yet"
        )}
      </span>
      <InfoTip label="Which games this reads">
        <span>
          Ranked and public All Pick games as {profile.selected_role_label}; the last{" "}
          {scope.analyzed_matches} eligible {scope.analyzed_matches === 1 ? "match" : "matches"}.
        </span>
        {profile.overrode_recommendation && profile.recommended_role_label ? (
          <span>
            Your measured performance was stronger as {profile.recommended_role_label}; you
            chose {profile.selected_role_label}.
          </span>
        ) : null}
      </InfoTip>
      {extra}
      <button
        type="button"
        onClick={onChangeRole}
        className="focus-neon ml-auto cursor-pointer rounded text-xs text-function transition-colors duration-200 ease-out hover:text-ink"
      >
        Change role
      </button>
      <ScopeLine
        role={profile.selected_role_label}
        matches={performance?.matches ?? 0}
        className="w-full"
      />
    </div>
  );
}
