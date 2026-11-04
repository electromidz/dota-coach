import { Reveal } from "@/components/landing/Reveal";
import { SampleBadge } from "@/components/landing/SampleBadge";
import { ButtonLink } from "@/components/ui/Button";
import { Icon } from "@/components/ui/Icon";
import { steamLoginUrl } from "@/lib/api";

/**
 * The quickest thing a visitor can learn about themselves. The estimate is
 * the Rank page's: medal floors from the public MMR table, the middle of the
 * current star, and the modeled MMR per win — labelled as an estimate.
 */
export function NextMedalHook() {
  return (
    <section className="safe-x mx-auto max-w-5xl py-12 lg:py-16">
      <Reveal className="glass grid items-center gap-6 rounded-card p-6 sm:p-8 md:grid-cols-[1.3fr_1fr]">
        <div className="flex flex-col gap-3">
          <h2 className="font-display text-2xl tracking-wide sm:text-3xl">
            Find out exactly how many wins you are from your next medal — free.
          </h2>
          <p className="text-ink-muted">
            Sign in and the Rank page estimates it from your medal and your recent ranked games.
          </p>
          <ButtonLink href={steamLoginUrl()} className="w-fit whitespace-nowrap">
            <Icon name="steam" className="size-5" />
            Check my next medal
          </ButtonLink>
        </div>

        <div className="flex flex-col gap-3 rounded-xl border border-glass-edge bg-surface-2/50 p-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <p className="text-[0.6875rem] uppercase tracking-wider text-ink-faint">Next medal · est.</p>
            <SampleBadge />
          </div>
          <p className="font-display text-3xl tabular-nums text-ink">≈ 13 wins</p>
          <p className="text-sm text-ink-muted">Legend 3 → Ancient</p>
          <div className="h-1.5 overflow-hidden rounded-full bg-mark-track" aria-hidden>
            <div className="h-full w-[60%] rounded-r bg-you" />
          </div>
          <ul className="flex flex-wrap gap-1.5 font-mono text-[0.6875rem] text-ink-muted">
            <li className="rounded-full bg-surface-2 px-2 py-0.5">Streak 6W</li>
            <li className="rounded-full bg-surface-2 px-2 py-0.5">Avg +32 / −19 est.</li>
          </ul>
        </div>
      </Reveal>
    </section>
  );
}
