import { Card } from "@/components/ui/Card";
import { groupEvidence } from "@/lib/coach";
import type { Confidence, Evidence } from "@/lib/types";
import { cn, fixOrdinals } from "@/lib/utils";

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

      {groups.map((group) => {
        const shared = sharedSample(group.kind, group.items);
        return (
          <Card key={group.label} className="flex flex-col gap-3">
            <h3 className="flex flex-wrap items-baseline gap-2 text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
              {group.label}
              {shared ? (
                <span
                  className={cn(
                    "font-mono text-[0.625rem] normal-case tracking-normal",
                    CONFIDENCE_CLASS[shared.confidence],
                  )}
                >
                  {shared.sample} {shared.sample === 1 ? "match" : "matches"} ·{" "}
                  {CONFIDENCE_LABEL[shared.confidence]}
                </span>
              ) : null}
            </h3>

            <ul className="m-0 flex list-none flex-col gap-3 p-0">
              {group.items.map((item) => (
                <li key={item.id} className="flex flex-col gap-1">
                  <span className="flex flex-wrap items-baseline gap-2">
                    <span className="text-xs uppercase tracking-wider text-ink-muted">
                      {item.label}
                    </span>
                    {shared ? null : (
                      <span
                        className={cn(
                          "font-mono text-[0.625rem]",
                          CONFIDENCE_CLASS[item.confidence],
                        )}
                      >
                        {item.sample} {item.sample === 1 ? "match" : "matches"}{" "}
                        · {CONFIDENCE_LABEL[item.confidence]}
                      </span>
                    )}
                  </span>
                  <span className="text-sm leading-relaxed text-ink">
                    {fixOrdinals(item.statement)}
                  </span>
                </li>
              ))}
            </ul>
          </Card>
        );
      })}
    </section>
  );
}

/**
 * One count for a section whose rows all describe the same set of matches.
 *
 * "Since your last session" compares two sessions, so every row in it rests
 * on the current session's matches. The count is shown once on the heading;
 * older stored analyses put 0 on the header row, so the largest row count is
 * the session's.
 */
export function sharedSample(
  kind: Evidence["kind"],
  items: Evidence[],
): { sample: number; confidence: Confidence } | null {
  if (kind !== "progress" || items.length === 0) return null;
  const best = items.reduce((a, b) => (b.sample > a.sample ? b : a));
  return { sample: best.sample, confidence: best.confidence };
}
