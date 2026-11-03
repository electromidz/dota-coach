import { cn } from "@/lib/utils";

export interface DivergingRow {
  key: string;
  label: string;
  /** Signed so positive is always good. */
  value: number;
  /** Shown on hover/focus, e.g. "You 512 · Ancient 612". */
  detail?: string;
}

/**
 * One bar per row, out from a centre line: good to the right in the win teal,
 * bad to the left in the loss red. Every bar carries its value as text, so neither direction
 * nor colour is needed to read it.
 */
export function DivergingBars({
  rows,
  format = (v) => `${Math.round(v)}%`,
  label,
}: {
  rows: DivergingRow[];
  format?: (absolute: number) => string;
  label: string;
}) {
  const max = Math.max(10, ...rows.map((r) => Math.abs(r.value)));

  return (
    <ul aria-label={label} className="flex flex-1 flex-col justify-center gap-3">
      {rows.map((row) => {
        const good = row.value >= 0;
        // Capped short of the edge so the value label always fits beside the bar.
        const width = `${(Math.abs(row.value) / max) * 36}%`;
        const text = `${good ? "+" : "−"}${format(Math.abs(row.value))}`;
        return (
          <li
            key={row.key}
            tabIndex={0}
            aria-label={`${row.label}: ${text}${row.detail ? `. ${row.detail}` : ""}`}
            className="focus-neon group relative grid grid-cols-[5.5rem_1fr] items-center gap-3 rounded"
          >
            <span className="truncate text-xs text-ink-muted">{row.label}</span>
            <div className="relative h-5">
              <span aria-hidden className="absolute inset-y-0 left-1/2 w-px bg-border" />
              <span
                aria-hidden
                className={cn(
                  "absolute inset-y-1 rounded-sm",
                  good ? "left-1/2 rounded-l-none bg-mark-win" : "right-1/2 rounded-r-none bg-mark-loss",
                )}
                style={{ width }}
              />
              <span
                aria-hidden
                className={cn(
                  "absolute top-1/2 -translate-y-1/2 font-mono text-[0.6875rem] tabular-nums",
                  good ? "text-string" : "text-error",
                )}
                style={
                  good
                    ? { left: `calc(50% + ${width} + 0.375rem)` }
                    : { right: `calc(50% + ${width} + 0.375rem)` }
                }
              >
                {text}
              </span>
            </div>
            {row.detail ? (
              <span
                aria-hidden
                className="pointer-events-none absolute -top-7 right-0 z-10 hidden whitespace-nowrap rounded-lg border border-glass-edge bg-surface px-2 py-1 font-mono text-[0.6875rem] text-ink shadow-lg group-hover:block group-focus:block"
              >
                {row.detail}
              </span>
            ) : null}
          </li>
        );
      })}
    </ul>
  );
}
