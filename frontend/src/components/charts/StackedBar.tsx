import { cn } from "@/lib/utils";

export interface StackedSegment {
  key: string;
  label: string;
  /** Any positive amount; segments are drawn as shares of the total. */
  value: number;
  /** Shown under the segment, e.g. "45". Defaults to the share in percent. */
  display?: string;
}

/**
 * One bar split into labelled parts, e.g. what a score is made of.
 *
 * One hue at stepped opacity rather than a palette of its own: the segments
 * are parts of one thing, and every segment carries its label, so identity
 * never rests on colour.
 */
export function StackedBar({
  segments,
  label,
}: {
  segments: StackedSegment[];
  label: string;
}) {
  const total = segments.reduce((sum, s) => sum + Math.max(0, s.value), 0) || 1;
  const steps = [1, 0.7, 0.48, 0.3, 0.2];

  return (
    <figure className="m-0 flex flex-col gap-2" aria-label={label}>
      <div className="flex h-3 w-full gap-0.5 overflow-hidden rounded-full" role="img" aria-label={
        segments.map((s) => `${s.label} ${s.display ?? Math.round((s.value / total) * 100)}`).join(", ")
      }>
        {segments.map((s, i) => (
          <span
            key={s.key}
            className="h-full bg-you first:rounded-l-full last:rounded-r-full"
            style={{
              width: `${(Math.max(0, s.value) / total) * 100}%`,
              opacity: steps[i] ?? 0.2,
            }}
          />
        ))}
      </div>
      <figcaption className="flex flex-wrap gap-x-3 gap-y-1 text-[0.6875rem] text-ink-muted">
        {segments.map((s, i) => (
          <span key={s.key} className="inline-flex items-center gap-1.5">
            <span
              aria-hidden
              className={cn("size-2 rounded-sm bg-you")}
              style={{ opacity: steps[i] ?? 0.2 }}
            />
            {s.label}
            <span className="font-mono tabular-nums text-ink">
              {s.display ?? Math.round((s.value / total) * 100)}
            </span>
          </span>
        ))}
      </figcaption>
    </figure>
  );
}
