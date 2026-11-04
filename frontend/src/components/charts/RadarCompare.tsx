"use client";

import { useState } from "react";

import { formatMetricValue } from "@/components/charts/BulletRow";
import type { PercentilePoint } from "@/lib/benchmark-view";
import { cn, ordinal } from "@/lib/utils";

const SIZE = 340;
const C = SIZE / 2;
const R = 112;
const RINGS = [25, 50, 75, 100];
const PAD = 44;

/**
 * You against your rank and a target rank, every axis a percentile (0-100) in
 * your own bracket so metrics with different units share one shape.
 *
 * - You: filled polygon in the accent.
 * - Your rank: the p50 ring, grey dashed — a rank median is p50 by definition.
 * - Target: amber dashed, the target median placed on your bracket's scale.
 *
 * Hover or focus an axis for its raw figures. A visually hidden table carries
 * the same numbers for screen readers.
 */
export function RadarCompare({
  points,
  rankLabel,
  targetLabel,
  youLegend = "You",
  targetLegend,
}: {
  points: PercentilePoint[];
  rankLabel: string;
  targetLabel?: string;
  /** Legend text for the filled shape. */
  youLegend?: string;
  /** Legend text for the dashed target shape, when it is not a median. */
  targetLegend?: string;
}) {
  const [active, setActive] = useState<number | null>(null);
  const n = points.length;

  const angle = (i: number) => -Math.PI / 2 + (i / n) * Math.PI * 2;
  const at = (i: number, p: number, radius = R) => {
    const r = (Math.max(0, Math.min(100, p)) / 100) * radius;
    return [C + Math.cos(angle(i)) * r, C + Math.sin(angle(i)) * r] as const;
  };
  const polygon = (values: number[]) =>
    values.map((v, i) => at(i, v).map((c) => c.toFixed(1)).join(",")).join(" ");

  const you = points.map((p) => p.you ?? 0);
  const showRank = points.every((p) => p.rankMedian !== null);
  const showTarget =
    Boolean(targetLabel) && points.every((p) => p.targetMedian !== null);
  const focus = active !== null ? points[active] : null;

  return (
    <div className="flex flex-col gap-3">
      <div className="relative mx-auto aspect-square w-full max-w-[30rem]">
        <svg
          // Padded past the plot so the outer axis labels never clip.
          viewBox={`${-PAD} ${-PAD / 3} ${SIZE + PAD * 2} ${SIZE + (PAD * 2) / 3}`}
          className="size-full overflow-visible"
          aria-hidden
          onMouseLeave={() => setActive(null)}
        >
          {/* Grid: faint rings and spokes, so the data is the loudest thing. */}
          {RINGS.map((ring) => (
            <polygon
              key={ring}
              points={polygon(points.map(() => ring))}
              fill="none"
              stroke="var(--color-mark-track)"
              strokeOpacity={ring === 100 ? 0.9 : 0.5}
              strokeWidth={1}
            />
          ))}
          {points.map((_, i) => {
            const [x, y] = at(i, 100);
            return (
              <line
                key={i}
                x1={C}
                y1={C}
                x2={x}
                y2={y}
                stroke={active === i ? "var(--color-ink-faint)" : "var(--color-mark-track)"}
                strokeOpacity={active === i ? 1 : 0.5}
              />
            );
          })}

          {showRank ? (
            <polygon
              points={polygon(points.map((p) => p.rankMedian ?? 0))}
              fill="none"
              stroke="var(--color-rank)"
              strokeWidth={1.5}
              strokeDasharray="4 4"
            />
          ) : null}
          {showTarget ? (
            <polygon
              points={polygon(points.map((p) => p.targetMedian ?? 0))}
              fill="none"
              stroke="var(--color-target)"
              strokeWidth={2}
              strokeDasharray="7 4"
            />
          ) : null}

          <polygon
            points={polygon(you)}
            fill="var(--color-you)"
            fillOpacity={0.18}
            stroke="var(--color-you)"
            strokeWidth={2}
            strokeLinejoin="round"
          />
          {you.map((v, i) => {
            const [x, y] = at(i, v);
            return (
              <circle
                key={i}
                cx={x}
                cy={y}
                r={active === i ? 5.5 : 4}
                fill="var(--color-you)"
                stroke="var(--color-surface-2)"
                strokeWidth={2}
              />
            );
          })}

          {/* Axis labels with the percentile underneath; each is also the
              hover target for its axis. */}
          {points.map((p, i) => {
            const [x, y] = at(i, 100, R + 30);
            const anchor =
              Math.abs(x - C) < 8 ? "middle" : x > C ? "start" : "end";
            return (
              <g
                key={p.metric}
                onMouseEnter={() => setActive(i)}
                className="cursor-default"
              >
                <circle cx={x} cy={y} r={26} fill="transparent" />
                <text
                  x={x}
                  y={y - 3}
                  textAnchor={anchor}
                  className={cn(
                    "text-[11px]",
                    active === i ? "fill-ink" : "fill-ink-muted",
                  )}
                >
                  {p.axis}
                </text>
                <text
                  x={x}
                  y={y + 11}
                  textAnchor={anchor}
                  className="fill-ink-faint font-mono text-[10px]"
                >
                  {p.you !== null ? ordinal(p.you) : "—"}
                </text>
              </g>
            );
          })}
        </svg>

        {/* Keyboard access to the same per-axis detail. */}
        <div className="sr-only">
          {points.map((p, i) => (
            <button
              key={p.metric}
              type="button"
              onFocus={() => setActive(i)}
              onBlur={() => setActive(null)}
            >
              {p.label}
            </button>
          ))}
        </div>

        {focus ? (
          <div
            role="status"
            className="pointer-events-none absolute left-1/2 top-1/2 z-10 w-48 -translate-x-1/2 -translate-y-1/2 rounded-xl border border-glass-edge bg-surface/95 p-3 text-xs shadow-xl"
          >
            <p className="mb-1.5 font-semibold text-ink">{focus.label}</p>
            <Row
              swatch="bg-you"
              name={youLegend}
              value={formatMetricValue(focus.raw.you)}
              extra={focus.you !== null ? ordinal(focus.you) : undefined}
            />
            {focus.raw.rankMedian !== null ? (
              <Row swatch="bg-rank" name={rankLabel} value={formatMetricValue(focus.raw.rankMedian)} />
            ) : null}
            {targetLabel && focus.raw.targetMedian !== null ? (
              <Row
                swatch="bg-target"
                name={targetLabel}
                value={formatMetricValue(focus.raw.targetMedian)}
                extra={
                  focus.targetMedian !== null
                    ? ordinal(focus.targetMedian)
                    : undefined
                }
              />
            ) : null}
          </div>
        ) : null}
      </div>

      <ul className="flex flex-wrap justify-center gap-x-4 gap-y-1 text-xs text-ink-muted">
        <LegendItem kind="you">{youLegend}</LegendItem>
        {showRank ? <LegendItem kind="rank">{rankLabel} median</LegendItem> : null}
        {showTarget ? (
          <LegendItem kind="target">{targetLegend ?? `${targetLabel} median`}</LegendItem>
        ) : null}
      </ul>

      <table className="sr-only">
        <caption>Percentile in {rankLabel}, by metric</caption>
        <thead>
          <tr>
            <th>Metric</th>
            <th>You</th>
            <th>{rankLabel} median</th>
            {targetLabel ? <th>{targetLabel} median</th> : null}
          </tr>
        </thead>
        <tbody>
          {points.map((p) => (
            <tr key={p.metric}>
              <th>{p.label}</th>
              <td>
                {formatMetricValue(p.raw.you)}
                {p.you !== null ? ` (${ordinal(p.you)})` : ""}
              </td>
              <td>{p.raw.rankMedian !== null ? formatMetricValue(p.raw.rankMedian) : "—"}</td>
              {targetLabel ? (
                <td>
                  {p.raw.targetMedian !== null ? formatMetricValue(p.raw.targetMedian) : "—"}
                </td>
              ) : null}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function Row({
  swatch,
  name,
  value,
  extra,
}: {
  swatch: string;
  name: string;
  value: string;
  extra?: string;
}) {
  return (
    <p className="flex items-center justify-between gap-2 py-0.5">
      <span className="flex items-center gap-1.5 text-ink-muted">
        <span aria-hidden className={cn("size-2 rounded-full", swatch)} />
        {name}
      </span>
      <span className="font-mono tabular-nums text-ink">
        {value}
        {extra ? <span className="ml-1.5 text-ink-faint">{extra}</span> : null}
      </span>
    </p>
  );
}

export function LegendItem({
  kind,
  children,
}: {
  kind: "you" | "rank" | "target";
  children: React.ReactNode;
}) {
  return (
    <li className="flex items-center gap-1.5">
      {kind === "you" ? (
        <span aria-hidden className="h-2.5 w-4 rounded-sm border-2 border-you bg-you/20" />
      ) : (
        <span
          aria-hidden
          className={cn(
            "w-4 border-t-2 border-dashed",
            kind === "rank" ? "border-rank" : "border-target",
          )}
        />
      )}
      {children}
    </li>
  );
}
