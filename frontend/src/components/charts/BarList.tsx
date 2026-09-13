import { cn } from "@/lib/utils";

export interface BarDatum {
  label: string;
  value: number;
  /** Right-aligned annotation, e.g. a win rate. */
  meta?: string;
}

/**
 * Horizontal bars for comparing magnitude across a handful of named things.
 *
 * Horizontal because the labels are words, not dates. One hue at a single
 * step — magnitude is the job, so this is sequential, not categorical, and
 * there is nothing for a legend to disambiguate. Labels sit outside the bars,
 * so nothing is ever clipped by its own mark.
 */
export function BarList({
  data,
  caption,
  className,
}: {
  data: BarDatum[];
  caption: string;
  className?: string;
}) {
  if (data.length === 0) {
    return <p className={cn("text-sm text-ink-faint", className)}>No data yet.</p>;
  }

  const max = Math.max(...data.map((d) => d.value)) || 1;

  return (
    <figure className={cn("m-0 flex flex-col gap-3", className)}>
      <figcaption className="sr-only">{caption}</figcaption>

      {data.map((d) => (
        <div key={d.label} className="flex flex-col gap-1.5">
          <div className="flex items-baseline justify-between gap-3 text-sm">
            <span className="truncate text-ink">{d.label}</span>
            <span className="shrink-0 font-mono text-xs tabular-nums text-ink-muted">
              {d.meta ? `${d.meta} · ` : ""}
              {d.value}
            </span>
          </div>

          <div className="h-2 w-full overflow-hidden rounded-full bg-mark-track">
            <div
              className="h-full rounded-r bg-operator"
              style={{ width: `${(d.value / max) * 100}%` }}
            />
          </div>
        </div>
      ))}
    </figure>
  );
}
