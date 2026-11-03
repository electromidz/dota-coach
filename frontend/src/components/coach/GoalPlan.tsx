import { formatMetricValue } from "@/components/charts/BulletRow";
import { InfoTip } from "@/components/ui/InfoTip";
import { shortLabel } from "@/lib/benchmark-view";
import { firstSentence } from "@/lib/coach";
import type { GoalOutcome, GoalResult, GoalView, PlanStep } from "@/lib/types";
import { cn, fixOrdinals } from "@/lib/utils";

/** How many recent games the dots show. Mirrors the backend's count. */
const RECENT_GAMES = 5;

/**
 * The training plan as goals: each step's metric, where it is, where it
 * should get to, and whether the last few games got there. Last time's goals
 * are scored first.
 */
export function GoalPlan({
  plan,
  goals,
  previous,
}: {
  plan: PlanStep[];
  goals: GoalView[];
  previous: GoalResult[];
}) {
  if (plan.length === 0 && previous.length === 0) return null;
  const byStep = new Map(goals.map((g) => [g.position, g]));

  return (
    <section className="flex flex-col gap-3" aria-label="Training plan">
      <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
        Training plan
      </h2>

      {previous.length > 0 ? (
        <ul aria-label="Last plan's goals" className="flex flex-wrap gap-2">
          {previous.map((goal) => (
            <PreviousGoal key={goal.position} goal={goal} />
          ))}
        </ul>
      ) : null}

      <ol className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
        {plan.map((step) => {
          const goal = byStep.get(step.position);
          const action = fixOrdinals(step.action);
          return (
            <li key={step.position} className="glass flex min-w-0 flex-col gap-3 rounded-card p-4">
              <header className="flex items-start gap-2">
                <span
                  aria-hidden
                  className="flex size-6 shrink-0 items-center justify-center rounded-full border border-warn/40 font-mono text-xs text-warn"
                >
                  {step.position}
                </span>
                <h3 className="min-w-0 flex-1 text-sm leading-snug text-ink">
                  {fixOrdinals(step.title)}
                </h3>
                <InfoTip label={`How: ${step.title}`} align="end">
                  {action}
                </InfoTip>
              </header>
              <p className="line-clamp-2 text-xs text-ink-muted">{firstSentence(action)}</p>
              {goal ? <Goal goal={goal} /> : null}
            </li>
          );
        })}
      </ol>
    </section>
  );
}

function Goal({ goal }: { goal: GoalView }) {
  const pct = Math.round(goal.progress * 100);
  const met = goal.progress >= 1;
  const slots = Array.from({ length: RECENT_GAMES }, (_, i) => {
    const offset = RECENT_GAMES - goal.recent.length;
    return i < offset ? null : goal.recent[i - offset];
  });

  return (
    <div className="flex flex-col gap-2 border-t border-border pt-3">
      <div className="flex items-baseline justify-between gap-2 text-xs">
        <span className="truncate text-ink-muted">{shortLabel(goal.metric, goal.label)}</span>
        <span className="font-mono tabular-nums">
          <span className="text-ink">{formatMetricValue(goal.current)}</span>
          <span className="text-ink-faint"> / {formatMetricValue(goal.target)}</span>
        </span>
      </div>
      <div
        role="meter"
        aria-label={`${goal.label}: ${formatMetricValue(goal.current)} of a ${goal.target_label.toLowerCase()} target of ${formatMetricValue(goal.target)}`}
        aria-valuenow={pct}
        aria-valuemin={0}
        aria-valuemax={100}
        className="h-2 overflow-hidden rounded-full bg-mark-track"
      >
        <div
          className={cn("h-full rounded-full", met ? "bg-mark-win" : "bg-warn")}
          style={{ width: `${pct}%` }}
        />
      </div>
      <div className="flex items-center justify-between gap-2">
        <span className="text-[0.6875rem] text-ink-faint">Target: {goal.target_label}</span>
        <span
          className="flex items-center gap-1"
          role="img"
          aria-label={`Last ${goal.recent.length} games: ${goal.recent.filter(Boolean).length} hit the target`}
        >
          {slots.map((hit, i) => (
            <span
              key={i}
              className={cn(
                "size-2.5 rounded-full",
                hit === null ? "bg-mark-track/60" : hit ? "bg-mark-win" : "bg-mark-loss",
              )}
            />
          ))}
        </span>
      </div>
    </div>
  );
}

const OUTCOME: Record<GoalOutcome, { glyph: string; label: string; style: string }> = {
  hit: { glyph: "✓", label: "Hit", style: "border-string/40 bg-string/10 text-string" },
  missed: { glyph: "✗", label: "Missed", style: "border-error/40 bg-error/10 text-error" },
  in_progress: { glyph: "→", label: "In progress", style: "border-warn/40 bg-warn/10 text-warn" },
};

function PreviousGoal({ goal }: { goal: GoalResult }) {
  const o = OUTCOME[goal.outcome];
  return (
    <li
      className={cn("flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[0.6875rem]", o.style)}
      title={`${o.label}: ${goal.label}`}
    >
      <span aria-hidden>{o.glyph}</span>
      <span className="sr-only">{o.label}:</span>
      {shortLabel(goal.metric, goal.label)}
      <span className="font-mono tabular-nums text-ink-muted">
        {formatMetricValue(goal.baseline)} →{" "}
        {goal.current !== null ? formatMetricValue(goal.current) : "—"} /{" "}
        {formatMetricValue(goal.target)}
      </span>
    </li>
  );
}
