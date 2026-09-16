"use client";

import { useState } from "react";

import { Icon } from "@/components/ui/Icon";
import type { EligibilitySummary, SampleConfidence } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * What the screen below it is reading.
 *
 * The product has two populations — every match the player played, and the
 * eligible competitive ones the coach reasons over — and a user who cannot tell
 * which one they are looking at will read every number wrong. So the scope is
 * stated in words on the screen rather than implied by the route.
 *
 * Built to be reused by the coaching screens, which answer the same question
 * about a narrower population ("your Ranked/public All Pick Carry games").
 */
export function ScopeBanner({
  analyzed,
  confidence,
  confidenceLabel,
  caveat,
  description,
  eligibility,
  className,
}: {
  analyzed: number;
  confidence: SampleConfidence;
  confidenceLabel: string;
  caveat: string;
  /** What the population is, in the backend's own words. */
  description: string;
  /** When present, the excluded matches can be opened and read. */
  eligibility?: EligibilitySummary;
  className?: string;
}) {
  const [showExcluded, setShowExcluded] = useState(false);
  const excludedTotal = eligibility
    ? eligibility.total_matches - eligibility.eligible_matches
    : 0;

  return (
    <section
      className={cn(
        "glass flex flex-col gap-2 rounded-card px-4 py-3",
        className,
      )}
      aria-label="Analysis scope"
    >
      <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
        <p className="text-sm text-ink">
          <span className="font-mono tabular-nums text-keyword">{analyzed}</span>{" "}
          {analyzed === 1 ? "match" : "matches"} analysed
          <span className="text-ink-muted"> · Ranked + public All Pick</span>
        </p>

        <ConfidenceBadge confidence={confidence} label={confidenceLabel} />
      </div>

      <p className="text-xs leading-relaxed text-ink-faint">{caveat}</p>

      {eligibility && excludedTotal > 0 ? (
        <div className="flex flex-col gap-2">
          <button
            type="button"
            onClick={() => setShowExcluded((open) => !open)}
            aria-expanded={showExcluded}
            className="focus-neon flex w-fit cursor-pointer items-center gap-1.5 rounded text-xs text-function transition-colors duration-200 ease-out hover:text-ink"
          >
            <Icon
              name="alert"
              className={cn("size-3.5", showExcluded && "text-ink")}
            />
            {excludedTotal} of {eligibility.total_matches} stored{" "}
            {eligibility.total_matches === 1 ? "match" : "matches"} excluded
          </button>

          {showExcluded ? (
            <ul className="flex flex-col gap-2 border-l border-border pl-3">
              {eligibility.excluded.map((group) => (
                <li key={group.reason} className="flex flex-col gap-0.5">
                  <span className="text-xs text-ink">
                    <span className="font-mono tabular-nums">
                      {group.matches}
                    </span>{" "}
                    × {group.label}
                  </span>
                  <span className="text-[0.6875rem] leading-relaxed text-ink-faint">
                    {group.description}
                  </span>
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}

      <p className="sr-only">{description}</p>
    </section>
  );
}

/**
 * Sample confidence as a word, never as a bare colour: the whole point is that
 * a small sample announces itself.
 */
export function ConfidenceBadge({
  confidence,
  label,
  className,
}: {
  confidence: SampleConfidence;
  label: string;
  className?: string;
}) {
  const tone = {
    limited: "text-error",
    moderate: "text-number",
    strong: "text-string",
  }[confidence];

  return (
    <span
      className={cn(
        "flex items-center gap-1.5 text-[0.6875rem] uppercase tracking-wider text-ink-faint",
        className,
      )}
    >
      Confidence
      <span className={cn("font-mono tracking-normal", tone)}>{label}</span>
    </span>
  );
}
