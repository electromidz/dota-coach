import { Ring } from "@/components/charts/Ring";
import { WinLossStrip } from "@/components/charts/WinLossStrip";
import { FitBreakdown } from "@/components/heroes/FitBreakdown";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { InfoTip } from "@/components/ui/InfoTip";
import { SampleBadge } from "@/components/ui/SampleBadge";
import { TIER_CLASS } from "@/lib/hero-intel";
import type { FitPart, HeroFit, HeroPoolEntry } from "@/lib/types";
import { cn } from "@/lib/utils";

/** Short names for the fit components, for the mini bar. */
const SHORT: Record<string, string> = {
  player_performance: "Your results",
  meta_strength: "Meta",
  experience: "Experience",
  benchmark: "Benchmark",
  recent_form: "Recent form",
};

/**
 * Heroes with 5+ games, best fit first, as a horizontal row of compact cards:
 * portrait, fit ring, tier, games, what builds the score, and the last games.
 */
export function RecommendedRow({
  fits,
  pool,
}: {
  fits: HeroFit[];
  pool: HeroPoolEntry[];
}) {
  const byId = new Map(pool.map((h) => [h.hero_id, h]));
  return (
    <ul className="-mx-1 flex snap-x gap-3 overflow-x-auto px-1 pb-2">
      {fits.map((fit) => {
        const hero = byId.get(fit.hero_id);
        return (
          <li
            key={fit.hero_id}
            className="glass flex w-60 shrink-0 snap-start flex-col gap-3 rounded-card p-3"
          >
            <div className="flex items-start gap-2.5">
              <HeroPortrait heroId={fit.hero_id} heroName={fit.hero_name} size="sm" />
              <div className="flex min-w-0 flex-1 flex-col gap-1">
                <span className="truncate text-sm text-ink">{fit.hero_name}</span>
                {fit.tier ? (
                  <span
                    className={cn(
                      "w-fit rounded-full border px-1.5 text-[0.5625rem] uppercase tracking-wider",
                      TIER_CLASS[fit.tier],
                    )}
                  >
                    {fit.tier}
                  </span>
                ) : null}
              </div>
              <div className="flex shrink-0 flex-col items-center gap-0.5">
                <Ring
                  share={fit.fit_score / 100}
                  text={String(Math.round(fit.fit_score))}
                  label={`Fit score ${Math.round(fit.fit_score)} of 100`}
                />
                <span className="flex items-center gap-1 whitespace-nowrap text-[0.625rem] text-ink-faint">
                  {fit.matches} games
                  {hero ? <SampleBadge sample={fit.matches} confidence={hero.confidence} /> : null}
                </span>
              </div>
            </div>

            <div className="flex flex-col gap-1">
              <FitBar parts={fit.parts} />
              <FitLegend parts={fit.parts} />
            </div>

            <div className="flex items-center justify-between gap-2">
              {hero && hero.recent_results.length > 0 ? (
                <WinLossStrip results={hero.recent_results} label={`${fit.hero_name}, last games`} />
              ) : (
                <span />
              )}
              <span className="flex items-center gap-1 text-[0.6875rem] text-ink-faint">
                Why this score?
                <InfoTip label={`Why ${fit.hero_name} scores ${Math.round(fit.fit_score)}`} align="end">
                  <FitBreakdown parts={fit.parts} />
                </InfoTip>
              </span>
            </div>
          </li>
        );
      })}
    </ul>
  );
}

/** What builds the score: each part's contribution (score x weight). */
function FitBar({ parts }: { parts: FitPart[] }) {
  const contributions = parts.map((p) => ({ part: p, value: p.score * p.weight }));
  const total = contributions.reduce((sum, c) => sum + c.value, 0) || 1;
  const steps = [1, 0.7, 0.5, 0.34, 0.22];
  return (
    <div
      role="img"
      aria-label={`Score built from ${contributions
        .map((c) => `${c.part.label} ${Math.round(c.value)}`)
        .join(", ")}`}
      className="flex h-2 w-full gap-0.5 overflow-hidden rounded-full"
    >
      {contributions.map((c, i) => (
        <span
          key={c.part.component}
          title={`${SHORT[c.part.component] ?? c.part.label}: ${Math.round(c.value)}`}
          className="h-full bg-you"
          style={{ width: `${(c.value / total) * 100}%`, opacity: steps[i] ?? 0.2 }}
        />
      ))}
    </div>
  );
}

/** One line naming the bar's segments in order, so shade is never the key. */
function FitLegend({ parts }: { parts: FitPart[] }) {
  const steps = [1, 0.7, 0.5, 0.34, 0.22];
  return (
    <p className="flex flex-wrap gap-x-2 truncate text-[0.625rem] text-ink-faint">
      {parts.map((p, i) => (
        <span key={p.component} className="inline-flex items-center gap-1">
          <span aria-hidden className="size-1.5 rounded-sm bg-you" style={{ opacity: steps[i] ?? 0.2 }} />
          {SHORT[p.component] ?? p.label}
        </span>
      ))}
    </p>
  );
}
