import { Gauge } from "@/components/charts/Gauge";
import { Sparkline } from "@/components/charts/Sparkline";
import { ProBadge, SampleBadge } from "@/components/landing/SampleBadge";
import { Reveal } from "@/components/landing/Reveal";
import { Card } from "@/components/ui/Card";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { cn } from "@/lib/utils";

/** Sample figures only; each card says so. */
const MOMENTUM = [0, 32, 13, 45, 77, 58, 90, 122, 103, 135, 167, 199];

const BENCHMARK = [
  { metric: "GPM", you: 74, gap: "+8%" },
  { metric: "Kills/min", you: 58, gap: "+27%" },
  { metric: "Tower dmg", you: 41, gap: "+77%" },
];

const SKILLS = [
  { axis: "Farming", p: 74 },
  { axis: "Fighting", p: 65 },
  { axis: "Supporting", p: 66 },
  { axis: "Pushing", p: 63 },
  { axis: "Survival", p: 57 },
];

const QUADRANTS = [
  { label: "Learn these", hint: "strong meta, low fit", heroes: [{ id: 106, name: "Ember Spirit" }] },
  { label: "Play more", hint: "strong meta, good fit", heroes: [{ id: 9, name: "Mirana" }, { id: 112, name: "Winter Wyvern" }] },
  { label: "Drop for now", hint: "weak meta, low fit", heroes: [{ id: 75, name: "Silencer" }] },
  { label: "Pocket picks", hint: "weak meta, good fit", heroes: [{ id: 87, name: "Disruptor" }] },
];

