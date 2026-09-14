import type { FitPart } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * The components behind a fit score, each on the same 0-100 scale.
 *
 * One hue at one step: these are five readings of the same quantity, not five
 * categories, so colour carries magnitude and nothing else. The 50 mark is
 * drawn because it is the meaningful midpoint — "the same as your own average"
 * — and a bar without it would leave the reader guessing where neutral sits.
 *
 * The weight is printed beside each label: a component the backend dropped for
 * want of data is simply absent, and the remaining weights are the ones that
 * actually produced the total.
 */
export function FitBreakdown({
  parts,
  id,
  className,
}: {
  parts: FitPart[];
  id?: string;
  className?: string;
}) {
  return (
    <ul id={id} className={cn("m-0 flex list-none flex-col gap-3 p-0", className)}>
      {parts.map((part) => (
        <li key={part.component} className="flex flex-col gap-1.5">
          <div className="flex items-baseline justify-between gap-3">
            <span className="text-xs text-ink-muted">
              {part.label}
              <span className="ml-1.5 font-mono text-[0.625rem] text-ink-faint">
                {Math.round(part.weight * 100)}%
              </span>
            </span>
            <span className="font-mono text-xs tabular-nums text-ink">
              {Math.round(part.score)}
            </span>
          </div>

          <div
            role="meter"
            aria-valuenow={Math.round(part.score)}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={`${part.label}: ${part.detail}`}
            className="relative h-2 w-full overflow-hidden rounded-full bg-mark-track"
          >
            <div
              className="h-full rounded-r bg-operator transition-[width] duration-500 ease-out"
              style={{ width: `${Math.max(0, Math.min(100, part.score))}%` }}
            />
            {/* Neutral: level with your own average. */}
            <span
              aria-hidden
              className="absolute inset-y-0 left-1/2 w-px bg-base/70"
            />
          </div>

          <p className="text-[0.6875rem] leading-relaxed text-ink-faint">
            {part.detail}
          </p>
        </li>
      ))}
    </ul>
  );
}
