import { Sparkline } from "@/components/charts/Sparkline";
import { Reveal } from "@/components/landing/Reveal";
import { SampleBadge } from "@/components/landing/SampleBadge";
import { Card } from "@/components/ui/Card";
import { cn } from "@/lib/utils";

/** Sample figures only; each panel says so. */
const FOCUS_TREND = [52, 48, 50, 45, 44, 41, 40];

const IMPROVED = [
  { metric: "XPM", before: "583", after: "608", delta: "+4%" },
  { metric: "Last hits / min", before: "2.6", after: "2.9", delta: "+12%" },
  { metric: "GPM", before: "377", after: "397", delta: "+5%" },
];

const ROWS = [
  {
    title: "Measured against your rank — and the next one",
    body: "Every metric is a percentile among players at your medal, with the next medal's median beside it, so you can see the exact gap you have to close.",
    panel: (
      <div className="flex flex-col gap-4">
        <PanelHead>Gold per minute · Mirana, Soft Support</PanelHead>
        <div className="flex flex-wrap items-end gap-x-6 gap-y-3">
          <Figure value="512" label="You" strong />
          <Figure value="478" label="Legend median" />
          <Figure value="553" label="Ancient median" />
          <span className="mb-1 rounded-lg bg-string/15 px-2 py-1 font-mono text-xs font-semibold text-string">
            74th percentile
          </span>
        </div>
        <p className="font-mono text-xs text-ink-muted">Gap to Ancient: +8% GPM</p>
      </div>
    ),
  },
  {
    title: "One thing to train next",
    body: "The coach picks a single focus from a pattern that repeats across your games, sets a target, and tracks it match over match.",
    panel: (
      <div className="flex flex-col gap-3">
        <PanelHead>Current focus · Low teamfight participation</PanelHead>
        <Sparkline values={FOCUS_TREND} label="Share of games with low teamfight participation" formatValue={(v) => `${v}%`} />
        <p className="font-mono text-xs text-ink-muted">Happens in 40% of games → target 30%</p>
      </div>
    ),
  },
  {
    title: "Watch yourself improve",
    body: "Each coaching session compares itself with the last one, so progress is a number you can check rather than a feeling.",
    panel: (
      <div className="flex flex-col gap-3">
        <PanelHead>Since your last session</PanelHead>
        <p className="flex items-baseline gap-2">
          <span className="font-display text-3xl tabular-nums text-ink">60 → 63</span>
          <span className="text-sm text-ink-faint">performance score</span>
        </p>
        <ul className="flex flex-col divide-y divide-glass-edge text-sm">
          {IMPROVED.map((row) => (
            <li key={row.metric} className="flex items-center justify-between gap-3 py-1.5">
              <span className="text-ink-muted">{row.metric}</span>
              <span className="font-mono tabular-nums text-ink">
                {row.before} → {row.after}{" "}
                <span className="text-string">▲ {row.delta}</span>
              </span>
            </li>
          ))}
        </ul>
      </div>
    ),
  },
];

export function ProductDeepDive() {
  return (
    <section id="product" className="safe-x mx-auto max-w-7xl scroll-mt-20 py-16 lg:py-24">
      <Reveal className="mx-auto max-w-2xl text-center">
        <h2 className="font-display text-3xl tracking-wide sm:text-4xl">
          A sample Dota 2 coaching report, panel by panel
        </h2>
      </Reveal>

      <div className="mt-14 flex flex-col gap-16 lg:gap-24">
        {ROWS.map((row, i) => (
          <Reveal
            key={row.title}
            className="grid grid-cols-1 items-center gap-8 lg:grid-cols-2 lg:gap-16"
          >
            <div className={cn("flex flex-col gap-3", i % 2 === 1 && "lg:order-2")}>
              <h3 className="font-display text-2xl tracking-wide text-ink">{row.title}</h3>
              <p className="leading-relaxed text-ink-muted">{row.body}</p>
            </div>

            <Card className={cn("min-w-0", i % 2 === 1 && "lg:order-1")}>{row.panel}</Card>
          </Reveal>
        ))}
      </div>
    </section>
  );
}

function PanelHead({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <p className="text-xs uppercase tracking-widest text-ink-faint">{children}</p>
      <SampleBadge />
    </div>
  );
}

function Figure({ value, label, strong = false }: { value: string; label: string; strong?: boolean }) {
  return (
    <div className="flex flex-col items-center gap-1">
      <p className={cn("font-display text-3xl tabular-nums", strong ? "text-ink" : "text-ink-faint")}>{value}</p>
      <p className="text-xs text-ink-faint">{label}</p>
    </div>
  );
}
