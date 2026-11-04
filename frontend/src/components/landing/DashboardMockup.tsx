import { Gauge } from "@/components/charts/Gauge";
import { SampleBadge } from "@/components/landing/SampleBadge";
import { TiltCard } from "@/components/ui/TiltCard";

/**
 * The hero preview, built from three real cards in the product — Rank's next
 * medal, Overview's current focus and Coach's role score — using the app's
 * own primitives, so it matches what a visitor signs into.
 *
 * Every chip sits in normal flow (flex rows with a gap), never absolutely
 * positioned, so nothing can overlap at any width. All figures are sample
 * data and labelled as such.
 */
export function DashboardMockup() {
  return (
    <TiltCard className="relative mx-auto w-full max-w-md">
      <div className="glass pop-3d flex flex-col gap-4 rounded-card p-4 shadow-[0_40px_100px_-30px_oklch(from_#bb9af7_l_c_h_/_0.45)] sm:p-6">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <p className="text-xs uppercase tracking-widest text-ink-faint">Your dashboard</p>
          <SampleBadge />
        </div>

        {/* Rank · next medal */}
        <div className="rounded-xl border border-glass-edge bg-surface-2/50 p-3">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <p className="text-[0.6875rem] uppercase tracking-wider text-ink-faint">Next medal · est.</p>
            <span className="rounded-full bg-warn/15 px-2 py-0.5 text-[0.625rem] font-semibold text-warn">Rank</span>
          </div>
          <p className="mt-1.5 flex flex-wrap items-baseline gap-x-2 font-display text-lg tracking-wide text-ink">
            Legend 3 <span aria-hidden className="text-ink-faint">→</span> Ancient
            <span className="font-mono text-sm text-function">≈ 13 wins</span>
          </p>
          <div
            role="img"
            aria-label="Progress through Legend toward Ancient: about 60 percent"
            className="mt-2 h-1.5 overflow-hidden rounded-full bg-mark-track"
          >
            <div className="h-full w-[60%] rounded-r bg-you" />
          </div>
        </div>

        {/* Overview · current focus */}
        <div className="rounded-xl border border-keyword/30 bg-keyword/10 p-3">
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-[0.6875rem] uppercase tracking-wider text-ink-faint">Current focus</p>
            <span className="rounded-full bg-warn/15 px-2 py-0.5 text-[0.625rem] font-semibold uppercase text-warn">
              High impact
            </span>
          </div>
          <p className="mt-1.5 text-sm font-semibold text-ink">Low teamfight participation</p>
          <ul className="mt-2 flex flex-wrap gap-1.5 text-[0.6875rem]">
            <li className="rounded-full bg-surface-2 px-2 py-0.5 text-ink-muted">Recurring pattern</li>
            <li className="rounded-full bg-surface-2 px-2 py-0.5 font-mono text-ink-muted">40% → 30%</li>
          </ul>
        </div>

        {/* Coach · role score */}
        <div className="flex items-center gap-4 rounded-xl border border-glass-edge bg-surface-2/50 p-3">
          <div className="w-28 shrink-0">
            <Gauge value={63} marker={60} label="Soft Support role score" />
          </div>
          <div className="min-w-0">
            <p className="text-[0.6875rem] uppercase tracking-wider text-ink-faint">Role score · Soft Support</p>
            <p className="mt-1 font-display text-2xl tabular-nums text-ink">
              63<span className="text-sm text-ink-faint">/100</span>
            </p>
            <p className="mt-1 flex flex-wrap items-center gap-1.5 text-[0.6875rem]">
              <span className="rounded-full bg-mark-win/15 px-2 py-0.5 font-mono text-string">▲ 60 → 63</span>
              <span className="text-ink-faint">since last session</span>
            </p>
          </div>
        </div>
      </div>
    </TiltCard>
  );
}
