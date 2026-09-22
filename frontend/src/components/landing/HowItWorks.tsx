import { Reveal } from "@/components/landing/Reveal";
import { Icon, type IconName } from "@/components/ui/Icon";

const STEPS: {
  icon: IconName;
  title: string;
  body: string;
}[] = [
  {
    icon: "steam",
    title: "Connect your Steam account",
    body: "Sign in through Steam's own login. We never see or ask for your password, only your public match history.",
  },
  {
    icon: "gauge",
    title: "We analyze your recent matches",
    body: "Laning, deaths, gold and objectives from your Dota 2 match history — turned into deterministic numbers and benchmarked against your rank and role.",
  },
  {
    icon: "trophy",
    title: "Get a plan that updates every game",
    body: "One training focus, picked from real evidence, with progress that moves as you play — not a one-time report.",
  },
];

export function HowItWorks() {
  return (
    <section
      id="how-it-works"
      className="safe-x mx-auto max-w-7xl py-16 lg:py-24"
    >
      <Reveal className="mx-auto max-w-2xl text-center">
        <h2 className="font-display text-3xl tracking-wide sm:text-4xl">
          How the Dota 2 coaching works
        </h2>
        <p className="mt-3 text-ink-muted">
          Three steps, no setup, nothing to install — the coach reads the match
          history OpenDota already has on your account.
        </p>
      </Reveal>

      <div className="relative mt-14 grid grid-cols-1 gap-10 sm:grid-cols-3 sm:gap-6">
        {/* The connecting line, desktop only: a single hairline behind the
            three icons rather than three separate short lines, so the eye
            reads it as one path rather than three unrelated dashes. */}
        <div
          aria-hidden
          className="absolute top-7 right-[16.6%] left-[16.6%] hidden h-px bg-gradient-to-r from-transparent via-border to-transparent sm:block"
        />

        {STEPS.map((step, i) => (
          <Reveal
            key={step.title}
            delayMs={i * 120}
            className="relative flex flex-col items-center gap-3 text-center"
          >
            <span className="glass relative z-10 flex size-14 items-center justify-center rounded-full text-keyword">
              <Icon name={step.icon} className="size-6" />
            </span>
            <h3 className="font-semibold text-ink">{step.title}</h3>
            <p className="max-w-xs text-sm leading-relaxed text-ink-muted">
              {step.body}
            </p>
          </Reveal>
        ))}
      </div>
    </section>
  );
}
