import { Card } from "@/components/ui/Card";
import { groupEvidence } from "@/lib/coach";
import type { Confidence, Evidence } from "@/lib/types";
import { cn } from "@/lib/utils";

/** Sample size is printed on every row; this only flags what it means. */
const CONFIDENCE_CLASS: Record<Confidence, string> = {
  insufficient: "text-ink-faint",
  low: "text-number",
  adequate: "text-string",
};

const CONFIDENCE_LABEL: Record<Confidence, string> = {
  insufficient: "too few to generalize",
  low: "small sample",
  adequate: "solid sample",
};

/**
 * Everything the coach can currently see.
 *
 * Shown whether or not a model is configured: these sentences are the product
 * of the deterministic layers, and they are useful on their own. A player can
 * read exactly what any advice was built from.
 */
export function EvidenceList({ evidence }: { evidence: Evidence[] }) {
  const groups = groupEvidence(evidence);

  if (groups.length === 0) {
    return null;
  }

  return (
    <section className="flex flex-col gap-4">
      <header className="flex flex-col gap-1">
        <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          What the coach measured
        </h2>
        <p className="text-xs leading-relaxed text-ink-faint">
          Every sentence below is computed in the backend. The model interprets
          them; it never produces a number of its own.
        </p>
      </header>

      {groups.map((group) => (
        <Card key={group.label} className="flex flex-col gap-3">
          <h3 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
            {group.label}
          </h3>

          <ul className="m-0 flex list-none flex-col gap-3 p-0">
            {group.items.map((item) => (
              <li key={item.id} className="flex flex-col gap-1">
                <span className="flex flex-wrap items-baseline gap-2">
                  <span className="text-xs uppercase tracking-wider text-ink-muted">
                    {item.label}
                  </span>
                  <span
                    className={cn(
                      "font-mono text-[0.625rem]",
                      CONFIDENCE_CLASS[item.confidence],
                    )}
                  >
                    {item.sample} {item.sample === 1 ? "match" : "matches"} ·{" "}
                    {CONFIDENCE_LABEL[item.confidence]}
                  </span>
                </span>
                <span className="text-sm leading-relaxed text-ink">
                  {item.statement}
                </span>
              </li>
            ))}
          </ul>
        </Card>
      ))}
    </section>
  );
}