export function FeaturesBento() {
  return (
    <section id="features" className="safe-x mx-auto max-w-7xl py-16 lg:py-24">
      <Reveal className="mx-auto max-w-2xl text-center">
        <h2 className="font-display text-3xl tracking-wide sm:text-4xl">
          Everything your Dota 2 coach shows you
        </h2>
        <p className="mt-3 text-ink-muted">
          One card per tab in the app: Rank, Benchmark, Overview, Heroes and Coach.
        </p>
      </Reveal>

      <div className="mt-14 grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-6">
        <Reveal as="div" className="lg:col-span-3">
          <Tile title="Rank" line="Estimated medal and MMR range, how far the next medal is, and your momentum.">
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <p className="font-display text-xl tracking-wide text-ink">
                Legend 3 <span className="font-mono text-sm text-ink-faint">~3,464 est.</span>
              </p>
              <span className="whitespace-nowrap rounded-full bg-you/15 px-2 py-0.5 font-mono text-xs text-function">
                Next medal ≈ 13 wins
              </span>
            </div>
            <Sparkline values={MOMENTUM} label="Modeled MMR over the last 12 ranked games" formatValue={(v) => `+${Math.round(v)}`} />
            <ul className="flex flex-wrap gap-1.5 font-mono text-[0.6875rem] text-ink-muted">
              <Chip>Streak 6W</Chip>
              <Chip>Avg +32 / −19 est.</Chip>
              <Chip>Rank over time</Chip>
            </ul>
          </Tile>
        </Reveal>

        <Reveal as="div" delayMs={80} className="lg:col-span-3">
          <Tile title="Benchmark" line="You against your rank's median and the next rank's, as a percentile per metric.">
            <table className="w-full text-xs">
              <thead>
                <tr className="text-left text-[0.625rem] uppercase tracking-wider text-ink-faint">
                  <th className="pb-1.5 font-medium">Metric</th>
                  <th className="pb-1.5 font-medium">Percentile vs Legend</th>
                  <th className="pb-1.5 text-right font-medium">Gap to Ancient</th>
                </tr>
              </thead>
              <tbody>
                {BENCHMARK.map((row) => (
                  <tr key={row.metric}>
                    <td className="py-1 text-ink-muted">{row.metric}</td>
                    <td className="py-1 pr-3">
                      <span className="flex items-center gap-2">
                        <span className="relative block h-1.5 flex-1 rounded-full bg-mark-track">
                          <span className="block h-full rounded-full bg-you" style={{ width: `${row.you}%` }} />
                          <span aria-hidden className="absolute -inset-y-0.5 left-1/2 w-0.5 rounded bg-ink-faint" />
                        </span>
                        <span className="w-8 font-mono tabular-nums text-ink-muted">{row.you}th</span>
                      </span>
                    </td>
                    <td className="py-1 text-right font-mono tabular-nums text-number">{row.gap}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="text-[0.6875rem] text-ink-faint">Tick = your rank&apos;s median. Gap = how far below the Ancient median you are.</p>
          </Tile>
        </Reveal>

        <Reveal as="div" delayMs={120} className="lg:col-span-2">
          <Tile title="Overview" line="Your current focus, the last 20 games against the 20 before, and your skills.">
            <p className="flex flex-wrap items-center gap-1.5 text-xs">
              <span className="font-semibold text-ink">Low teamfight participation</span>
              <span className="rounded-full bg-warn/15 px-2 py-0.5 text-[0.625rem] font-semibold uppercase text-warn">High impact</span>
              <span className="font-mono text-ink-muted">40% → 30%</span>
            </p>
            <dl className="grid grid-cols-3 gap-2">
              <Kpi label="Win rate" value="58%" delta="▲ 6 pts" />
              <Kpi label="KDA" value="3.4" delta="▲ 0.4" />
              <Kpi label="GPM" value="512" delta="▲ 24" />
            </dl>
            <ul className="flex flex-col gap-1">
              {SKILLS.map((s) => (
                <li key={s.axis} className="flex items-center gap-2 text-[0.6875rem]">
                  <span className="w-16 text-ink-muted">{s.axis}</span>
                  <span className="block h-1.5 flex-1 overflow-hidden rounded-full bg-mark-track">
                    <span className="block h-full rounded-r bg-you" style={{ width: `${s.p}%` }} />
                  </span>
                  <span className="w-8 text-right font-mono tabular-nums text-ink-faint">{s.p}th</span>
                </li>
              ))}
            </ul>
          </Tile>
        </Reveal>

        <Reveal as="div" delayMs={160} className="lg:col-span-2">
          <Tile title="Heroes" line="Which strong heroes actually fit you — not just the highest win rate.">
            <div className="grid grid-cols-2 gap-1.5">
              {QUADRANTS.map((q) => (
                <div key={q.label} className="flex min-w-0 flex-col gap-1.5 rounded-lg border border-glass-edge bg-surface-2/40 p-2">
                  <span className="text-[0.6875rem] font-semibold text-ink">{q.label}</span>
                  <span className="text-[0.625rem] text-ink-faint">{q.hint}</span>
                  <span className="flex gap-1">
                    {q.heroes.map((h) => (
                      <HeroPortrait key={h.id} heroId={h.id} heroName={h.name} size="sm" className="!h-5 !w-9" />
                    ))}
                  </span>
                </div>
              ))}
            </div>
            <div className="flex items-center gap-2 rounded-lg border border-keyword/30 bg-keyword/10 p-2">
              <HeroPortrait heroId={9} heroName="Mirana" size="sm" className="!h-6 !w-11 shrink-0" />
              <span className="min-w-0 flex-1 text-xs">
                <span className="block text-ink">Recommended: Mirana</span>
                <span className="block text-ink-faint">Comfort 72 · Why this score?</span>
              </span>
            </div>
            <p className="text-[0.6875rem] text-ink-faint">Plus a “Worth trying” list for strong heroes you have barely played.</p>
          </Tile>
        </Reveal>

        <Reveal as="div" delayMs={200} className="md:col-span-2 lg:col-span-2">
          <Tile title="Coach" pro line="An AI read of your numbers, with a role score and a training plan.">
            <div className="flex items-center gap-3">
              <div className="w-24 shrink-0">
                <Gauge value={63} marker={60} label="Role score" />
              </div>
              <div className="flex min-w-0 flex-col gap-1.5 text-[0.6875rem]">
                <span className="text-ink-faint">This week</span>
                <span className="flex flex-wrap gap-1">
                  <span className="rounded-full bg-mark-win/15 px-2 py-0.5 text-string">2 strengths</span>
                  <span className="rounded-full bg-mark-loss/15 px-2 py-0.5 text-error">1 weakness</span>
                  <span className="rounded-full bg-warn/15 px-2 py-0.5 text-warn">1 focus</span>
                </span>
                <span className="font-mono text-ink-muted">Since last session 60 → 63</span>
              </div>
            </div>
            <ul className="flex flex-col gap-1.5 text-xs">
              <Finding kind="Strength" tone="text-string">Deaths below peers · 63rd percentile</Finding>
              <Finding kind="Weakness" tone="text-error">Teamfight participation in 4 of 10 games</Finding>
              <Finding kind="Warning" tone="text-warn">Hero fit only moderate on your top hero</Finding>
            </ul>
            <p className="text-[0.6875rem] text-ink-faint">Training plan with tracked goals, and a chat that answers from your own stats.</p>
          </Tile>
        </Reveal>
      </div>
    </section>
  );
}

function Tile({
  title,
  line,
  pro = false,
  children,
}: {
  title: string;
  line: string;
  pro?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Card className="flex h-full flex-col gap-4">
      <div className="flex flex-col gap-1">
        <div className="flex flex-wrap items-center gap-2">
          <p className="font-semibold text-ink">{title}</p>
          {pro ? <ProBadge /> : null}
          <SampleBadge className="ml-auto" />
        </div>
        <p className="text-sm text-ink-muted">{line}</p>
      </div>
      {children}
    </Card>
  );
}

function Chip({ children }: { children: React.ReactNode }) {
  return <li className="rounded-full bg-surface-2 px-2 py-0.5">{children}</li>;
}

function Kpi({ label, value, delta }: { label: string; value: string; delta: string }) {
  return (
    <div className="min-w-0 rounded-lg border border-glass-edge bg-surface-2/40 p-2">
      <dt className="truncate text-[0.625rem] uppercase tracking-wider text-ink-faint">{label}</dt>
      <dd className="font-display text-lg tabular-nums text-ink">{value}</dd>
      <dd className="font-mono text-[0.625rem] text-string">{delta}</dd>
    </div>
  );
}

function Finding({ kind, tone, children }: { kind: string; tone: string; children: React.ReactNode }) {
  return (
    <li className="flex items-baseline gap-2">
      <span className={cn("w-16 shrink-0 text-[0.625rem] font-semibold uppercase tracking-wider", tone)}>{kind}</span>
      <span className="min-w-0 text-ink-muted">{children}</span>
    </li>
  );
}
