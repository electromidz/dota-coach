import { BulletBar } from "@/components/charts/BulletBar";
import { SampleBadge } from "@/components/ui/SampleBadge";
import { shortLabel } from "@/lib/benchmark-view";
import type { Evidence } from "@/lib/types";
import { cn, ordinal } from "@/lib/utils";

/**
 * One benchmark figure as a bullet: your value as the bar, the peer median and
 * the top-20% line as ticks, the percentile as a badge.
 */
export function PeerBullet({ item }: { item: Evidence }) {
  const f = item.figure;
  if (!f || !f.metric) return null;
  const label = item.label.replace(/ vs peers$/, "");
  const p = f.percentile;

  return (
    <BulletBar
      label={shortLabel(f.metric, label)}
      fullLabel={label}
      value={f.value}
      percentile={p}
      rankMedian={f.median}
      targetMedian={f.top_20}
      cleared={null}
      shortfall={null}
      rankLabel="Median"
      targetLabel="Top 20%"
      trailing={
        <span className="flex items-center gap-1">
          {p !== null ? (
            <span
              className={cn(
                "rounded px-1.5 py-0.5 text-[0.625rem]",
                // Clearly ahead or behind gets a colour; around the median
                // stays neutral so it never contradicts the card's verdict.
                p >= 60
                  ? "bg-string/15 text-string"
                  : p < 40
                    ? "bg-error/15 text-error"
                    : "bg-mark-track text-ink-muted",
              )}
            >
              {ordinal(p)}
            </span>
          ) : (
            <span className="text-ink-faint">—</span>
          )}
          <SampleBadge sample={item.sample} confidence={item.confidence} />
        </span>
      }
    />
  );
}

/** The legend shared by every list of peer bullets. */
export function PeerLegend() {
  return (
    <ul className="flex flex-wrap gap-3 text-[0.6875rem] text-ink-faint">
      <li className="flex items-center gap-1.5">
        <span aria-hidden className="h-2 w-4 rounded-full bg-you" />
        You
      </li>
      <li className="flex items-center gap-1.5">
        <span aria-hidden className="h-3 w-0.5 rounded bg-rank" />
        Median
      </li>
      <li className="flex items-center gap-1.5">
        <span aria-hidden className="h-3 w-1 rounded bg-target" />
        Top 20%
      </li>
    </ul>
  );
}
