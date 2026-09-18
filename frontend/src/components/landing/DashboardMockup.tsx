import { Sparkline } from "@/components/charts/Sparkline";
import { TiltCard } from "@/components/ui/TiltCard";

/** Static series — enough shape to read as a real trend line, no live data
 *  behind it. See the landing-page placeholder notes for what this stands in
 *  for. */
const GPM_TREND = [412, 438, 401, 455, 470, 462, 498, 512];

/**
 * A perspective-tilted preview of the coaching dashboard, built from the
 * app's own primitives (`TiltCard`, `Sparkline`, the glass/card tokens)
 * rather than a screenshot — it never goes stale when the real UI changes,
 * and it costs nothing to ship.
 *
 * Three small callout chips point at real fields (win rate, a flagged
 * mistake, a hero recommendation) so a first-time visitor can match this
 * mockup to the product they are about to sign into.
 */
export function DashboardMockup() {
  return (
    <TiltCard className="relative mx-auto w-full max-w-md">
      <div className="glass pop-3d rounded-card p-5 shadow-[0_40px_100px_-30px_oklch(from_#bb9af7_l_c_h_/_0.45)] sm:p-6">
        <div className="flex items-center justify-between gap-3">
          <div>
            <p className="text-xs uppercase tracking-widest text-ink-faint">
              Overview
            </p>
            <p className="mt-1 font-display text-lg tracking-wide text-ink">
              Last 20 matches
            </p>
          </div>
          <span className="rounded-xl border border-border px-3 py-1 font-mono text-xs uppercase tracking-widest text-string">
            Ancient 3
          </span>
        </div>

        <div className="relative mt-5 grid grid-cols-2 gap-3">
          <Stat label="Win rate" value="58%" tone="text-string" />
          <Stat label="Avg KDA" value="3.4" tone="text-number" />

          <span
            aria-hidden
            className="pointer-events-none absolute -right-3 -top-3 hidden rotate-3 items-center gap-1.5 rounded-lg border border-glass-edge bg-surface-2/95 px-2.5 py-1.5 text-[0.6875rem] font-medium text-ink shadow-lg sm:flex"
          >
            <span className="size-1.5 shrink-0 rounded-full bg-string" />
            +6% this week
          </span>
        </div>

        <div className="mt-5">
          <p className="text-xs uppercase tracking-wider text-ink-faint">
            GPM trend
          </p>
          <Sparkline
            values={GPM_TREND}
            label="Gold per minute"
            formatValue={(v) => Math.round(v).toString()}
            className="mt-1"
          />
        </div>

        <div className="relative mt-5 rounded-xl border border-error/30 bg-error/10 p-3">
          <p className="text-xs font-semibold text-error">
            Flagged · 24:10
          </p>
          <p className="mt-0.5 text-xs leading-relaxed text-ink-muted">
            Died to a rotation you had ward vision on. 4th time this pattern
            showed up in 12 games.
          </p>

          <span
            aria-hidden
            className="pointer-events-none absolute -left-4 top-1/2 hidden -translate-y-1/2 -rotate-2 items-center gap-1.5 rounded-lg border border-glass-edge bg-surface-2/95 px-2.5 py-1.5 text-[0.6875rem] font-medium text-ink shadow-lg sm:flex"
          >
            Recurring pattern
          </span>
        </div>

        <div className="relative mt-5 flex items-center justify-between gap-3 rounded-xl border border-keyword/30 bg-keyword/10 p-3">
          <div>
            <p className="text-xs font-semibold text-keyword">
              Try next: Ember Spirit
            </p>
            <p className="mt-0.5 text-xs text-ink-muted">
              Strong patch, fits your mid pool
            </p>
          </div>
          <span className="shrink-0 rounded-lg bg-keyword px-2 py-1 font-mono text-xs font-semibold text-base">
            94
          </span>

          <span
            aria-hidden
            className="pointer-events-none absolute -bottom-4 right-6 hidden rotate-2 items-center gap-1.5 rounded-lg border border-glass-edge bg-surface-2/95 px-2.5 py-1.5 text-[0.6875rem] font-medium text-ink shadow-lg sm:flex"
          >
            Fit score
          </span>
        </div>
      </div>
    </TiltCard>
  );
}

function Stat({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone: string;
}) {
  return (
    <div className="rounded-xl border border-glass-edge bg-surface-2/50 p-3">
      <p className="text-[0.6875rem] uppercase tracking-wider text-ink-faint">
        {label}
      </p>
      <p className={`mt-1 font-display text-xl tabular-nums ${tone}`}>
        {value}
      </p>
    </div>
  );
}
