"use client";

import { useState } from "react";

import { cn } from "@/lib/utils";

const W = 640;
const PAD_L = 40;
const PAD_R = 12;
const PAD_T = 12;
const PAD_B = 18;

export interface TrendPoint {
  value: number;
  /** One line for the tooltip, e.g. "Luna · Sep 28 · Won". */
  label: string;
}

/**
 * One series over time, with a crosshair and tooltip on hover.
 *
 * Single series, so no legend: the card title names it. A 2px line, a light
 * area wash, the last point marked and labelled; values for every other
 * point live in the tooltip. `reference` draws a dashed guide (e.g. 50% win
 * rate) that is not part of the data.
 */
export function TrendLine({
  points,
  label,
  format = (v) => v.toFixed(1),
  reference,
  height: H = 180,
  className,
}: {
  points: TrendPoint[];
  /** Names the series for screen readers. */
  label: string;
  format?: (value: number) => string;
  reference?: { value: number; label: string };
  /** Drawing height on the 640-wide grid; it scales with the card width. */
  height?: number;
  className?: string;
}) {
  const [hover, setHover] = useState<number | null>(null);

  if (points.length < 2) {
    return (
      <p className={cn("text-sm text-ink-faint", className)}>
        Not enough matches yet to plot a trend.
      </p>
    );
  }

  const values = points.map((p) => p.value);
  const lowData = Math.min(...values, reference?.value ?? Infinity);
  const highData = Math.max(...values, reference?.value ?? -Infinity);
  const pad = (highData - lowData || Math.abs(highData) || 1) * 0.1;
  const lo = lowData - pad;
  const hi = highData + pad;

  const plotW = W - PAD_L - PAD_R;
  const plotH = H - PAD_T - PAD_B;
  const x = (i: number) => PAD_L + (i / (points.length - 1)) * plotW;
  const y = (v: number) => PAD_T + plotH - ((v - lo) / (hi - lo)) * plotH;

  const line = points.map((p, i) => `${x(i).toFixed(1)},${y(p.value).toFixed(1)}`).join(" ");
  const area = `${x(0)},${PAD_T + plotH} ${line} ${x(points.length - 1)},${PAD_T + plotH}`;
  const last = points.length - 1;
  const ticks = [highData, lowData];

  function onMove(e: React.PointerEvent<SVGSVGElement>) {
    const rect = e.currentTarget.getBoundingClientRect();
    const px = ((e.clientX - rect.left) / rect.width) * W;
    const i = Math.round(((px - PAD_L) / plotW) * last);
    setHover(Math.max(0, Math.min(last, i)));
  }

  const active = hover !== null ? points[hover] : null;
  // Keep the tooltip inside the frame: flip it left past the midpoint.
  const tipLeft = hover !== null ? (x(hover) / W) * 100 : 0;

  return (
    <figure className={cn("relative m-0", className)}>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="h-auto w-full touch-none overflow-visible"
        role="img"
        aria-label={`${label}: ${points.length} points, from ${format(values[0])} to ${format(values[last])}.`}
        onPointerMove={onMove}
        onPointerLeave={() => setHover(null)}
      >
        {ticks.map((v, i) => (
          <g key={i}>
            <line x1={PAD_L} x2={W - PAD_R} y1={y(v)} y2={y(v)} stroke="var(--color-mark-track)" strokeOpacity={0.6} />
            <text x={PAD_L - 6} y={y(v) + 4} textAnchor="end" className="fill-ink-faint font-mono text-[11px]">
              {format(v)}
            </text>
          </g>
        ))}

        {reference ? (
          <g>
            <line
              x1={PAD_L}
              x2={W - PAD_R}
              y1={y(reference.value)}
              y2={y(reference.value)}
              stroke="var(--color-ink-faint)"
              strokeDasharray="4 4"
            />
            <text x={W - PAD_R} y={y(reference.value) - 4} textAnchor="end" className="fill-ink-faint text-[11px]">
              {reference.label}
            </text>
          </g>
        ) : null}

        <polygon points={area} fill="var(--color-you)" fillOpacity={0.1} />
        <polyline
          points={line}
          fill="none"
          stroke="var(--color-you)"
          strokeWidth={2}
          strokeLinejoin="round"
          strokeLinecap="round"
        />

        <circle cx={x(last)} cy={y(values[last])} r={4} fill="var(--color-you)" stroke="var(--color-surface-2)" strokeWidth={2} />
        <text x={x(last) - 8} y={y(values[last]) - 8} textAnchor="end" className="fill-ink font-mono text-[12px]">
          {format(values[last])}
        </text>

        {hover !== null ? (
          <g pointerEvents="none">
            <line x1={x(hover)} x2={x(hover)} y1={PAD_T} y2={PAD_T + plotH} stroke="var(--color-ink-faint)" strokeDasharray="3 3" />
            <circle cx={x(hover)} cy={y(values[hover])} r={4} fill="var(--color-you)" stroke="var(--color-surface-2)" strokeWidth={2} />
          </g>
        ) : null}
      </svg>

      {active ? (
        <div
          role="status"
          className={cn(
            "pointer-events-none absolute top-0 z-10 rounded-lg border border-glass-edge bg-surface-2 px-2.5 py-1.5 text-xs shadow-lg",
            tipLeft > 50 ? "-translate-x-full" : "",
          )}
          style={{ left: `${tipLeft}%` }}
        >
          <p className="font-mono tabular-nums text-ink">{format(active.value)}</p>
          <p className="whitespace-nowrap text-ink-faint">{active.label}</p>
        </div>
      ) : null}
    </figure>
  );
}
