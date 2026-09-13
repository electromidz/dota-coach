import { cn } from "@/lib/utils";

const W = 240;
const H = 56;
const PAD = 6;

/**
 * Single-series trend line.
 *
 * One series, so there is no legend — the caption above names what is plotted.
 * Marks follow the house spec: a 2px line, a ~10% area wash beneath it, and an
 * 8px end marker carrying a 2px surface ring so it stays legible where it
 * crosses the line. Only the endpoint is labelled; a number on every point is
 * noise.
 */
export function Sparkline({
  values,
  label,
  formatValue = (v) => v.toFixed(1),
  className,
}: {
  values: number[];
  /** Names the series. Rendered by the caller; used here for the a11y text. */
  label: string;
  formatValue?: (value: number) => string;
  className?: string;
}) {
  if (values.length < 2) {
    return (
      <p className={cn("text-sm text-ink-faint", className)}>
        Not enough matches yet to plot a trend.
      </p>
    );
  }

  const min = Math.min(...values);
  const max = Math.max(...values);
  // A flat series would divide by zero; give it a centred band instead.
  const span = max - min || 1;

  const x = (i: number) => PAD + (i / (values.length - 1)) * (W - PAD * 2);
  const y = (v: number) => H - PAD - ((v - min) / span) * (H - PAD * 2);

  const line = values.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
  const area = `${PAD},${H - PAD} ${line} ${W - PAD},${H - PAD}`;

  const last = values[values.length - 1];

  return (
    <figure className={cn("m-0", className)}>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="h-14 w-full overflow-visible"
        role="img"
        aria-label={`${label}: ${values.length} matches, latest ${formatValue(last)}, range ${formatValue(min)} to ${formatValue(max)}.`}
      >
        <polyline points={area} fill="var(--color-mark-line)" fillOpacity={0.1} stroke="none" />
        <polyline
          points={line}
          fill="none"
          stroke="var(--color-mark-line)"
          strokeWidth={2}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        {/* End marker: r=4 gives the 8px minimum, ringed in the surface colour. */}
        <circle
          cx={x(values.length - 1)}
          cy={y(last)}
          r={4}
          fill="var(--color-mark-line)"
          stroke="var(--color-surface-2)"
          strokeWidth={2}
        />
      </svg>
    </figure>
  );
}
