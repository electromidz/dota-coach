import { Reveal } from "@/components/landing/Reveal";
import { BarList } from "@/components/charts/BarList";
import { Card } from "@/components/ui/Card";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { Icon, type IconName } from "@/components/ui/Icon";

const PHASE_MISTAKES = [
  { label: "Laning (0–10 min)", value: 9, meta: "37%" },
  { label: "Midgame (10–25 min)", value: 6, meta: "25%" },
  { label: "Late (25 min+)", value: 9, meta: "38%" },
];

const RECOMMENDATIONS = [
  { heroId: 106, heroName: "Ember Spirit", fit: 94 },
  { heroId: 126, heroName: "Void Spirit", fit: 88 },
];

const FLAGS = [
  { time: "12:40", label: "Missed a rune swing on cooldown" },
  { time: "24:10", label: "Died to a rotation you had vision on" },
  { time: "31:55", label: "Bought BKB two full minutes late" },
];

const SMALL_TILES: { icon: IconName; title: string; body: string }[] = [
  {
    icon: "refresh",
    title: "Patch-aware advice",
    body: "Recommendations shift with every balance update — never last patch's meta.",
  },
  {
    icon: "swords",
    title: "Laning analysis",
    body: "Last hits, denies and trades at 10 minutes, benchmarked against your bracket.",
  },
  {
    icon: "clock",
    title: "Item timing benchmarks",
    body: "See exactly how far behind (or ahead) your core timings really are.",
  },
  {
    icon: "gauge",
    title: "Progress tracking",
    body: "Your training focus, charted match over match, not just at the start and end.",
  },
];

export function FeaturesBento() {
  return (
    <section id="features" className="safe-x mx-auto max-w-7xl py-16 lg:py-24">
      <Reveal className="mx-auto max-w-2xl text-center">
        <h2 className="font-display text-3xl tracking-wide sm:text-4xl">
          Everything the coach actually shows you
        </h2>
        <p className="mt-3 text-ink-muted">
          Real panels from the product, not icons standing in for features.
        </p>
      </Reveal>

      <div className="mt-14 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-flow-dense lg:grid-cols-4">
        <Reveal as="div" className="lg:col-span-2 lg:row-span-2">
          <Card className="flex h-full flex-col gap-4">
            <div>
              <p className="font-semibold text-ink">Mistake breakdown by phase</p>
              <p className="mt-1 text-sm text-ink-muted">
                Where your deaths and misplays actually cluster, over your last
                24 games.
              </p>
            </div>
            <BarList data={PHASE_MISTAKES} caption="Mistakes by game phase" className="mt-2" />
          </Card>
        </Reveal>

        <Reveal as="div" delayMs={80} className="lg:col-span-2">
          <Card className="flex h-full flex-col gap-4">
            <div>
              <p className="font-semibold text-ink">Hero &amp; build recommendations</p>
              <p className="mt-1 text-sm text-ink-muted">
                Strong on the current patch, and a good fit for your pool.
              </p>
            </div>
            <div className="flex flex-col gap-2">
              {RECOMMENDATIONS.map((rec) => (
                <div
                  key={rec.heroId}
                  className="flex items-center gap-3 rounded-xl border border-glass-edge bg-surface-2/50 p-2.5"
                >
                  <HeroPortrait heroId={rec.heroId} heroName={rec.heroName} size="sm" />
                  <span className="min-w-0 flex-1 truncate text-sm text-ink">
                    {rec.heroName}
                  </span>
                  <span className="shrink-0 rounded-lg bg-keyword px-2 py-1 font-mono text-xs font-semibold text-base">
                    {rec.fit}
                  </span>
                </div>
              ))}
            </div>
          </Card>
        </Reveal>

        <Reveal as="div" delayMs={160} className="lg:col-span-2">
          <Card className="flex h-full flex-col gap-4">
            <div>
              <p className="font-semibold text-ink">Timeline of flagged moments</p>
              <p className="mt-1 text-sm text-ink-muted">
                Every match, timestamped, so you know exactly what to review.
              </p>
            </div>
            <ul className="flex flex-col gap-2.5">
              {FLAGS.map((flag) => (
                <li key={flag.time} className="flex items-baseline gap-3 text-sm">
                  <span className="shrink-0 font-mono text-xs tabular-nums text-error">
                    {flag.time}
                  </span>
                  <span className="text-ink-muted">{flag.label}</span>
                </li>
              ))}
            </ul>
          </Card>
        </Reveal>

        {SMALL_TILES.map((tile, i) => (
          <Reveal key={tile.title} as="div" delayMs={240 + i * 80}>
            <Card className="flex h-full flex-col gap-3">
              <span className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-surface-2 text-function">
                <Icon name={tile.icon} className="size-5" />
              </span>
              <p className="font-semibold text-ink">{tile.title}</p>
              <p className="text-sm leading-relaxed text-ink-muted">{tile.body}</p>
            </Card>
          </Reveal>
        ))}
      </div>
    </section>
  );
}
