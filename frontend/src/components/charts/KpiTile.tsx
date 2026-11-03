import { InfoTip } from "@/components/ui/InfoTip";
import { cn } from "@/lib/utils";

/**
 * One headline number: a label, the value, a comparison and, when there is a
 * series behind it, a sparkline. The comparison is a node so callers can pass
 * a `Delta` or a small meter.
 */
export function KpiTile({
  label,
  value,
  comparison,
  badge,
  info,
  trend,
  visual,
  className,
}: {
  label: string;
  value: React.ReactNode;
  comparison?: React.ReactNode;
  badge?: React.ReactNode;
  info?: React.ReactNode;
  /** Values to sparkline, oldest first. Two or more to draw. */
  trend?: number[];
  /** Replaces the sparkline with any small visual, e.g. a portrait. */
  visual?: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "glass flex min-w-0 flex-col gap-2 rounded-card p-4",
        className,
      )}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="flex min-w-0 items-center gap-1.5 text-[0.6875rem] font-semibold uppercase tracking-[0.16em] text-ink-faint">
          <span className="truncate">{label}</span>
          {info ? <InfoTip label={`About ${label}`}>{info}</InfoTip> : null}
        </span>
        {badge}
      </div>
      <div className="flex items-end justify-between gap-3">
        <span className="min-w-0 truncate font-display text-2xl leading-none text-ink sm:text-3xl">
          {value}
        </span>
        {visual ?? (trend && trend.length > 1 ? <MiniSpark values={trend} /> : null)}
      </div>
      {comparison ? <div className="min-h-4 truncate">{comparison}</div> : null}
    </div>
  );
}

function MiniSpark({ values }: { values: number[] }) {
  const W = 72;
  const H = 28;
  const min = Math.min(...values);
  const span = Math.max(...values) - min || 1;
  const x = (i: number) => 2 + (i / (values.length - 1)) * (W - 4);
  const y = (v: number) => H - 3 - ((v - min) / span) * (H - 6);
  const points = values.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
  const last = values[values.length - 1];

  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="h-7 w-[4.5rem] shrink-0 overflow-visible" aria-hidden>
      <polyline
        points={points}
        fill="none"
        stroke="var(--color-you)"
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx={x(values.length - 1)} cy={y(last)} r={3} fill="var(--color-you)" stroke="var(--color-surface-2)" strokeWidth={1.5} />
    </svg>
  );
}

/** Filled/empty pips, e.g. 6 of 8 metrics cleared. */
export function PipMeter({
  filled,
  total,
  label,
}: {
  filled: number;
  total: number;
  label: string;
}) {
  return (
    <span role="img" aria-label={label} className="flex gap-1">
      {Array.from({ length: total }, (_, i) => (
        <span
          key={i}
          className={cn(
            "h-1.5 flex-1 rounded-full",
            i < filled ? "bg-you" : "bg-mark-track",
          )}
        />
      ))}
    </span>
  );
}
