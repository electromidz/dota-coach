"use client";

import { useId, useMemo, useState } from "react";

import { cn } from "@/lib/utils";
import { useWidth } from "@/lib/useWidth";

export interface TimeSeries {
  key: string;
  label: string;
  /** A CSS colour, from the fixed categorical order (`--color-series-N`). */
  color: string;
}

export interface TimeDatum {
  /** `YYYY-MM-DD`. */
  date: string;
  [key: string]: string | number;
}

const HEIGHT = 240;
const PAD = { top: 12, right: 12, bottom: 28, left: 40 };

/** Round a maximum up to 1/2/5×10ⁿ so the gridlines land on clean numbers. */
export function niceMax(value: number): number {
  if (value <= 4) return 4;
  const magnitude = 10 ** Math.floor(Math.log10(value));
  for (const step of [1, 2, 5, 10]) {
    if (step * magnitude >= value) return step * magnitude;
  }
  return 10 * magnitude;
}

function dayLabel(date: string, withYear = false): string {
  return new Date(`${date}T00:00:00Z`).toLocaleDateString("en-GB", {
    day: "numeric",
    month: "short",
    ...(withYear ? { year: "numeric" } : {}),
    timeZone: "UTC",
  });
}

/**
 * Several counts over the same days, on one shared axis.
 *
 * Dated x-axis, a labelled y-axis, a crosshair and tooltip on hover or arrow
 * keys, a legend that hides and shows series, and the same numbers as a table
 * for anyone who would rather read than trace lines.
 */
