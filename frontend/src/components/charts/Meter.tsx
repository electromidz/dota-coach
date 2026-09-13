import { cn } from "@/lib/utils";

/**
 * A single ratio against its limit — the right form for "win rate", where a
 * two-slice pie would be the wrong one.
 *
 * The track is the same ramp as the fill, one step down, so the bar reads as
 * "this much of that" rather than as two competing categories. The value is
 * labelled once, beside the meter.
 */
export function Meter({
  value,
  label,
  valueText,
  className,
}: {
  /** 0-1. */
  value: number;
  label: string;
  valueText: string;
  className?: string;
}) {
  const pct = Math.max(0, Math.min(1, value)) * 100;

  return (
    <figure className={cn("m-0 flex flex-col gap-2", className)}>
      <figcaption className="flex items-baseline justify-between gap-3">
        <span className="text-xs uppercase tracking-wider text-ink-faint">
          {label}
        </span>
        <span className="font-mono text-sm tabular-nums text-ink">
          {valueText}
        </span>
      </figcaption>

      <div
        role="meter"
        aria-valuenow={Math.round(pct)}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={label}
        className="h-2.5 w-full overflow-hidden rounded-full bg-mark-track"
      >
        {/* Rounded data-end, square at the baseline, per the mark spec. */}
        <div
          className="h-full rounded-r bg-mark-win transition-[width] duration-500 ease-out"
          style={{ width: `${pct}%` }}
        />
      </div>
    </figure>
  );
}
