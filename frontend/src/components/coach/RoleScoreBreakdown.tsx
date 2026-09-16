import { formatComponentValue, formatWeight } from "@/lib/roles";
import type { RolePerformance } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * What the role score is made of.
 *
 * "We recommend Support" is not an answer a player can argue with; "you win
 * 63% as Support and 44% as Carry, and win rate is 45% of the score" is. The
 * recommendation is advisory, so the reasoning has to be inspectable — a
 * player deciding to override it deserves to see what they are overriding.
 *
 * Every figure here was computed by the backend, including the weights, which
 * are renormalised per role around any measure that role had no data for. That
 * is why they are shown rather than assumed: a role missing kill participation
 * genuinely is scored differently, and hiding it would make two scores look
 * more comparable than they are.
 */
export function RoleScoreBreakdown({
  performance,
  className,
}: {
  performance: RolePerformance;
  className?: string;
}) {
  if (performance.components.length === 0) return null;

  const adjusted =
    Math.round(performance.raw_performance) !==
    Math.round(performance.performance);

  return (
    <div className={cn("flex flex-col gap-2", className)}>
      <h4 className="text-[0.6875rem] uppercase tracking-wider text-ink-faint">
        How this score is built
      </h4>

      <ul className="flex flex-col gap-1.5">
        {performance.components.map((component) => (
          <li
            key={component.key}
            className="flex items-baseline justify-between gap-3 text-xs"
          >
            <span className="truncate text-ink-muted">{component.label}</span>
            <span className="flex shrink-0 items-baseline gap-2 font-mono tabular-nums">
              <span className="text-ink">
                {formatComponentValue(component)}
              </span>
              <span className="text-[0.6875rem] text-ink-faint">
                {formatWeight(component.weight)} of score
              </span>
            </span>
          </li>
        ))}
      </ul>

      {adjusted ? (
        <p className="text-[0.6875rem] leading-relaxed text-ink-faint">
          Measured {Math.round(performance.raw_performance)}/100 across{" "}
          {performance.matches}{" "}
          {performance.matches === 1 ? "game" : "games"}, reported as{" "}
          {Math.round(performance.performance)} — a score is pulled toward the
          middle until there are enough games behind it to stand on its own.
        </p>
      ) : null}
    </div>
  );
}
