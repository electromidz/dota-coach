"use client";

import { useState } from "react";

import { InfoTip } from "@/components/ui/InfoTip";
import { ScopeLine } from "@/components/ui/ScopeLine";
import { heroPortraitUrl } from "@/lib/heroes";
import { niceTicks, streakRuns } from "@/lib/rank-view";
import type { Momentum, MomentumPoint } from "@/lib/types";
import { useWidth } from "@/lib/useWidth";
import { cn, shortDate } from "@/lib/utils";

/** Fewer ranked games than this and the chart is not drawn. */
export const MIN_MOMENTUM_GAMES = 5;
/** Below this width the axis portraits are hidden; every bar stays. */
const NARROW = 480;

const PAD_L = 44;
const PAD_R = 44;
const PAD_T = 14;

/**
 * Recent momentum: one bar per ranked game (win up, loss down) and the
 * running total as a line, all modeled MMR on one axis.
 *
 * Every magnitude is the server's disclosed estimate; the win/loss sequence
 * is real. Values appear only in the tooltip. Click a bar to open the match.
 */
export function MomentumCard({ momentum }: { momentum: Momentum }) {
  const games = momentum.points.length;
  const sign = (v: number) => (v >= 0 ? "+" : "−");
  const fmt = (v: number) => `${sign(v)}${Math.abs(Math.round(v))}`;

  return (
    <section aria-label="Recent momentum" className="glass flex min-w-0 flex-col gap-3 rounded-card p-4 sm:p-5">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.18em] text-ink-muted">
          Recent momentum
          <InfoTip label="About recent momentum">
            Your last {momentum.window} ranked games, oldest on the left. Bars are each
            game&apos;s modeled MMR change, the line the running total. Estimates, not
            Valve&apos;s numbers. Click a bar to open the match.
          </InfoTip>
        </h2>
        {games > 0 ? (
          <ul className="flex flex-wrap gap-1.5 font-mono text-[0.6875rem] tabular-nums">
            <Chip>
              {momentum.wins}W – {momentum.losses}L
            </Chip>
            <Chip tone={momentum.net >= 0 ? "good" : "bad"}>{fmt(momentum.net)} est.</Chip>
            {momentum.avg_win_delta !== null || momentum.avg_loss_delta !== null ? (
              <Chip>
                Avg {momentum.avg_win_delta !== null ? fmt(momentum.avg_win_delta) : "—"} /{" "}
                {momentum.avg_loss_delta !== null ? fmt(momentum.avg_loss_delta) : "—"}
              </Chip>
            ) : null}
          </ul>
        ) : null}
      </header>

      <ScopeLine mode="Ranked" matches={games} className="-mt-1" />

      {games < MIN_MOMENTUM_GAMES ? (
        <p className="text-sm text-ink-faint">
          {games === 0 ? "No ranked games yet." : `${games} ranked ${games === 1 ? "game" : "games"} so far.`} The
          chart starts at {MIN_MOMENTUM_GAMES}.
        </p>
      ) : (
        <MomentumChart points={momentum.points} />
      )}
    </section>
  );
}

function Chip({ tone, children }: { tone?: "good" | "bad"; children: React.ReactNode }) {
  return (
    <li
      className={cn(
        "rounded-full bg-surface-2 px-2 py-0.5",
        tone === "good" ? "text-string" : tone === "bad" ? "text-error" : "text-ink-muted",
      )}
    >
      {children}
    </li>
  );
}

