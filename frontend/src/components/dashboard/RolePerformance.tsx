import Link from "next/link";

import { BarList } from "@/components/charts/BarList";
import { ConfidenceBadge } from "@/components/dashboard/ScopeBanner";
import { Card } from "@/components/ui/Card";
import { formatPercent } from "@/lib/stats";
import type { RoleAnalysis } from "@/lib/types";

/**
 * Performance by role, across the eligible competitive window.
 *
 * The bar is the **score**, not the number of games: "which role am I best at"
 * is the question this card exists to answer, and ranking by games played
 * answers "which role do I play most", which the meta beside each bar already
 * says. Every figure comes from the backend — nothing here divides anything.
 */
export function RolePerformance({ analysis }: { analysis: RoleAnalysis }) {
  const { roles, unclassified_matches: unclassified } = analysis;

  return (
    <Card className="flex min-w-0 flex-col gap-4">
      <div className="flex items-baseline justify-between gap-3">
        <h2 className="text-xs uppercase tracking-wider text-ink-faint">
          Role performance
        </h2>
        {roles.length > 0 ? (
          <span className="font-mono text-[0.6875rem] tabular-nums text-ink-faint">
            score / 100
          </span>
        ) : null}
      </div>

      {roles.length > 0 ? (
        <BarList
          caption="Performance score by role, strongest first"
          data={roles.map((role) => ({
            label: role.role_label,
            value: Math.round(role.performance),
            meta: `${role.matches}g · ${formatPercent(role.win_rate)}`,
          }))}
        />
      ) : (
        <p className="text-sm leading-relaxed text-ink-muted">
          No eligible match could be attributed to a specific role yet.
        </p>
      )}

      {analysis.recommendation ? (
        <div className="flex flex-col gap-1.5 border-t border-border pt-3">
          <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
            <h3 className="text-xs uppercase tracking-wider text-ink-faint">
              Strongest role
              <span className="ml-2 font-mono tracking-normal text-keyword">
                {analysis.recommendation.role_label}
              </span>
            </h3>
            <ConfidenceBadge
              confidence={analysis.recommendation.confidence}
              label={
                analysis.recommendation.confidence.charAt(0).toUpperCase() +
                analysis.recommendation.confidence.slice(1)
              }
            />
          </div>
          <p className="text-xs leading-relaxed text-ink-muted">
            {analysis.recommendation.why}
          </p>
        </div>
      ) : analysis.note ? (
        <p className="border-t border-border pt-3 text-xs leading-relaxed text-ink-faint">
          {analysis.note}
        </p>
      ) : null}

      {/* The handoff the product flow turns on: this card answers "which role
          am I best at", and the next question is always "so coach me at one".
          Kept at the bottom of the card, because the numbers are the reason a
          player would click it. */}
      <Link
        href="/coach"
        className="focus-neon w-fit cursor-pointer rounded text-xs text-function transition-colors duration-200 ease-out hover:text-ink"
      >
        {analysis.recommendation
          ? `Start coaching ${analysis.recommendation.role_label} →`
          : "Start coaching →"}
      </Link>

      {unclassified > 0 ? (
        <p className="text-[0.6875rem] leading-relaxed text-ink-faint">
          {unclassified} eligible {unclassified === 1 ? "match" : "matches"}{" "}
          could not be attributed to one of the five roles. Without a parsed
          replay the data only shows whether you played a core or a support, and
          guessing the lane would put the wrong games in the wrong role.
        </p>
      ) : null}
    </Card>
  );
}
