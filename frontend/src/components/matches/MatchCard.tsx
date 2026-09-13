import Link from "next/link";

import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { TiltCard } from "@/components/ui/TiltCard";
import type { Match } from "@/lib/types";
import { cn, formatDuration, timeAgo } from "@/lib/utils";

/** Rough ceilings for the inline bars, so a good game visibly fills them. */
const GPM_CEILING = 900;
const XPM_CEILING = 1000;

/**
 * A match, in enough detail to judge it without opening it: hero portrait,
 * result, role, KDA, and where GPM/XPM sat relative to a strong game.
 *
 * The whole card is one link and one tap target. It tilts in 3D toward the
 * press, and the portrait sits forward on the Z axis so the depth is real.
 */
export function MatchCard({ match, index = 0 }: { match: Match; index?: number }) {
  const won = match.won;

  return (
    <li
      className="rise-in"
      // Small stagger down the list; capped so a long page never crawls.
      style={{ animationDelay: `${Math.min(index, 8) * 45}ms` }}
    >
      <TiltCard>
        <Link
          href={`/matches/${match.id}`}
          className={cn(
            "glass soft-raised focus-neon block cursor-pointer rounded-card p-3.5",
            "transition-[border-color,box-shadow] duration-200 ease-out",
            won ? "hover:border-string/40" : "hover:border-error/40",
          )}
        >
          <div className="flex items-center gap-3">
            <div className="pop-3d">
              <HeroPortrait heroId={match.hero_id} heroName={match.hero_name} />
            </div>

            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="truncate font-display text-sm tracking-wide">
                  {match.hero_name}
                </span>
                <span
                  className={cn(
                    "shrink-0 rounded px-1.5 py-0.5 text-[0.625rem] font-semibold uppercase tracking-wider",
                    won
                      ? "bg-mark-win/15 text-string"
                      : "bg-mark-loss/15 text-error",
                  )}
                >
                  {won ? "Win" : "Loss"}
                </span>
              </div>

              <p className="mt-0.5 truncate text-xs text-ink-muted">
                <span className="text-operator">{match.role}</span>
                {" · "}
                <span className="font-mono tabular-nums">
                  {formatDuration(match.duration_seconds)}
                </span>
                {" · "}
                {timeAgo(match.started_at)}
              </p>
            </div>

            <div className="shrink-0 text-right">
              <p className="font-mono text-sm tabular-nums text-number">
                {match.kills}/{match.deaths}/{match.assists}
              </p>
              <p className="text-[0.6875rem] text-ink-faint">
                {match.kda?.toFixed(1) ?? "—"} KDA
              </p>
            </div>
          </div>

          {/* Economy at a glance. Two independent measures, so two separate
              tracks rather than one chart with two scales. */}
          <div className="mt-3 grid grid-cols-2 gap-3 border-t border-glass-edge pt-3">
            <MiniBar label="GPM" value={match.gpm} ceiling={GPM_CEILING} />
            <MiniBar label="XPM" value={match.xpm} ceiling={XPM_CEILING} />
          </div>
        </Link>
      </TiltCard>
    </li>
  );
}

function MiniBar({
  label,
  value,
  ceiling,
}: {
  label: string;
  value: number;
  ceiling: number;
}) {
  const pct = Math.max(0, Math.min(1, value / ceiling)) * 100;

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-[0.625rem] uppercase tracking-wider text-ink-faint">
          {label}
        </span>
        <span className="font-mono text-xs tabular-nums text-ink-muted">
          {value.toLocaleString()}
        </span>
      </div>
      <div className="h-1.5 w-full overflow-hidden rounded-full bg-mark-track">
        <div
          className="h-full rounded-r bg-function/70"
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}
