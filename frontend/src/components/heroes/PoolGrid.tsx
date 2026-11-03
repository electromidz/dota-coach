"use client";

import { useState } from "react";

import { Ring } from "@/components/charts/Ring";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { InfoTip } from "@/components/ui/InfoTip";
import { RelativeTime } from "@/components/ui/RelativeTime";
import { Segmented } from "@/components/ui/Segmented";
import { byTier, TIER_BLURB, TIER_CLASS } from "@/lib/hero-intel";
import type { HeroPoolEntry } from "@/lib/types";
import { cn } from "@/lib/utils";

/** Stretch tiles shown before "+N more". */
const STRETCH_PREVIEW = 6;
/** Played within this many days counts as fresh. */
const FRESH_DAYS = 14;

type SortKey = "games" | "win" | "recent";

/**
 * The whole pool as a dense portrait grid, grouped by tier (Signature,
 * Comfort, Risk, Stretch). Stretch is collapsed by default.
 */
export function PoolGrid({
  pool,
  highlight,
}: {
  pool: HeroPoolEntry[];
  /** Hero to scroll to and ring, e.g. picked on the chart. */
  highlight: number | null;
}) {
  const [sort, setSort] = useState<SortKey>("games");
  const [role, setRole] = useState("all");
  const [showStretch, setShowStretch] = useState(false);
  /** Read once, so "fresh" does not change between renders. */
  const [now] = useState(() => Date.now());

  const roles = Array.from(new Set(pool.map((h) => h.role))).filter(
    (r) => r && r !== "Unknown",
  );
  const filtered = pool.filter((h) => role === "all" || h.role === role);
  const sorted = [...filtered].sort((a, b) => {
    if (sort === "win") return b.shrunk_win_rate - a.shrunk_win_rate;
    if (sort === "recent") return b.last_played_at.localeCompare(a.last_played_at);
    return b.matches - a.matches;
  });
  const groups = byTier(sorted);

  return (
    <section className="flex flex-col gap-4" aria-label="Your hero pool">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          Your hero pool
        </h2>
        <div className="flex flex-wrap items-center gap-2">
          {roles.length > 1 ? (
            <select
              aria-label="Played as"
              value={role}
              onChange={(e) => setRole(e.target.value)}
              className="focus-neon min-h-9 cursor-pointer rounded-xl border border-glass-edge bg-surface-2/60 px-2 text-xs text-ink outline-none"
            >
              <option value="all">Any position</option>
              {roles.map((r) => (
                <option key={r} value={r}>
                  Played as {r}
                </option>
              ))}
            </select>
          ) : null}
          <Segmented
            label="Sort heroes"
            value={sort}
            onChange={setSort}
            options={[
              { value: "games", label: "Games" },
              { value: "win", label: "Win rate" },
              { value: "recent", label: "Last played" },
            ]}
          />
        </div>
      </header>

      {groups.map((group) => {
        const collapsed = group.tier === "stretch" && !showStretch;
        const shown = collapsed ? group.heroes.slice(0, STRETCH_PREVIEW) : group.heroes;
        const hidden = group.heroes.length - shown.length;
        return (
          <div key={group.tier} className="flex flex-col gap-2">
            <h3 className="flex items-center gap-2">
              <span
                className={cn(
                  "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
                  TIER_CLASS[group.tier],
                )}
              >
                {group.tier}
              </span>
              <span className="font-mono text-[0.6875rem] text-ink-faint">{group.heroes.length}</span>
              <InfoTip label={`About ${group.tier}`}>{TIER_BLURB[group.tier]}</InfoTip>
            </h3>
            <ul className="grid grid-cols-1 gap-2 min-[420px]:grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
              {shown.map((hero) => (
                <Tile key={hero.hero_id} hero={hero} now={now} highlighted={highlight === hero.hero_id} />
              ))}
              {hidden > 0 ? (
                <li>
                  <button
                    type="button"
                    onClick={() => setShowStretch(true)}
                    className="focus-neon flex h-full min-h-14 w-full cursor-pointer items-center justify-center rounded-xl border border-dashed border-glass-edge text-xs text-function hover:text-ink"
                  >
                    +{hidden} more
                  </button>
                </li>
              ) : null}
            </ul>
          </div>
        );
      })}
    </section>
  );
}

function Tile({
  hero,
  now,
  highlighted,
}: {
  hero: HeroPoolEntry;
  now: number;
  highlighted: boolean;
}) {
  const delta = hero.win_rate_delta;
  const tone = delta === null ? "neutral" : delta >= 0.05 ? "good" : delta <= -0.05 ? "bad" : "you";
  const days = (now - new Date(hero.last_played_at).getTime()) / 86_400_000;
  const fresh = days <= FRESH_DAYS;

  return (
    <li
      id={`hero-${hero.hero_id}`}
      className={cn(
        "glass flex scroll-mt-24 items-center gap-2 rounded-xl p-2 transition-shadow",
        highlighted && "ring-2 ring-function",
      )}
    >
      <HeroPortrait heroId={hero.hero_id} heroName={hero.hero_name} size="sm" className="!w-12 !h-7" />
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-xs text-ink" title={hero.hero_name}>
          {hero.hero_name}
        </span>
        <span className="flex items-center gap-1.5 whitespace-nowrap font-mono text-[0.625rem] tabular-nums text-ink-faint">
          {hero.wins}-{hero.losses}
          <span
            aria-hidden
            className={cn("size-1.5 rounded-full", fresh ? "bg-mark-win" : "bg-mark-track")}
          />
          <RelativeTime iso={hero.last_played_at} />
        </span>
      </div>
      <Ring
        size={32}
        share={hero.win_rate}
        text={`${Math.round(hero.win_rate * 100)}`}
        tone={tone}
        label={`${hero.hero_name}: ${Math.round(hero.win_rate * 100)}% won over ${hero.matches} games${
          hero.role && hero.role !== "Unknown" ? `, played as ${hero.role}` : ""
        }`}
      />
    </li>
  );
}
