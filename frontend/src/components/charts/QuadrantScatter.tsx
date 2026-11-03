"use client";

import { useState } from "react";

import { cn } from "@/lib/utils";

export interface ScatterPoint {
  id: string;
  x: number;
  y: number;
  /** Diameter in px. */
  size: number;
  /** Names the point, e.g. a hero. */
  label: string;
  /** Lines for the tooltip, e.g. "+6 pts win rate". */
  detail: string[];
  /** The mark itself, e.g. a portrait. Fills a circle of `size`. */
  mark: React.ReactNode;
}

/**
 * Points on two axes split into four named quadrants.
 *
 * Hover (or keyboard focus) shows a tooltip; a mouse click selects. On touch,
 * a tap shows the tooltip, which carries the select button, so a small
 * screen never jumps away on the first tap.
 */
export function QuadrantScatter({
  points,
  xDomain,
  yDomain,
  xSplit,
  ySplit,
  xLabel,
  yLabel,
  quadrants,
  onSelect,
  selectLabel = "Show",
}: {
  points: ScatterPoint[];
  xDomain: [number, number];
  yDomain: [number, number];
  xSplit: number;
  ySplit: number;
  xLabel: string;
  yLabel: string;
  /** Top-left, top-right, bottom-left, bottom-right. */
  quadrants: [string, string, string, string];
  onSelect?: (id: string) => void;
  selectLabel?: string;
}) {
  const [active, setActive] = useState<string | null>(null);
  const px = (x: number) => ((x - xDomain[0]) / (xDomain[1] - xDomain[0])) * 100;
  const py = (y: number) => ((y - yDomain[0]) / (yDomain[1] - yDomain[0])) * 100;
  const clamp = (v: number) => Math.max(2, Math.min(98, v));
  const focus = points.find((p) => p.id === active);

  return (
    <div className="flex flex-col gap-2">
      <div
        className="relative h-72 w-full overflow-visible rounded-xl border border-glass-edge bg-surface-2/40 sm:h-80"
        onPointerLeave={(e) => e.pointerType === "mouse" && setActive(null)}
      >
        {/* Quadrant split lines. */}
        <span aria-hidden className="absolute inset-y-0 w-px bg-border" style={{ left: `${px(xSplit)}%` }} />
        <span aria-hidden className="absolute inset-x-0 h-px bg-border" style={{ bottom: `${py(ySplit)}%` }} />

        {/* Quadrant names, recessive. */}
        {quadrants.map((name, i) => (
          <span
            key={name}
            aria-hidden
            className={cn(
              "pointer-events-none absolute text-[0.625rem] uppercase tracking-wider text-ink-faint",
              i < 2 ? "top-2" : "bottom-2",
              i % 2 === 0 ? "left-2" : "right-2",
            )}
          >
            {name}
          </span>
        ))}

        {points.map((p) => (
          <button
            key={p.id}
            type="button"
            aria-label={`${p.label}: ${p.detail.join(", ")}`}
            onFocus={() => setActive(p.id)}
            onBlur={() => setActive((a) => (a === p.id ? null : a))}
            onPointerEnter={(e) => e.pointerType === "mouse" && setActive(p.id)}
            onClick={(e) => {
              // A mouse click selects; a tap (no hover) first shows details.
              if ((e.nativeEvent as PointerEvent).pointerType === "touch" && active !== p.id) {
                setActive(p.id);
                return;
              }
              onSelect?.(p.id);
            }}
            className={cn(
              "focus-neon absolute -translate-x-1/2 translate-y-1/2 cursor-pointer overflow-hidden rounded-full ring-2 transition-transform duration-150",
              active === p.id ? "z-20 scale-125 ring-ink" : "z-10 ring-surface-2 hover:scale-110",
            )}
            style={{
              left: `${clamp(px(p.x))}%`,
              bottom: `${clamp(py(p.y))}%`,
              width: p.size,
              height: p.size,
            }}
          >
            {p.mark}
          </button>
        ))}

        {focus ? (
          <div
            role="status"
            className="absolute z-30 w-44 rounded-xl border border-glass-edge bg-surface p-2.5 text-xs shadow-xl"
            style={{
              left: `${Math.min(70, Math.max(2, px(focus.x) - 15))}%`,
              bottom: `${py(focus.y) > 55 ? Math.max(2, py(focus.y) - 40) : Math.min(60, py(focus.y) + 12)}%`,
            }}
          >
            <p className="mb-1 font-semibold text-ink">{focus.label}</p>
            {focus.detail.map((line) => (
              <p key={line} className="font-mono text-[0.6875rem] text-ink-muted">
                {line}
              </p>
            ))}
            {onSelect ? (
              <button
                type="button"
                onClick={() => onSelect(focus.id)}
                className="focus-neon mt-1.5 cursor-pointer rounded text-[0.6875rem] text-function hover:text-ink"
              >
                {selectLabel} →
              </button>
            ) : null}
          </div>
        ) : null}
      </div>

      <div className="flex justify-between text-[0.625rem] uppercase tracking-wider text-ink-faint">
        <span>↑ {yLabel}</span>
        <span>{xLabel} →</span>
      </div>
    </div>
  );
}
