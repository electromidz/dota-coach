import Link from "next/link";

import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { TiltCard } from "@/components/ui/TiltCard";
import type { MatchesMode, MatchView } from "@/lib/types";
import { cn, formatDuration } from "@/lib/utils";
import { RelativeTime } from "@/components/ui/RelativeTime";

/** Rough ceilings for the inline bars, so a good game visibly fills them. */

/**
 * A match, in enough detail to judge it without opening it: hero portrait,
 * result, role, KDA, and where GPM/XPM sat relative to a strong game.
 *
 * The whole card is one link and one tap target. It tilts in 3D toward the
 * press, and the portrait sits forward on the Z axis so the depth is real.
 */
export function MatchCard({
  match,
  index = 0,
  mode = "eligible",
}: {
  match: MatchView;
  index?: number;
  /** In the default view every game is coached, so nothing is labelled. */
  mode?: MatchesMode;
}) {
  const won = match.won;

  return (
    <li
      className="min-w-0 rise-in"
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
                <span className="text-operator">{match.position_label}</span>
                {" · "}
                <span className="font-mono tabular-nums">
                  {formatDuration(match.duration_seconds)}
                </span>
                {" · "}
                <RelativeTime iso={match.started_at} />
              </p>

              {/* Only outside the default view, and only on games the coach
                  does not read. Eligibility is the server's; never recomputed. */}
              {mode !== "eligible" && !match.eligible ? (
                <p className="mt-1">
                  <span
                    title={match.mode_label}
                    className="rounded bg-surface-2 px-1.5 py-0.5 text-[0.625rem] uppercase tracking-wider text-ink-faint"
                  >
                    Not coached
                  </span>
                </p>
              ) : null}
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
            <PeerBar label="GPM" value={match.gpm} peer={match.peer_gpm} hero={match.hero_name} />
            <PeerBar label="XPM" value={match.xpm} peer={match.peer_xpm} hero={match.hero_name} />
          </div>
        </Link>
      </TiltCard>
    </li>
  );
}

/**
 * A value against the peer median for this hero, never against the rest of
 * the list: one Turbo game must not stretch every other bar. The track runs
 * to 1.5x the median, with a tick at the median; green at or above it, red
 * below. No peer figure (Turbo, provider down): the number alone.
 */
export function PeerBar({
  label,
  value,
  peer,
  hero,
}: {
  label: string;
  value: number;
  peer: number | null;
  hero: string;
}) {
  const ceiling = peer ? peer * 1.5 : null;
  const at = (v: number) => `${Math.max(0, Math.min(1, v / (ceiling ?? 1))) * 100}%`;
  const ahead = peer !== null && value >= peer;

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-[0.625rem] uppercase tracking-wider text-ink-faint">{label}</span>
        <span className="font-mono text-xs tabular-nums text-ink-muted">
          {value.toLocaleString()}
        </span>
      </div>
      {peer !== null ? (
        <div
          role="img"
          aria-label={`${label} ${value}, peer median on ${hero} ${Math.round(peer)}`}
          className="relative h-1.5 w-full rounded-full bg-mark-track"
        >
          <div
            className={cn("h-full rounded-full", ahead ? "bg-mark-win" : "bg-mark-loss")}
            style={{ width: at(value) }}
          />
          <span
            aria-hidden
            className="absolute -inset-y-0.5 w-0.5 rounded bg-ink-faint"
            style={{ left: at(peer) }}
          />
        </div>
      ) : (
        <div className="h-1.5" title="No peer reference for this game" />
      )}
    </div>
  );
}
