"use client";

import { useState } from "react";

import { Icon } from "@/components/ui/Icon";
import type { BenchmarkContextInfo } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * What this comparison is, and what it is not.
 *
 * The product asks to benchmark against players of the same rank, role and
 * hero. The provider segments on hero and rank; role and patch it cannot do,
 * and rank itself degrades to all-ranks for an unranked player or a hero it
 * publishes no bracket data for. Showing the gap beats letting the page imply
 * a peer group it never had: a percentile whose peer set is quietly wider than
 * the heading suggests is worse than no percentile, because it looks like an
 * answer. The server decides which caveats apply; this only renders them.
 *
 * The reasons are collapsed by default. A reader who trusts the number should
 * not have to wade through caveats to reach it; a reader who is about to act on
 * it gets the full account in one tap.
 */
export function ComparisonContext({
  context,
  className,
}: {
  context: BenchmarkContextInfo;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const { population, unavailable } = context;

  return (
    <section
      className={cn("flex flex-col gap-2", className)}
      aria-label="What this comparison covers"
    >
      <p className="text-xs leading-relaxed text-ink-muted">
        <span className="text-ink">You:</span> {population.player}{" "}
        <span className="text-ink">Peers:</span> {population.peers}
      </p>

      {unavailable.length > 0 ? (
        <>
          <button
            type="button"
            onClick={() => setOpen((shown) => !shown)}
            aria-expanded={open}
            className="focus-neon flex w-fit cursor-pointer items-center gap-1.5 rounded text-xs text-function transition-colors duration-200 ease-out hover:text-ink"
          >
            <Icon name="alert" className={cn("size-3.5", open && "text-ink")} />
            Not segmented by{" "}
            {unavailable.map((u) => u.label.toLowerCase()).join(", ")}
          </button>

          {open ? (
            <ul className="flex flex-col gap-2 border-l border-border pl-3">
              {unavailable.map((entry) => (
                <li key={entry.segment} className="flex flex-col gap-0.5">
                  <span className="text-xs text-ink">{entry.label}</span>
                  <span className="text-[0.6875rem] leading-relaxed text-ink-faint">
                    {entry.reason}
                  </span>
                </li>
              ))}
              <li className="text-[0.6875rem] leading-relaxed text-ink-faint">
                {population.note}
              </li>
            </ul>
          ) : null}
        </>
      ) : null}
    </section>
  );
}
