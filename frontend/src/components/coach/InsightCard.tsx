import { Card } from "@/components/ui/Card";
import { citedBy, INSIGHT_CLASS } from "@/lib/coach";
import type { Evidence, Insight } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * One insight, with the measured facts it rests on printed underneath.
 *
 * The evidence is not an appendix — it is the reason the insight is allowed to
 * exist. Every figure on this card comes from the backend's own statement; the
 * model supplied only the title and the explanation above them.
 */
export function InsightCard({
  insight,
  evidence,
}: {
  insight: Insight;
  evidence: Evidence[];
}) {
  const cited = citedBy(insight, evidence);

  return (
    <Card className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <span
          className={cn(
            "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
            INSIGHT_CLASS[insight.kind],
          )}
        >
          {insight.kind_label}
        </span>
        <h3 className="font-display text-base text-ink">{insight.title}</h3>
      </div>

      <p className="text-sm leading-relaxed text-ink-muted">
        {insight.explanation}
      </p>

      {cited.length > 0 ? (
        <ul className="m-0 flex list-none flex-col gap-2 border-l-2 border-glass-edge p-0 pl-3">
          {cited.map((item) => (
            <li key={item.id} className="flex flex-col gap-0.5">
              <span className="text-[0.625rem] uppercase tracking-wider text-ink-faint">
                {item.label}
                <span className="ml-1.5 font-mono normal-case tracking-normal">
                  {item.sample} {item.sample === 1 ? "match" : "matches"}
                </span>
              </span>
              <span className="text-xs leading-relaxed text-ink">
                {item.statement}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
    </Card>
  );
}
