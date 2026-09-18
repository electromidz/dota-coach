import { Reveal } from "@/components/landing/Reveal";
import { Meter } from "@/components/charts/Meter";
import { Sparkline } from "@/components/charts/Sparkline";
import { Card } from "@/components/ui/Card";
import { cn } from "@/lib/utils";

const PROGRESS = [58, 51, 60, 64, 69, 71, 76];

const ROWS = [
  {
    title: "Every match, read for you",
    body: "Laning, economy and deaths turned into numbers you can act on — not a wall of raw stats you still have to interpret yourself.",
    panel: (
      <div className="flex flex-col gap-4">
        <p className="text-xs uppercase tracking-widest text-ink-faint">
          Laning phase · 0–10 min
        </p>
        <Meter value={0.72} label="Last hits vs. bracket median" valueText="72nd pct" />
        <Meter value={0.41} label="Deaths before 10 min" valueText="Below median" />
      </div>
    ),
  },
  {
    title: "Measured against your peers",
    body: "Every figure is benchmarked against your rank and role — never a global average that flatters a Herald and undersells an Immortal alike.",
    panel: (
      <div className="flex flex-col gap-3">
        <p className="text-xs uppercase tracking-widest text-ink-faint">
          Kill participation · Ancient, offlane
        </p>
        <div className="flex items-end gap-6">
          <div className="flex flex-col items-center gap-1">
            <p className="font-display text-3xl text-ink">61%</p>
            <p className="text-xs text-ink-faint">You</p>
          </div>
          <div className="flex flex-col items-center gap-1">
            <p className="font-display text-3xl text-ink-faint">54%</p>
            <p className="text-xs text-ink-faint">Peer median</p>
          </div>
          <span className="mb-1 rounded-lg bg-string/15 px-2 py-1 font-mono text-xs font-semibold text-string">
            72nd percentile
          </span>
        </div>
      </div>
    ),
  },
  {
    title: "One thing to train next",
    body: "The coach picks a single focus from real evidence and tracks it match over match, so you always know whether it's actually working.",
    panel: (
      <div className="flex flex-col gap-3">
        <p className="text-xs uppercase tracking-widest text-ink-faint">
          Training focus · Deaths per 10 minutes
        </p>
        <Sparkline values={PROGRESS} label="Training focus progress" />
        <p className="text-xs text-ink-muted">
          Baseline 4.1 → target 2.5 → current 3.0
        </p>
      </div>
    ),
  },
];

export function ProductDeepDive() {
  return (
    <section id="product" className="safe-x mx-auto max-w-7xl py-16 lg:py-24">
      <Reveal className="mx-auto max-w-2xl text-center">
        <h2 className="font-display text-3xl tracking-wide sm:text-4xl">
          A sample report, panel by panel
        </h2>
      </Reveal>

      <div className="mt-14 flex flex-col gap-16 lg:gap-24">
        {ROWS.map((row, i) => (
          <Reveal
            key={row.title}
            className={cn(
              "grid grid-cols-1 items-center gap-8 lg:grid-cols-2 lg:gap-16",
            )}
          >
            <div
              className={cn(
                "flex flex-col gap-3",
                i % 2 === 1 && "lg:order-2",
              )}
            >
              <h3 className="font-display text-2xl tracking-wide text-ink">
                {row.title}
              </h3>
              <p className="text-ink-muted leading-relaxed">{row.body}</p>
            </div>

            <Card className={cn(i % 2 === 1 && "lg:order-1")}>{row.panel}</Card>
          </Reveal>
        ))}
      </div>
    </section>
  );
}
