"use client";

import { useState } from "react";

import { Segmented } from "@/components/ui/Segmented";
import { medalChanges, medalOf, withinDays } from "@/lib/rank-view";
import type { TrajectoryPoint } from "@/lib/types";
import { useWidth } from "@/lib/useWidth";
import { cn, shortDate } from "@/lib/utils";

const MEDALS = ["", "Herald", "Guardian", "Crusader", "Archon", "Legend", "Ancient", "Divine", "Immortal"];

type Range = "30" | "90" | "all";
const RANGES: { value: Range; label: string }[] = [
  { value: "30", label: "30d" },
  { value: "90", label: "90d" },
  { value: "all", label: "All" },
];

const HEIGHT = 280;
const PAD_L = 64;
const PAD_R = 16;
const PAD_T = 16;
const PAD_B = 24;

/**
 * Rank over time, on a real time axis with medal bands behind it.
 *
 * Measured and modeled points never look alike: a recorded reading is a
 * filled dot on a solid line, a modeled one a hollow dot on a dashed line.
 * After a stretch with no ranked games (`gap_before`) the line breaks.
 * Medal changes are labelled where they happen.
 */
export function TrajectoryChart({ points: all, className }: { points: TrajectoryPoint[]; className?: string }) {
  const [range, setRange] = useState<Range>("90");
  const [ref, width] = useWidth<HTMLDivElement>();
  const [hover, setHover] = useState<number | null>(null);

  const points = withinDays(all, range === "all" ? null : Number(range));

  const toolbar = (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <Legend />
      <Segmented label="Time range" value={range} onChange={setRange} options={RANGES} />
    </div>
  );

  if (points.length < 2) {
    return (
      <div className={cn("flex flex-col gap-3", className)}>
        {toolbar}
        <p className="text-sm text-ink-faint">
          {all.length < 2
            ? "One rank reading so far. A second gives this a shape — sync again tomorrow."
            : "Fewer than two readings in this range. Try a longer one."}
        </p>
      </div>
    );
  }

  const times = points.map((p) => new Date(p.at).getTime());
  const t0 = times[0];
  const t1 = times[times.length - 1];
  const tiers = points.map((p) => p.rank_tier);
  // A star of room above and below, and never less than one medal of span.
  let lo = Math.min(...tiers) - 1;
  let hi = Math.max(...tiers) + 1;
  if (hi - lo < 6) {
    const mid = (hi + lo) / 2;
    lo = mid - 3;
    hi = mid + 3;
  }

  const plotW = Math.max(1, width - PAD_L - PAD_R);
  const plotH = HEIGHT - PAD_T - PAD_B;
  const x = (t: number) => PAD_L + ((t - t0) / (t1 - t0 || 1)) * plotW;
  const y = (tier: number) => PAD_T + plotH - ((tier - lo) / (hi - lo)) * plotH;

  // Medal bands: each medal spans tiers m*10+1 … m*10+5, drawn from
  // m*10+0.5 to m*10+5.5 and clipped to the plot.
  const medals: number[] = [];
  for (let m = medalOf(Math.floor(lo)); m <= medalOf(Math.ceil(hi)); m++) if (MEDALS[m]) medals.push(m);

  const segments = points.slice(1).flatMap((p, i) => {
    if (p.gap_before) return [];
    const prev = points[i];
    return [{ key: p.at, x1: x(times[i]), y1: y(prev.rank_tier), x2: x(times[i + 1]), y2: y(p.rank_tier), estimated: prev.estimated || p.estimated }];
  });
  const changes = medalChanges(points);
  const xTicks = [0, 0.25, 0.5, 0.75, 1].map((f) => t0 + (t1 - t0) * f);

  function onMove(e: React.PointerEvent<SVGSVGElement>) {
    const rect = e.currentTarget.getBoundingClientRect();
    const px = ((e.clientX - rect.left) / rect.width) * width;
    let best = 0;
    for (let i = 1; i < times.length; i++) if (Math.abs(x(times[i]) - px) < Math.abs(x(times[best]) - px)) best = i;
    setHover(best);
  }

  const active = hover !== null ? points[hover] : null;
  const measured = points.filter((p) => !p.estimated).length;

  return (
    <div className={cn("flex flex-col gap-3", className)}>
      {toolbar}
      <div ref={ref} className="relative w-full">
        <svg
          width={width}
          height={HEIGHT}
          viewBox={`0 0 ${width} ${HEIGHT}`}
          style={{ maxWidth: "100%", height: "auto" }}
          role="img"
          aria-label={`Rank from ${points[0].label ?? points[0].rank_tier} to ${points[points.length - 1].label ?? points[points.length - 1].rank_tier}, ${shortDate(points[0].at)} to ${shortDate(points[points.length - 1].at)}. ${measured} recorded rank readings; ${points.length - measured} estimated points modeled between them.`}
          className="block touch-none overflow-visible"
          onPointerMove={onMove}
          onPointerLeave={() => setHover(null)}
        >
          <defs>
            <clipPath id="trajectory-plot">
              <rect x={PAD_L} y={PAD_T} width={plotW} height={plotH} />
            </clipPath>
          </defs>

          <g clipPath="url(#trajectory-plot)">
            {medals.map((m) => (
              <rect
                key={m}
                x={PAD_L}
                width={plotW}
                y={y(m * 10 + 5.5)}
                height={y(m * 10 + 0.5) - y(m * 10 + 5.5)}
                fill={m % 2 === 0 ? "var(--color-surface-2)" : "var(--color-mark-track)"}
                fillOpacity={m % 2 === 0 ? 0.5 : 0.25}
              />
            ))}
          </g>
          {medals.map((m) => {
            const mid = Math.min(Math.max(m * 10 + 3, lo + 0.5), hi - 0.5);
            return (
              <text key={`l-${m}`} x={PAD_L - 8} y={y(mid) + 4} textAnchor="end" className="fill-ink-faint text-[11px]">
                {MEDALS[m]}
              </text>
            );
          })}

          {xTicks.map((t, i) => (
            <text
              key={i}
              x={x(t)}
              y={HEIGHT - 6}
              textAnchor={i === 0 ? "start" : i === xTicks.length - 1 ? "end" : "middle"}
              className="fill-ink-faint text-[10px]"
            >
              {shortDate(new Date(t).toISOString())}
            </text>
          ))}

          {segments.map((s) => (
            <line
              key={s.key}
              x1={s.x1}
              y1={s.y1}
              x2={s.x2}
              y2={s.y2}
              stroke="var(--color-mark-line)"
              strokeWidth={2}
              strokeLinecap="round"
              strokeDasharray={s.estimated ? "4 4" : undefined}
              strokeOpacity={s.estimated ? 0.7 : 1}
            />
          ))}

          {changes.map((i) => (
            <g key={`c-${i}`}>
              <line x1={x(times[i])} x2={x(times[i])} y1={PAD_T} y2={PAD_T + plotH} stroke="var(--color-ink-faint)" strokeOpacity={0.4} strokeDasharray="2 3" />
              <text
                x={x(times[i])}
                y={PAD_T + 10}
                textAnchor={x(times[i]) > width - 80 ? "end" : "start"}
                dx={x(times[i]) > width - 80 ? -4 : 4}
                className="fill-ink-muted text-[10px]"
              >
                → {MEDALS[medalOf(points[i].rank_tier)]}
              </text>
            </g>
          ))}

          {points.map((p, i) => (
            <circle
              key={p.at}
              cx={x(times[i])}
              cy={y(p.rank_tier)}
              r={p.estimated ? 3 : 4}
              fill={p.estimated ? "var(--color-surface-2)" : "var(--color-mark-line)"}
              stroke={p.estimated ? "var(--color-mark-line)" : "var(--color-surface-2)"}
              strokeWidth={2}
            />
          ))}

          {hover !== null ? (
            <line x1={x(times[hover])} x2={x(times[hover])} y1={PAD_T} y2={PAD_T + plotH} stroke="var(--color-ink-faint)" strokeDasharray="3 3" pointerEvents="none" />
          ) : null}
        </svg>

        {active && hover !== null ? (
          <div
            role="status"
            className={cn(
              "pointer-events-none absolute top-0 z-10 rounded-lg border border-glass-edge bg-surface-2 px-2.5 py-1.5 text-xs shadow-lg",
              x(times[hover]) > width / 2 ? "-translate-x-full" : "",
            )}
            style={{ left: x(times[hover]) + (x(times[hover]) > width / 2 ? -10 : 10) }}
          >
            <p className="font-semibold text-ink">{active.label ?? `Tier ${active.rank_tier}`}</p>
            <p className="text-ink-faint">
              {shortDate(active.at)} · {active.estimated ? "estimated" : "recorded rank"}
            </p>
          </div>
        ) : null}
      </div>
    </div>
  );
}

function Legend() {
  return (
    <ul className="flex flex-wrap items-center gap-4 text-[0.6875rem] text-ink-faint">
      <li className="flex items-center gap-1.5">
        <Swatch />
        Recorded rank
      </li>
      <li className="flex items-center gap-1.5">
        <Swatch estimated />
        Estimated
      </li>
      <li>Break · no ranked games</li>
    </ul>
  );
}

function Swatch({ estimated = false }: { estimated?: boolean }) {
  return (
    <svg aria-hidden viewBox="0 0 24 10" className="h-2.5 w-6 shrink-0">
      <line
        x1={1}
        y1={5}
        x2={23}
        y2={5}
        stroke="var(--color-mark-line)"
        strokeWidth={2}
        strokeLinecap="round"
        strokeDasharray={estimated ? "4 4" : undefined}
        strokeOpacity={estimated ? 0.7 : 1}
      />
      <circle
        cx={12}
        cy={5}
        r={estimated ? 3 : 4}
        fill={estimated ? "var(--color-surface-2)" : "var(--color-mark-line)"}
        stroke={estimated ? "var(--color-mark-line)" : "var(--color-surface-2)"}
        strokeWidth={2}
      />
    </svg>
  );
}
