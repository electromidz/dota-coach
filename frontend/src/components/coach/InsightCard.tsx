import { Card } from "@/components/ui/Card";
import { citedBy, INSIGHT_CLASS } from "@/lib/coach";
import type { Evidence, Insight } from "@/lib/types";
import { cn } from "@/lib/utils";

/** Major reads as urgent; minor reads as context. */
const SEVERITY_CLASS = {
  major: "border-error/50 bg-error/10 text-error",
  minor: "border-glass-edge bg-surface-2 text-ink-muted",
} as const;

/**
 * One insight, with the measured facts it rests on printed underneath.
 *
 * The evidence is not an appendix — it is the reason the insight is allowed to
 * exist. Every figure on this card comes from the backend's own statement; the
 * model supplied only the prose above them.
 *
 * Two shapes, and the card renders whichever arrived. A single-match insight
 * comes split into what happened, why it mattered and what to do instead, and
 * the headings stay on the page rather than being flattened into a paragraph:
 * the third one is the only part the reader can act on, and it should be
 * findable without reading the first two.
 *
 * `rank` numbers the card when it sits in an ordered list of mistakes. The
 * backend hands them over most-important-first, so the number is the ranking
 * rather than decoration.
 */
export function InsightCard({
  insight,
  evidence,
  rank,
}: {
  insight: Insight;
  evidence: Evidence[];
  rank?: number;
}) {
  const cited = citedBy(insight, evidence);
  const split =
    insight.what_happened ?? insight.why_it_matters ?? insight.better_action;

  return (
    <Card className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        {rank !== undefined ? (
          <span
            aria-hidden
            className="flex size-6 shrink-0 items-center justify-center rounded-full border border-keyword/40 font-mono text-xs text-keyword"
          >
            {rank}
          </span>
        ) : null}

        <span
          className={cn(
            "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
            INSIGHT_CLASS[insight.kind],
          )}
        >
          {insight.kind_label}
        </span>

        {insight.severity ? (
          <span
            className={cn(
              "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
              SEVERITY_CLASS[insight.severity],
            )}
          >
            {insight.severity}
          </span>
        ) : null}

        {/* Monospaced and tabular so a column of timestamps lines up, and
            because it is a reading off a clock rather than prose. */}
        {insight.timestamp ? (
          <span className="font-mono text-xs tabular-nums text-operator">
            {insight.timestamp}
          </span>
        ) : null}

        <h3 className="font-display text-base text-ink">{insight.title}</h3>
      </div>

      {split ? (
        <dl className="m-0 flex flex-col gap-3">
          <Part label="What happened" body={insight.what_happened} />
          <Part label="Why it mattered" body={insight.why_it_matters} />
          <Part label="Better play" body={insight.better_action} accent />
        </dl>
      ) : (
        <p className="text-sm leading-relaxed text-ink-muted">
          {insight.explanation}
        </p>
      )}

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

/**
 * One labelled part of a split insight.
 *
 * Absent rather than blank when the model left it out. The backend only accepts
 * the split form with all three present, so this is defence against an older
 * stored row rather than an expected state — and an empty heading would read as
 * a rendering fault.
 */
function Part({
  label,
  body,
  accent = false,
}: {
  label: string;
  body: string | null;
  accent?: boolean;
}) {
  if (!body) return null;

  return (
    <div className="flex flex-col gap-1">
      <dt
        className={cn(
          "text-[0.625rem] uppercase tracking-wider",
          // The advice is the part the reader came for, so it is the one part
          // that is not grey.
          accent ? "text-function" : "text-ink-faint",
        )}
      >
        {label}
      </dt>
      <dd className="m-0 text-sm leading-relaxed text-ink-muted">{body}</dd>
    </div>
  );
}
