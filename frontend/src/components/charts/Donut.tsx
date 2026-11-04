"use client";

import { useState } from "react";

import { cn } from "@/lib/utils";

export interface DonutSlice {
  key: string;
  label: string;
  value: number;
  /** A CSS colour, normally a theme variable. */
  color: string;
  /** Shown in the legend and tooltip instead of the raw value. */
  display?: string;
}

/**
 * Parts of a whole as a ring, with a legend that names every slice (so
 * identity never rests on colour) and the total or hovered slice in the
 * middle. 2px surface gaps between slices.
 */
export function Donut({
  slices,
  label,
  center,
  size = 132,
}: {
  slices: DonutSlice[];
  /** Names the chart for screen readers. */
  label: string;
  /** What the middle says when nothing is hovered. */
  center: { value: string; caption: string };
  size?: number;
}) {
  const [active, setActive] = useState<number | null>(null);
  const total = slices.reduce((sum, s) => sum + s.value, 0) || 1;
  const r = 15.9155; // circumference 100
  // Each slice starts where the previous ones end, as a share of 100.
  const shares = slices.map((s) => (s.value / total) * 100);
  const starts = shares.map((_, i) => shares.slice(0, i).reduce((a, b) => a + b, 0));
  const focus = active !== null ? slices[active] : null;

  return (
    <div className="flex flex-wrap items-center gap-4">
      <div className="relative shrink-0" style={{ width: size, height: size }}>
        <svg viewBox="0 0 42 42" className="size-full -rotate-90" role="img" aria-label={label}>
          <circle cx={21} cy={21} r={r} fill="none" stroke="var(--color-mark-track)" strokeWidth={6} />
          {slices.map((s, i) => {
            const gap = slices.length > 1 ? 0.8 : 0;
            const length = Math.max(0, shares[i] - gap);
            return (
              <circle
                key={s.key}
                cx={21}
                cy={21}
                r={r}
                fill="none"
                stroke={s.color}
                strokeWidth={active === i ? 7 : 6}
                strokeDasharray={`${length} ${100 - length}`}
                strokeDashoffset={-starts[i]}
                onPointerEnter={() => setActive(i)}
                onPointerLeave={() => setActive(null)}
              />
            );
          })}
        </svg>
        <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center">
          <span className="font-mono text-lg tabular-nums text-ink">
            {focus ? focus.display ?? String(focus.value) : center.value}
          </span>
          <span className="max-w-[5.5rem] truncate text-[0.625rem] text-ink-faint">{focus ? focus.label : center.caption}</span>
        </div>
      </div>

      <ul className="flex min-w-0 flex-1 flex-col gap-1.5 text-xs">
        {slices.map((s, i) => (
          <li
            key={s.key}
            className={cn("flex items-center gap-2", active !== null && active !== i && "opacity-60")}
            onPointerEnter={() => setActive(i)}
            onPointerLeave={() => setActive(null)}
          >
            <span aria-hidden className="size-2.5 shrink-0 rounded-sm" style={{ background: s.color }} />
            <span className="min-w-0 flex-1 truncate text-ink-muted">{s.label}</span>
            <span className="font-mono tabular-nums text-ink-faint">{s.display ?? s.value}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