export function TimeSeriesChart({
  data,
  series,
  caption,
}: {
  data: TimeDatum[];
  series: TimeSeries[];
  caption: string;
}) {
  const [ref, width] = useWidth<HTMLDivElement>(720);
  const [hidden, setHidden] = useState<Set<string>>(new Set());
  const [active, setActive] = useState<number | null>(null);
  const [showTable, setShowTable] = useState(false);
  const tableId = useId();

  const visible = series.filter((s) => !hidden.has(s.key));
  const plotW = Math.max(120, width - PAD.left - PAD.right);
  const plotH = HEIGHT - PAD.top - PAD.bottom;

  const max = useMemo(() => {
    let top = 0;
    for (const d of data) for (const s of visible) top = Math.max(top, Number(d[s.key]) || 0);
    return niceMax(top);
  }, [data, visible]);

  const x = (i: number) =>
    PAD.left + (data.length <= 1 ? plotW / 2 : (i / (data.length - 1)) * plotW);
  const y = (v: number) => PAD.top + plotH - (v / max) * plotH;

  const yTicks = [0, 0.25, 0.5, 0.75, 1].map((f) => Math.round(max * f));
  const xTickCount = Math.max(2, Math.min(data.length, Math.floor(plotW / 90)));
  const xTicks =
    data.length <= 1
      ? [0]
      : Array.from({ length: xTickCount }, (_, k) =>
          Math.round((k / (xTickCount - 1)) * (data.length - 1)),
        ).filter((v, k, all) => all.indexOf(v) === k);
  const spansYears =
    data.length > 0 && data[0].date.slice(0, 4) !== data[data.length - 1].date.slice(0, 4);

  function toggle(key: string) {
    setHidden((current) => {
      const next = new Set(current);
      if (next.has(key)) next.delete(key);
      // Never hide the last visible series: an empty plot looks like no data.
      else if (series.length - next.size > 1) next.add(key);
      return next;
    });
  }

  function indexAt(clientX: number, rect: DOMRect): number {
    const px = ((clientX - rect.left) / rect.width) * width;
    const ratio = (px - PAD.left) / plotW;
    return Math.max(0, Math.min(data.length - 1, Math.round(ratio * (data.length - 1))));
  }

  function onKey(e: React.KeyboardEvent) {
    if (data.length === 0) return;
    if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
      e.preventDefault();
      const step = e.key === "ArrowRight" ? 1 : -1;
      setActive((i) => Math.max(0, Math.min(data.length - 1, (i ?? (step > 0 ? -1 : data.length)) + step)));
    } else if (e.key === "Escape") {
      setActive(null);
    }
  }

  const point = active === null ? null : data[active];
  const tooltipLeft = active === null ? 0 : x(active);
  const flip = tooltipLeft > width * 0.6;

  return (
    <figure className="m-0 flex flex-col gap-3">
      <figcaption className="sr-only">{caption}</figcaption>

      <ul className="flex flex-wrap gap-2" aria-label="Series">
        {series.map((s) => {
          const on = !hidden.has(s.key);
          return (
            <li key={s.key}>
              <button
                type="button"
                aria-pressed={on}
                onClick={() => toggle(s.key)}
                className={cn(
                  "focus-neon inline-flex min-h-9 cursor-pointer items-center gap-2 rounded-lg border px-3 text-xs transition-colors duration-200",
                  on
                    ? "border-glass-edge text-ink"
                    : "border-dashed border-glass-edge text-ink-faint line-through",
                )}
              >
                <span
                  aria-hidden
                  className="h-0.5 w-4 rounded-full"
                  style={{ background: s.color, opacity: on ? 1 : 0.35 }}
                />
                {s.label}
              </button>
            </li>
          );
        })}
      </ul>

      <div ref={ref} className="relative w-full">
        <svg
          width={width}
          height={HEIGHT}
          role="img"
          aria-label={`${caption}. Use the left and right arrow keys to read each day.`}
          tabIndex={0}
          onKeyDown={onKey}
          onPointerMove={(e) => setActive(indexAt(e.clientX, e.currentTarget.getBoundingClientRect()))}
          onPointerLeave={() => setActive(null)}
          onBlur={() => setActive(null)}
          className="focus-neon block touch-pan-y rounded-lg"
        >
          {/* Recessive grid and the y-axis labels. */}
          {yTicks.map((t) => (
            <g key={t}>
              <line
                x1={PAD.left}
                x2={PAD.left + plotW}
                y1={y(t)}
                y2={y(t)}
                stroke="var(--color-mark-track)"
                strokeWidth={1}
                strokeDasharray={t === 0 ? undefined : "2 4"}
              />
              <text
                x={PAD.left - 8}
                y={y(t)}
                textAnchor="end"
                dominantBaseline="middle"
                className="fill-ink-faint text-[0.6875rem] tabular-nums"
              >
                {t.toLocaleString("en-US")}
              </text>
            </g>
          ))}

          {/* Dated x-axis. */}
          {xTicks.map((i) => (
            <text
              key={i}
              x={x(i)}
              y={HEIGHT - 8}
              textAnchor={i === 0 ? "start" : i === data.length - 1 ? "end" : "middle"}
              className="fill-ink-faint text-[0.6875rem]"
            >
              {data[i] ? dayLabel(data[i].date, spansYears) : ""}
            </text>
          ))}

          {/* Drawn last-to-first so the first series sits on top. */}
          {[...visible].reverse().map((s) => (
            <polyline
              key={s.key}
              fill="none"
              stroke={s.color}
              strokeWidth={2}
              strokeLinejoin="round"
              strokeLinecap="round"
              points={data.map((d, i) => `${x(i).toFixed(1)},${y(Number(d[s.key]) || 0).toFixed(1)}`).join(" ")}
            />
          ))}

          {point && active !== null ? (
            <g>
              <line
                x1={x(active)}
                x2={x(active)}
                y1={PAD.top}
                y2={PAD.top + plotH}
                stroke="var(--color-ink-faint)"
                strokeWidth={1}
              />
              {visible.map((s) => (
                <circle
                  key={s.key}
                  cx={x(active)}
                  cy={y(Number(point[s.key]) || 0)}
                  r={4}
                  fill={s.color}
                  stroke="var(--color-surface)"
                  strokeWidth={2}
                />
              ))}
            </g>
          ) : null}
        </svg>

        {point && active !== null ? (
          <div
            role="status"
            className="pointer-events-none absolute top-2 z-10 min-w-36 rounded-lg border border-glass-edge bg-surface px-3 py-2 text-xs shadow-lg"
            style={flip ? { right: width - tooltipLeft + 8 } : { left: tooltipLeft + 8 }}
          >
            <p className="mb-1 text-ink">{dayLabel(point.date, true)}</p>
            <ul className="flex flex-col gap-0.5">
              {visible.map((s) => (
                <li key={s.key} className="flex items-center justify-between gap-4">
                  <span className="flex items-center gap-1.5 text-ink-muted">
                    <span aria-hidden className="size-2 rounded-full" style={{ background: s.color }} />
                    {s.label}
                  </span>
                  <span className="font-mono tabular-nums text-ink">
                    {Number(point[s.key] || 0).toLocaleString("en-US")}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </div>

      <button
        type="button"
        aria-expanded={showTable}
        aria-controls={tableId}
        onClick={() => setShowTable((v) => !v)}
        className="focus-neon w-fit cursor-pointer rounded text-xs text-function transition-colors hover:text-ink"
      >
        {showTable ? "Hide table" : "Show as table"}
      </button>
      {showTable ? (
        <div id={tableId} className="max-h-64 overflow-auto rounded-lg border border-glass-edge">
          <table className="w-full text-left text-xs">
            <caption className="sr-only">{caption}</caption>
            <thead className="sticky top-0 bg-surface text-ink-faint">
              <tr>
                <th scope="col" className="px-3 py-2 font-normal">Date</th>
                {series.map((s) => (
                  <th key={s.key} scope="col" className="px-3 py-2 text-right font-normal">
                    {s.label}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {[...data].reverse().map((d) => (
                <tr key={d.date} className="border-t border-glass-edge">
                  <th scope="row" className="px-3 py-1.5 font-normal text-ink-muted">
                    {dayLabel(d.date, true)}
                  </th>
                  {series.map((s) => (
                    <td key={s.key} className="px-3 py-1.5 text-right font-mono tabular-nums text-ink">
                      {Number(d[s.key] || 0).toLocaleString("en-US")}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </figure>
  );
}