export function MomentumChart({ points }: { points: MomentumPoint[] }) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const [hover, setHover] = useState<number | null>(null);

  const narrow = width < NARROW;
  const height = narrow ? 180 : 220;
  // Room under the plot for portraits (wide) and dates.
  const padB = narrow ? 22 : 44;
  const plotW = Math.max(1, width - PAD_L - PAD_R);
  const plotH = height - PAD_T - padB;

  const values = points.flatMap((p) => [p.delta, p.cumulative, 0]);
  const ticks = niceTicks(Math.min(...values), Math.max(...values));
  const lo = ticks[0];
  const hi = ticks[ticks.length - 1];

  const n = points.length;
  const slot = plotW / n;
  const barW = Math.max(3, Math.min(18, slot * 0.6));
  const cx = (i: number) => PAD_L + slot * (i + 0.5);
  const y = (v: number) => PAD_T + plotH - ((v - lo) / (hi - lo || 1)) * plotH;
  const zero = y(0);

  const line = points.map((p, i) => `${cx(i).toFixed(1)},${y(p.cumulative).toFixed(1)}`).join(" ");
  const area = `${cx(0)},${zero} ${line} ${cx(n - 1)},${zero}`;
  const last = points[n - 1];
  const runs = streakRuns(points.map((p) => p.won));

  const active = hover !== null ? points[hover] : null;

  return (
    <div ref={ref} className="relative w-full">
      <svg
        width={width}
        height={height}
        viewBox={`0 0 ${width} ${height}`}
        style={{ maxWidth: "100%", height: "auto" }}
        role="img"
        aria-label={`Modeled MMR over ${n} ranked games: ${points.filter((p) => p.won).length} won, ending ${last.cumulative >= 0 ? "+" : "−"}${Math.abs(Math.round(last.cumulative))} estimated.`}
        className="block overflow-visible"
        onPointerLeave={() => setHover(null)}
      >
        {/* Streaks of 3+, behind everything. */}
        {runs.map((r) => (
          <rect
            key={r.start}
            x={PAD_L + slot * r.start}
            y={PAD_T}
            width={slot * (r.end - r.start + 1)}
            height={plotH}
            fill={r.won ? "var(--color-mark-win)" : "var(--color-mark-loss)"}
            fillOpacity={0.07}
            rx={4}
          />
        ))}

        {ticks.map((t) => (
          <g key={t}>
            {t !== 0 ? (
              <line x1={PAD_L} x2={PAD_L + plotW} y1={y(t)} y2={y(t)} stroke="var(--color-mark-track)" strokeOpacity={0.5} />
            ) : null}
            <text x={PAD_L - 6} y={y(t) + 4} textAnchor="end" className="fill-ink-faint font-mono text-[10px]">
              {t > 0 ? `+${t}` : t < 0 ? `−${Math.abs(t)}` : "0"}
            </text>
          </g>
        ))}
        <text
          transform={`translate(10 ${PAD_T + plotH / 2}) rotate(-90)`}
          textAnchor="middle"
          className="fill-ink-faint text-[10px]"
        >
          MMR est.
        </text>
        <line x1={PAD_L} x2={PAD_L + plotW} y1={zero} y2={zero} stroke="var(--color-ink-faint)" strokeDasharray="4 4" />

        {points.map((p, i) => {
          const top = Math.min(y(p.delta), zero);
          const h = Math.max(2, Math.abs(y(p.delta) - zero));
          return (
            <rect
              key={p.id}
              x={cx(i) - barW / 2}
              y={top}
              width={barW}
              height={h}
              rx={2}
              fill={p.won ? "var(--color-mark-win)" : "var(--color-mark-loss)"}
              fillOpacity={hover === null || hover === i ? 1 : 0.55}
            />
          );
        })}

        <polygon points={area} fill="var(--color-you)" fillOpacity={0.1} />
        <polyline points={line} fill="none" stroke="var(--color-you)" strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" />
        <circle cx={cx(n - 1)} cy={y(last.cumulative)} r={4} fill="var(--color-you)" stroke="var(--color-surface-2)" strokeWidth={2} />
        <text x={cx(n - 1) + 8} y={y(last.cumulative) + 4} className="fill-ink font-mono text-[11px]">
          {last.cumulative >= 0 ? "+" : "−"}
          {Math.abs(Math.round(last.cumulative))}
        </text>

        {/* X axis: a portrait under every bar on wide screens, a date on
            every 5th. */}
        {points.map((p, i) => {
          const src = heroPortraitUrl(p.hero_id);
          const showDate = i % 5 === 0 || i === n - 1;
          return (
            <g key={`x-${p.id}`}>
              {!narrow && src ? (
                <image href={src} x={cx(i) - 8} y={PAD_T + plotH + 6} width={16} height={9} preserveAspectRatio="xMidYMid slice" />
              ) : null}
              {showDate ? (
                <text x={cx(i)} y={height - 4} textAnchor="middle" className="fill-ink-faint text-[10px]">
                  {shortDate(p.started_at)}
                </text>
              ) : null}
            </g>
          );
        })}

        {/* Hit targets: the full column per game, each a link to the match. */}
        {points.map((p, i) => (
          <a key={`hit-${p.id}`} href={`/matches/${p.id}`} aria-label={describe(p)}>
            <rect
              x={PAD_L + slot * i}
              y={PAD_T}
              width={slot}
              height={plotH + padB}
              fill="transparent"
              onPointerEnter={() => setHover(i)}
              onFocus={() => setHover(i)}
            />
          </a>
        ))}
      </svg>

      {active && hover !== null ? (
        <div
          role="status"
          className={cn(
            "pointer-events-none absolute top-0 z-10 rounded-lg border border-glass-edge bg-surface-2 px-2.5 py-1.5 text-xs shadow-lg",
            cx(hover) > width / 2 ? "-translate-x-full" : "",
          )}
          style={{ left: cx(hover) + (cx(hover) > width / 2 ? -10 : 10) }}
        >
          <p className="font-semibold text-ink">
            {active.hero_name} · <span className={active.won ? "text-string" : "text-error"}>{active.won ? "Won" : "Lost"}</span>
          </p>
          <p className="font-mono tabular-nums text-ink-muted">
            {active.kills}/{active.deaths}/{active.assists}
            {active.rating !== null ? ` · rating ${active.rating.toFixed(1)}` : ""}
          </p>
          <p className="font-mono tabular-nums text-ink-muted">
            {active.delta >= 0 ? "+" : "−"}
            {Math.abs(Math.round(active.delta))} est. · {shortDate(active.started_at)}
          </p>
        </div>
      ) : null}
    </div>
  );
}

function describe(p: MomentumPoint): string {
  const delta = `${p.delta >= 0 ? "+" : "minus "}${Math.abs(Math.round(p.delta))}`;
  return `${p.hero_name}, ${p.won ? "won" : "lost"}, ${p.kills}/${p.deaths}/${p.assists}, ${delta} estimated, ${shortDate(p.started_at)}`;
}
