import { Card } from "@/components/ui/Card";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { byTier, TIER_BLURB, TIER_CLASS } from "@/lib/hero-intel";
import { formatPercent } from "@/lib/stats";
import type { HeroPoolEntry, PoolSummary } from "@/lib/types";
import { cn, timeAgo } from "@/lib/utils";

/**
 * The player's repertoire, grouped by what each hero is *to them*.
 *
 * Grouped rather than sorted by win rate, because the question the pool
 * answers is "what can I rely on?" — and a 100% win rate over two games
 * belongs under Stretch, not at the top of the list. Every row carries its
 * match count for the same reason.
 */
export function HeroPool({
  pool,
  summary,
  recentWindow,
}: {
  pool: HeroPoolEntry[];
  summary: PoolSummary;
  recentWindow: number;
}) {
  const groups = byTier(pool);

  return (
    <section className="flex flex-col gap-4">
      <header className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          Your hero pool
        </h2>
        <p className="font-mono text-xs tabular-nums text-ink-faint">
          {summary.heroes} {summary.heroes === 1 ? "hero" : "heroes"} ·{" "}
          {summary.established} with enough games to judge
        </p>
      </header>

      {groups.map((group) => (
        <Card key={group.tier} className="flex flex-col gap-4">
          <div className="flex flex-wrap items-center gap-2">
            <span
              className={cn(
                "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
                TIER_CLASS[group.tier],
              )}
            >
              {group.tier}
            </span>
            <p className="text-xs text-ink-faint">{TIER_BLURB[group.tier]}</p>
          </div>

          <ul className="m-0 flex list-none flex-col gap-3 p-0">
            {group.heroes.map((hero) => (
              <li key={hero.hero_id} className="flex items-center gap-3">
                <HeroPortrait
                  heroId={hero.hero_id}
                  heroName={hero.hero_name}
                  size="sm"
                />

                <div className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate text-sm text-ink">
                    {hero.hero_name}
                  </span>
                  <span className="text-[0.6875rem] text-ink-faint">
                    {hero.role} · last played {timeAgo(hero.last_played_at)}
                  </span>
                </div>

                <div className="flex shrink-0 flex-col items-end font-mono text-xs tabular-nums">
                  <span className="text-ink">
                    {formatPercent(hero.win_rate)}
                    <span className="ml-1.5 text-ink-faint">
                      {hero.wins}-{hero.losses}
                    </span>
                  </span>
                  <span className="text-[0.6875rem] text-ink-faint">
                    {/* Recent form is per hero, so the denominator is stated. */}
                    last {Math.min(hero.recent_matches, recentWindow)}:{" "}
                    {formatPercent(hero.recent_win_rate)}
                  </span>
                </div>
              </li>
            ))}
          </ul>
        </Card>
      ))}
    </section>
  );
}
