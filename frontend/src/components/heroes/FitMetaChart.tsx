"use client";

import { QuadrantScatter } from "@/components/charts/QuadrantScatter";
import { HeroDot } from "@/components/heroes/HeroDot";
import type { HeroPoolEntry } from "@/lib/types";

/**
 * Your shrunk win-rate delta (x) against meta strength (y), one portrait per
 * hero, sized by games played. Heroes under 3 games have no delta and are
 * left out rather than plotted at a made-up position.
 */
export function FitMetaChart({
  pool,
  onSelect,
}: {
  pool: HeroPoolEntry[];
  onSelect: (heroId: number) => void;
}) {
  const plotted = pool.filter(
    (h): h is HeroPoolEntry & { win_rate_delta: number; meta_strength: number } =>
      h.win_rate_delta !== null && h.meta_strength !== null,
  );
  if (plotted.length === 0) return null;

  const deltas = plotted.map((h) => h.win_rate_delta * 100);
  const reach = Math.max(10, Math.ceil(Math.max(...deltas.map(Math.abs)) * 1.25));
  const most = Math.max(...plotted.map((h) => h.matches));

  return (
    <QuadrantScatter
      xDomain={[-reach, reach]}
      yDomain={[0, 100]}
      xSplit={0}
      ySplit={50}
      xLabel="Your win rate vs your average"
      yLabel="Meta strength"
      quadrants={["Learn these", "Play more", "Drop for now", "Your pocket picks"]}
      selectLabel="Show in pool"
      onSelect={(id) => onSelect(Number(id))}
      points={plotted.map((h) => {
        const delta = h.win_rate_delta * 100;
        return {
          id: String(h.hero_id),
          x: delta,
          y: h.meta_strength,
          size: Math.round(24 + 22 * Math.sqrt(h.matches / most)),
          label: h.hero_name,
          detail: [
            `${delta >= 0 ? "+" : "−"}${Math.abs(Math.round(delta))} pts win rate`,
            `Meta ${Math.round(h.meta_strength)}`,
            `${h.matches} ${h.matches === 1 ? "game" : "games"}`,
          ],
          mark: <HeroDot heroId={h.hero_id} heroName={h.hero_name} />,
        };
      })}
    />
  );
}
