"use client";

import { useEffect, useState } from "react";

import { Card } from "@/components/ui/Card";
import { InfoTip } from "@/components/ui/InfoTip";
import { getMatchTimeline } from "@/lib/api";
import type { MatchTimelineResponse } from "@/lib/types";
import { formatDuration } from "@/lib/utils";

const W = 600;
const H = 140;
const PAD_L = 40;
const PAD_R = 8;
const PAD_T = 8;
const PAD_B = 18;

/**
 * The player's net worth minute by minute, with their deaths on a lane
 * underneath on the same time axis. Only when the replay was parsed; the
 * coaching section already says when it was not, so this renders nothing.
 */
export function NetWorthTimeline({ id }: { id: string }) {
  const [data, setData] = useState<MatchTimelineResponse | null>(null);

  useEffect(() => {
    let cancelled = false;
    getMatchTimeline(id)
      .then((r) => {
        if (!cancelled) setData(r);
      })
      // A missing timeline costs this chart, nothing else.
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [id]);

  if (!data || !data.available || data.net_worth_per_minute.length < 2) return null;
  return <Chart data={data} />;
}

function Chart({ data }: { data: MatchTimelineResponse }) {
  const [hover, setHover] = useState<number | null>(null);
  const series = data.net_worth_per_minute;
  const minutes = series.length - 1;
  const endSeconds = Math.max(data.duration_seconds, minutes * 60);
  const max = Math.max(...series) || 1;
  const plotW = W - PAD_L - PAD_R;
  const plotH = H - PAD_T - PAD_B;
  const x = (seconds: number) => PAD_L + (seconds / endSeconds) * plotW;
  const y = (v: number) => PAD_T + plotH - (v / max) * plotH;
  const line = series.map((v, m) => `${x(m * 60).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
  const area = `${x(0)},${y(0)} ${line} ${x(minutes * 60)},${y(0)}`;
  const gridValues = [0.5, 1].map((f) => Math.round((max * f) / 1000) * 1000).filter((v) => v > 0);

  function onMove(e: React.MouseEvent<SVGSVGElement>) {
    const rect = e.currentTarget.getBoundingClientRect();
    const px = ((e.clientX - rect.left) / rect.width) * W;
    const minute = Math.round(((px - PAD_L) / plotW) * (endSeconds / 60));
    setHover(Math.max(0, Math.min(minutes, minute)));
  }

  return (
    <Card className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <h3 className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
          Net worth &amp; deaths
          <InfoTip label="About this chart">
            Your own net worth at each minute from the parsed replay, with each
            death marked below on the same clock.
          </InfoTip>
        </h3>
        <span className="font-mono text-[0.6875rem] tabular-nums text-ink-faint">
          {hover !== null
            ? `${hover}:00 · ${series[hover].toLocaleString("en-US")}`
            : `${data.deaths.length} ${data.deaths.length === 1 ? "death" : "deaths"}`}
        </span>
      </div>

      <svg
        viewBox={`0 0 ${W} ${H + 22}`}
        className="w-full overflow-visible"
        role="img"
        aria-label={`Net worth from 0 to ${series[minutes].toLocaleString("en-US")} over ${minutes} minutes, ${data.deaths.length} deaths.`}
        onMouseMove={onMove}
        onMouseLeave={() => setHover(null)}
      >
        {gridValues.map((v) => (
          <g key={v}>
            <line x1={PAD_L} x2={W - PAD_R} y1={y(v)} y2={y(v)} stroke="var(--color-mark-track)" strokeOpacity={0.6} />
            <text x={PAD_L - 6} y={y(v) + 3} textAnchor="end" className="fill-ink-faint font-mono text-[10px]">
              {v >= 1000 ? `${v / 1000}k` : v}
            </text>
          </g>
        ))}
        <line x1={PAD_L} x2={W - PAD_R} y1={y(0)} y2={y(0)} stroke="var(--color-mark-track)" />

        <polygon points={area} fill="var(--color-you)" fillOpacity={0.12} />
        <polyline
          points={line}
          fill="none"
          stroke="var(--color-you)"
          strokeWidth={2}
          strokeLinejoin="round"
          strokeLinecap="round"
        />

        {/* Death lane, on the same time axis. */}
        <line x1={PAD_L} x2={W - PAD_R} y1={H + 10} y2={H + 10} stroke="var(--color-mark-track)" strokeOpacity={0.6} />
        <text x={PAD_L - 6} y={H + 13} textAnchor="end" className="fill-ink-faint text-[10px]">
          deaths
        </text>
        {data.deaths.map((d, i) => (
          <g key={i}>
            <title>
              {`Died at ${formatDuration(d.time_seconds)}${d.killer_hero_name ? ` to ${d.killer_hero_name}` : ""}`}
            </title>
            <line x1={x(d.time_seconds)} x2={x(d.time_seconds)} y1={PAD_T} y2={H + 4} stroke="var(--color-mark-loss)" strokeOpacity={0.25} />
            <circle cx={x(d.time_seconds)} cy={H + 10} r={4.5} fill="var(--color-mark-loss)" stroke="var(--color-surface-2)" strokeWidth={2} />
          </g>
        ))}

        {hover !== null ? (
          <g pointerEvents="none">
            <line x1={x(hover * 60)} x2={x(hover * 60)} y1={PAD_T} y2={y(0)} stroke="var(--color-ink-faint)" strokeDasharray="3 3" />
            <circle cx={x(hover * 60)} cy={y(series[hover])} r={4} fill="var(--color-you)" stroke="var(--color-surface-2)" strokeWidth={2} />
          </g>
        ) : null}

        {[0, 10, 20, 30, 40, 50, 60, 70, 80, 90]
          .filter((m) => m * 60 <= endSeconds)
          .map((m) => (
            <text key={m} x={x(m * 60)} y={H - 2} textAnchor="middle" className="fill-ink-faint font-mono text-[10px]">
              {m}
            </text>
          ))}
      </svg>
    </Card>
  );
}
