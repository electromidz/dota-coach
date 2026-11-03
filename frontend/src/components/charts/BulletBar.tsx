import { Icon } from "@/components/ui/Icon";
import { formatMetricValue } from "@/components/charts/BulletRow";
import { ordinal } from "@/lib/utils";

/**
 * One compact row: your value as a bar, your rank's median and the target's as
 * ticks, then the figure and either a ✓ (target cleared) or the shortfall.
 */
export function BulletBar({
  label,
  fullLabel,
  value,
  percentile,
  rankMedian,
  targetMedian,
  cleared,
  shortfall,
  rankLabel,
  targetLabel,
  note,
  trailing,
}: {
  label: string;
  fullLabel: string;
  value: number;
  percentile: number | null;
  rankMedian: number | null;
  targetMedian: number | null;
  /** True when the target median is already met. Null without a target. */
  cleared: boolean | null;
  /** Server gap to the target median, positive = work to do. */
  shortfall: number | null;
  rankLabel: string;
  targetLabel?: string;
  note?: string | null;
  /** Replaces the default ✓ / shortfall / percentile at the end of the row. */
  trailing?: React.ReactNode;
}) {
  const ceiling = Math.max(value, rankMedian ?? 0, targetMedian ?? 0) * 1.1 || 1;
  const at = (v: number) => `${Math.min(1, Math.max(0, v / ceiling)) * 100}%`;

  const parts = [`${fullLabel}: you ${formatMetricValue(value)}`];
  if (percentile !== null) parts.push(`${ordinal(percentile)} in ${rankLabel}`);
  if (rankMedian !== null) parts.push(`${rankLabel} median ${formatMetricValue(rankMedian)}`);
  if (targetMedian !== null && targetLabel)
    parts.push(`${targetLabel} median ${formatMetricValue(targetMedian)}`);
  if (cleared) parts.push(`past ${targetLabel}`);

  return (
    <li
      className="grid grid-cols-[5.5rem_1fr_auto] items-center gap-3"
      title={note ?? undefined}
    >
      <span className="truncate text-xs text-ink-muted">{label}</span>
      <div
        role="img"
        aria-label={`${parts.join("; ")}.`}
        className="relative h-2 rounded-full bg-mark-track"
      >
        <div
          className="absolute inset-y-0 left-0 rounded-full bg-you"
          style={{ width: at(value) }}
        />
        {rankMedian !== null ? (
          <span
            aria-hidden
            className="absolute -inset-y-1 w-0.5 rounded bg-rank"
            style={{ left: at(rankMedian) }}
          />
        ) : null}
        {targetMedian !== null ? (
          <span
            aria-hidden
            className="absolute -inset-y-1.5 w-1 rounded bg-target ring-2 ring-surface-2"
            style={{ left: at(targetMedian) }}
          />
        ) : null}
      </div>
      <span className="flex w-24 items-center justify-end gap-2 font-mono text-xs tabular-nums">
        <span className="text-ink">{formatMetricValue(value)}</span>
        {trailing !== undefined ? (
          trailing
        ) : cleared ? (
          <Icon name="check" className="size-3.5 text-string" />
        ) : cleared === false && shortfall !== null ? (
          <span className="text-error">−{formatMetricValue(Math.abs(shortfall))}</span>
        ) : percentile !== null ? (
          <span className="text-ink-faint">{ordinal(percentile)}</span>
        ) : null}
      </span>
    </li>
  );
}
