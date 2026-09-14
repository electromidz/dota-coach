"use client";

import { useState } from "react";

import { FitBreakdown } from "@/components/heroes/FitBreakdown";
import { Card } from "@/components/ui/Card";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { formatScore, LEVEL_CLASS, TIER_CLASS } from "@/lib/hero-intel";
import type { HeroFit } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * One scored hero.
 *
 * The score leads, the reasons follow, and the breakdown is one tap away — the
 * spec's rule is that a fit score must be explainable, not that every number
 * has to be on screen at once. Caveats are never collapsed: what the score
 * cannot see is the part a reader most needs before acting on it.
 */
export function RecommendationCard({ fit }: { fit: HeroFit }) {
  const [open, setOpen] = useState(false);
  const detailId = `fit-detail-${fit.hero_id}`;

  return (
    <Card className="flex flex-col gap-4">
      <div className="flex items-start gap-3">
        <HeroPortrait heroId={fit.hero_id} heroName={fit.hero_name} size="md" />

        <div className="flex min-w-0 flex-1 flex-col gap-1.5">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="truncate font-display text-base text-ink">
              {fit.hero_name}
            </h3>
            <span
              className={cn(
                "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
                LEVEL_CLASS[fit.level],
              )}
            >
              {fit.level_label}
            </span>
            {fit.tier ? (
              <span
                className={cn(
                  "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
                  TIER_CLASS[fit.tier],
                )}
              >
                {fit.tier}
              </span>
            ) : null}
          </div>

          <p className="font-mono text-xs tabular-nums text-ink-faint">
            {fit.matches} {fit.matches === 1 ? "match" : "matches"}
            {fit.meta_strength !== null
              ? ` · meta ${formatScore(fit.meta_strength)}`
              : " · meta unavailable"}
          </p>
        </div>

        <div className="flex shrink-0 flex-col items-end">
          <span className="font-mono text-2xl tabular-nums text-number">
            {formatScore(fit.fit_score)}
          </span>
          <span className="text-[0.625rem] uppercase tracking-wider text-ink-faint">
            fit
          </span>
        </div>
      </div>

      {fit.reasons.length > 0 ? (
        <ul className="m-0 flex list-none flex-col gap-1 p-0">
          {fit.reasons.map((reason) => (
            <li key={reason} className="text-sm leading-relaxed text-ink-muted">
              {reason}
            </li>
          ))}
        </ul>
      ) : null}

      {fit.caveats.length > 0 ? (
        <ul className="m-0 flex list-none flex-col gap-1 p-0">
          {fit.caveats.map((caveat) => (
            <li
              key={caveat}
              className="text-xs leading-relaxed text-ink-faint before:mr-1.5 before:text-number before:content-['·']"
            >
              {caveat}
            </li>
          ))}
        </ul>
      ) : null}

      <div className="flex flex-col gap-3">
        <button
          type="button"
          onClick={() => setOpen((was) => !was)}
          aria-expanded={open}
          aria-controls={detailId}
          className="focus-neon min-h-11 cursor-pointer self-start text-xs uppercase tracking-wider text-ink-faint transition-colors duration-200 hover:text-ink"
        >
          {open ? "Hide the breakdown" : "Why this score?"}
        </button>

        {open ? <FitBreakdown id={detailId} parts={fit.parts} /> : null}
      </div>
    </Card>
  );
}
