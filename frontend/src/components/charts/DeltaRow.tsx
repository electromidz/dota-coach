import { Delta } from "@/components/charts/Delta";
import { cn } from "@/lib/utils";

/**
 * One metric, before -> after, with the change: `397 → 420 ▲ +6%`.
 *
 * `status` is the backend's verdict (improved / declined / unchanged). The
 * arrow colour follows it rather than the sign, so "fewer deaths" is green.
 */
export function DeltaRow({
  label,
  before,
  after,
  change,
  status,
  format,
  formatDelta,
}: {
  label: string;
  before: number;
  after: number;
  /** Relative change as a fraction (0.06 = 6%), or null to show the raw delta. */
  change: number | null;
  status: "improved" | "declined" | "stable" | string;
  format: (value: number) => string;
  /** How to write an absolute change; defaults to `format`. Points on a
   *  percentile scale are not themselves percentiles. */
  formatDelta?: (absolute: number) => string;
}) {
  const delta = after - before;
  const good = status === "improved";
  const bad = status === "declined";

  return (
    <li className="grid grid-cols-[minmax(0,1fr)_auto_auto] items-baseline gap-3 py-1.5">
      <span className="truncate text-xs text-ink-muted">{label}</span>
      <span className="font-mono text-xs tabular-nums">
        <span className="text-ink-faint">{format(before)}</span>
        <span aria-hidden className="mx-1.5 text-ink-faint">→</span>
        <span className="sr-only"> to </span>
        <span className="text-ink">{format(after)}</span>
      </span>
      <span className={cn("w-20 text-right", !good && !bad && "opacity-70")}>
        <span className="sr-only">{good ? "Improved" : bad ? "Declined" : "Unchanged"} </span>
        {good || bad ? (
          <Delta
            value={change !== null ? change * 100 : delta}
            lowerIsBetter={(delta > 0) === bad}
            format={(v) =>
              change !== null ? `${Math.round(v)}%` : (formatDelta ?? format)(v)
            }
          />
        ) : (
          <span className="font-mono text-xs text-ink-faint">unchanged</span>
        )}
      </span>
    </li>
  );
}
