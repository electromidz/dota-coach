"use client";

import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { Icon } from "@/components/ui/Icon";
import type { MatchView } from "@/lib/types";
import { cn, formatDuration } from "@/lib/utils";

/** OpenDota's `game_mode` for Turbo. The badge is the one mode players ask about. */
const TURBO = 23;

/** Where the 1–10 rating changes colour. */
const POOR_BELOW = 4;
const STRONG_ABOVE = 7;

/**
 * A fixed locale, not the reader's.
 *
 * This renders on the server and again in the browser, and a date formatted with
 * two different locales is a hydration mismatch. The format is short and
 * unambiguous either way: `Sep 24, 26`.
 */
const DATE = new Intl.DateTimeFormat("en-US", {
  month: "short",
  day: "numeric",
  year: "2-digit",
});

/** One match, as a row of the rank tab's history table. */
export function MatchHistoryRow({ match }: { match: MatchView }) {
  const turbo = match.game_mode === TURBO;

  return (
    <tr className="border-t border-glass-edge transition-colors duration-150 ease-out hover:bg-surface-2/50">
      <td className="py-2 pl-1 pr-3">
        <div className="flex items-center gap-2.5">
          <HeroPortrait
            heroId={match.hero_id}
            heroName={match.hero_name}
            size="sm"
          />
          <div className="min-w-0">
            <p className="flex items-center gap-1.5">
              <span className="truncate font-display text-xs tracking-wide">
                {match.hero_name}
              </span>
              {turbo ? (
                <span className="shrink-0 rounded bg-surface-2 px-1 py-0.5 text-[0.5625rem] uppercase tracking-wider text-ink-faint">
                  Turbo
                </span>
              ) : null}
            </p>
            <p className="font-mono text-[0.625rem] tabular-nums text-ink-faint">
              {DATE.format(new Date(match.started_at))}
            </p>
          </div>
        </div>
      </td>

      <td className="px-3">
        <span
          className={cn(
            "rounded px-1.5 py-0.5 text-[0.625rem] font-semibold uppercase tracking-wider",
            match.won
              ? "bg-mark-win/15 text-string"
              : "bg-mark-loss/15 text-error",
          )}
        >
          {match.won ? "Win" : "Loss"}
        </span>
      </td>

      <td className="px-3 font-mono text-xs tabular-nums text-ink">
        {match.kills}
        <span className="text-ink-faint"> / </span>
        {/* Deaths in red: the one figure in the row a player is looking for. */}
        <span className="text-error">{match.deaths}</span>
        <span className="text-ink-faint"> / </span>
        {match.assists}
      </td>

      <Numeric>{match.gpm.toLocaleString("en-US")}</Numeric>
      <Numeric className="hidden md:table-cell">
        {match.xpm.toLocaleString("en-US")}
      </Numeric>
      <Numeric>{formatDuration(match.duration_seconds)}</Numeric>

      <td className="px-3 py-2">
        <Rating value={match.rating} />
      </td>

      <td className="px-3">
        <MmrDelta value={match.mmr_delta_estimate} />
      </td>

      <td className="hidden px-3 md:table-cell">
        <a
          href={`https://www.opendota.com/matches/${match.match_id}`}
          target="_blank"
          rel="noopener noreferrer"
          className="focus-neon inline-flex items-center gap-1 font-mono text-[0.625rem] tabular-nums text-ink-faint transition-colors duration-150 hover:text-function"
        >
          {match.match_id}
          <Icon name="external" className="size-3" />
        </a>
      </td>
    </tr>
  );
}

function Numeric({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <td
      className={cn(
        "px-3 font-mono text-xs tabular-nums text-ink-muted",
        className,
      )}
    >
      {children}
    </td>
  );
}

/**
 * The 1–10 rating, with the same value as a row of five stars.
 *
 * The number is the figure; the stars are how it reads at a glance down a
 * column. One partially filled star rather than rounding to a half, because the
 * rating carries a decimal and rounding it twice would put a 7.4 and a 7.6 on
 * different star counts while showing neighbouring numbers.
 */
export function Rating({ value }: { value: number }) {
  const pct = Math.max(0, Math.min(1, value / 10)) * 100;

  const tone =
    value < POOR_BELOW
      ? "text-bad"
      : value > STRONG_ABOVE
        ? "text-good"
        : "text-warn";

  return (
    <div className="flex items-center gap-2">
      <span className={cn("font-mono text-xs tabular-nums", tone)}>
        {value.toFixed(1)}
      </span>

      {/* Outline stars underneath, the same stars clipped to the rating on top.
          `aria-hidden` because the number beside them says it already. */}
      <span aria-hidden className="relative hidden shrink-0 sm:block">
        <Stars className="text-mark-track" />
        <span
          className="absolute inset-y-0 left-0 overflow-hidden"
          style={{ width: `${pct}%` }}
        >
          <Stars className={cn("fill-current", tone)} />
        </span>
      </span>
    </div>
  );
}

function Stars({ className }: { className: string }) {
  return (
    <span className={cn("flex", className)}>
      {[0, 1, 2, 3, 4].map((i) => (
        <Icon key={i} name="star" className="size-3 shrink-0" />
      ))}
    </span>
  );
}

/**
 * Estimated ladder movement.
 *
 * Labelled `est.` without exception. Valve publishes no per-match MMR, so this
 * is a disclosed model and a bare `+27` would read as their number. A game that
 * cannot move a medal shows no movement at all rather than a zero a reader would
 * take for "you gained nothing".
 */
export function MmrDelta({ value }: { value: number | null }) {
  if (value === null) {
    return (
      <span
        title="Only ranked matches move a medal."
        className="font-mono text-xs tabular-nums text-ink-faint"
      >
        —
      </span>
    );
  }

  const tone =
    value > 0 ? "text-good" : value < 0 ? "text-bad" : "text-ink-faint";

  return (
    <span
      title="Estimated. Valve publishes no per-match MMR, so this is modeled from your results — the same model as the momentum curve above."
      className={cn("font-mono text-xs tabular-nums", tone)}
    >
      {value > 0 ? `+${value}` : value}
      <span className="ml-1 text-[0.5625rem] text-ink-faint">est.</span>
    </span>
  );
}
