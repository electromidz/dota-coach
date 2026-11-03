import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { InfoTip } from "@/components/ui/InfoTip";
import { formatPercent } from "@/lib/stats";
import type { HeroMeta, MetaContext } from "@/lib/types";
import { cn } from "@/lib/utils";

/** Meta strength, strongest first; heroes in your pool stand out. */
export function MetaBars({
  leaders,
  meta,
  inPool,
}: {
  leaders: HeroMeta[];
  meta: MetaContext;
  inPool: Set<number>;
}) {
  const top = leaders.slice(0, 10);
  return (
    <section className="flex flex-col gap-3" aria-label="Current meta">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="flex items-center gap-2 font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          Current meta
          <InfoTip label="What meta strength means">
            <span>
              0-100: win rate against the roster, how contested the hero is, and its
              recent direction. Small samples are pulled toward the middle.
            </span>
            <span className="text-ink-faint">
              {meta.source ? `Source: ${meta.source}. ` : ""}
              {meta.bracket_label ? `Bracket: ${meta.bracket_label}.` : "All brackets."}
            </span>
          </InfoTip>
        </h2>
        <span className="flex items-center gap-3 text-[0.6875rem] text-ink-faint">
          <span className="flex items-center gap-1.5">
            <span aria-hidden className="h-2 w-3 rounded-sm bg-you" /> In your pool
          </span>
          <span className="flex items-center gap-1.5">
            <span aria-hidden className="h-2 w-3 rounded-sm bg-rank/60" /> Not played
          </span>
        </span>
      </header>
      <ul className="glass flex flex-col gap-2 rounded-card p-4">
        {top.map((hero) => {
          const mine = inPool.has(hero.hero_id);
          return (
            <li
              key={hero.hero_id}
              tabIndex={0}
              aria-label={`${hero.hero_name}: meta ${Math.round(hero.meta_strength)}, ${formatPercent(hero.win_rate)} win, ${formatPercent(hero.pick_rate)} pick${mine ? ", in your pool" : ""}`}
              className="focus-neon group relative grid grid-cols-[2.25rem_6.5rem_1fr_2rem] items-center gap-2 rounded sm:grid-cols-[2.25rem_9rem_1fr_2rem]"
            >
              <HeroPortrait heroId={hero.hero_id} heroName={hero.hero_name} size="sm" className="!w-9 !h-5" />
              <span className={cn("truncate text-xs", mine ? "text-ink" : "text-ink-muted")}>
                {hero.hero_name}
              </span>
              <span className="h-2.5 overflow-hidden rounded-full bg-mark-track">
                <span
                  className={cn("block h-full rounded-full", mine ? "bg-you" : "bg-rank/60")}
                  style={{ width: `${Math.max(0, Math.min(100, hero.meta_strength))}%` }}
                />
              </span>
              <span className="text-right font-mono text-xs tabular-nums text-ink">
                {Math.round(hero.meta_strength)}
              </span>
              <span
                aria-hidden
                className="pointer-events-none absolute -top-7 right-8 z-10 hidden whitespace-nowrap rounded-lg border border-glass-edge bg-surface px-2 py-1 font-mono text-[0.6875rem] text-ink shadow-lg group-hover:block group-focus:block"
              >
                {formatPercent(hero.win_rate)} win · {formatPercent(hero.pick_rate)} pick
              </span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
