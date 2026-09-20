import type { Evidence, Insight, InsightKind } from "./types";

/**
 * Presentation helpers for coaching.
 *
 * The important one is `citedBy`: an insight arrives carrying evidence *ids*,
 * and the client renders the backend's own statements for them. The model's
 * prose never supplies a number — it points at one.
 */

/** Kind colours, warmest for the things that need acting on. */
export const INSIGHT_CLASS: Record<InsightKind, string> = {
  strength: "border-string/50 bg-string/10 text-string",
  improvement: "border-function/50 bg-function/10 text-function",
  recommendation: "border-keyword/50 bg-keyword/10 text-keyword",
  recurring_pattern: "border-operator/50 bg-operator/10 text-operator",
  weakness: "border-number/50 bg-number/10 text-number",
  warning: "border-error/50 bg-error/10 text-error",
};

/**
 * Resolve an insight's citations against the evidence it was validated with.
 *
 * Ids the backend verified always resolve; anything that does not is dropped
 * rather than rendered as a dangling reference.
 */
export function citedBy(insight: Insight, evidence: Evidence[]): Evidence[] {
  return insight.evidence
    .map((id) => evidence.find((item) => item.id === id))
    .filter((item): item is Evidence => item !== undefined);
}

/** Evidence grouped by kind, in reading order. */
export const EVIDENCE_GROUPS: Array<{ kind: Evidence["kind"]; label: string }> = [
  { kind: "match", label: "This match" },
  // Change leads, where there is any: "you are dying less than last time" is
  // the thing a returning player came back to read.
  { kind: "progress", label: "Since your last session" },
  { kind: "focus", label: "Training focus" },
  { kind: "pattern", label: "Recurring patterns" },
  { kind: "overall", label: "Career" },
  { kind: "form", label: "Recent form" },
  { kind: "benchmark", label: "Against peers" },
  { kind: "hero", label: "Heroes" },
];

export function groupEvidence(
  evidence: Evidence[],
): Array<{ label: string; items: Evidence[] }> {
  return EVIDENCE_GROUPS.map(({ kind, label }) => ({
    label,
    items: evidence.filter((item) => item.kind === kind),
  })).filter((group) => group.items.length > 0);
}

/** `"2026-09-14T10:00:00Z"` -> `"14 Sep 2026, 10:00"`. */
export function formatGeneratedAt(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime())
    ? "unknown"
    : date.toLocaleString(undefined, {
        day: "numeric",
        month: "short",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
}
