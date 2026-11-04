import { Icon, type IconName } from "@/components/ui/Icon";
import { InfoTip } from "@/components/ui/InfoTip";
import { delta, deltaLabel, formatCount } from "@/lib/admin/format";
import { cn } from "@/lib/utils";

/**
 * One headline number, and how it moved against the previous period of the
 * same length.
 *
 * The arrow and the word carry the direction; colour only reinforces it.
 * `higherIsBetter` decides which way is good — more signups is good, more
 * expired trials is not.
 */
export function KpiCard({
  label,
  value,
  previous,
  format = formatCount,
  icon,
  higherIsBetter = true,
  hint,
}: {
  label: string;
  value: number;
  /** Omitted for a figure with nothing to compare against ("right now"). */
  previous?: number;
  format?: (n: number) => string;
  icon?: IconName;
  higherIsBetter?: boolean;
  hint?: string;
}) {
  const d = previous === undefined ? null : delta(value, previous);
  const good = d && d.direction !== "flat" && (d.direction === "up") === higherIsBetter;
  const bad = d && d.direction !== "flat" && !good;

  return (
    <div className="glass flex min-w-0 flex-col gap-2 rounded-card p-4">
      <p className="flex items-center gap-1.5 text-xs text-ink-faint">
        {icon ? <Icon name={icon} className="size-4" /> : null}
        <span className="truncate">{label}</span>
        {hint ? <InfoTip label={label}>{hint}</InfoTip> : null}
      </p>
      <p className="font-mono text-2xl tabular-nums text-number">{format(value)}</p>
      {d ? (
        <p
          className={cn(
            "flex items-center gap-1 text-xs",
            good ? "text-string" : bad ? "text-error" : "text-ink-faint",
          )}
          title={`Previous period: ${format(previous as number)}`}
        >
          {d.direction !== "flat" ? (
            <Icon name={d.direction === "up" ? "chevron-up" : "chevron-down"} className="size-3.5" />
          ) : null}
          {deltaLabel(d, format)}
          <span className="sr-only"> (previous period: {format(previous as number)})</span>
        </p>
      ) : (
        <p className="text-xs text-ink-faint">Right now</p>
      )}
    </div>
  );
}
