import { Card } from "@/components/ui/Card";
import type { Evidence, PlanStep } from "@/lib/types";

/**
 * What to actually do next, in order.
 *
 * Every step reached this list by naming the measured weakness it exists to
 * fix, and by stating no figure the backend had not already computed — steps
 * that failed either check were dropped server-side rather than rendered with a
 * caveat. So the evidence line under each step is not a disclaimer; it is the
 * reason the step is there at all, and it is worth showing for the same reason
 * the insights show theirs.
 */
export function TrainingPlan({
  plan,
  evidence,
}: {
  plan: PlanStep[];
  evidence: Evidence[];
}) {
  if (plan.length === 0) return null;

  const labels = new Map(evidence.map((item) => [item.id, item.label]));

  return (
    <section className="flex flex-col gap-3">
      <h3 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
        Your training plan
      </h3>

      <Card className="flex flex-col gap-0 p-0">
        <ol className="flex flex-col">
          {plan.map((step, index) => (
            <li
              key={step.position}
              className={
                index > 0 ? "border-t border-border px-4 py-3" : "px-4 py-3"
              }
            >
              <div className="flex gap-3">
                <span
                  aria-hidden
                  className="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-full border border-keyword/40 font-mono text-xs text-keyword"
                >
                  {step.position}
                </span>

                <div className="flex min-w-0 flex-col gap-1">
                  <p className="text-sm text-ink">{step.title}</p>
                  <p className="text-xs leading-relaxed text-ink-muted">
                    {step.action}
                  </p>
                  <p className="text-[0.6875rem] text-ink-faint">
                    From:{" "}
                    {step.evidence
                      .map((id) => labels.get(id) ?? id)
                      .join(" · ")}
                  </p>
                </div>
              </div>
            </li>
          ))}
        </ol>
      </Card>
    </section>
  );
}
