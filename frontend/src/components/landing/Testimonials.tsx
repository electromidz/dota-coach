import { Reveal } from "@/components/landing/Reveal";
import { Card } from "@/components/ui/Card";

/**
 * Placeholder copy — see the landing-page placeholder notes. Replace every
 * quote, handle and rank with a real player's before this ships; nothing
 * here is a real testimonial.
 */
const TESTIMONIALS = [
  {
    name: "\"Skew\"",
    rank: "Divine 2 · Mid",
    quote:
      "It told me I was dying to the same rotation every single game. I hadn't noticed the pattern in 40 replays. +740 MMR in six weeks.",
  },
  {
    name: "\"quietcarry\"",
    rank: "Legend 5 · Carry",
    quote:
      "My last hits at 10 were fine — my deaths after 25 were the actual problem. Never would have found that myself.",
  },
  {
    name: "\"Offlane Enjoyer\"",
    rank: "Ancient 4 · Offlane",
    quote:
      "One focus at a time instead of a list of ten things. That's the part that actually made it stick.",
  },
];

export function Testimonials() {
  return (
    <section className="safe-x mx-auto max-w-7xl py-16 lg:py-24">
      <Reveal className="mx-auto max-w-2xl text-center">
        <h2 className="font-display text-3xl tracking-wide sm:text-4xl">
          Players stuck at their MMR, unstuck
        </h2>
        <p className="mt-3 text-xs uppercase tracking-widest text-ink-faint">
          Illustrative quotes — placeholder, to be replaced with real players
        </p>
      </Reveal>

      <div className="mt-10 grid grid-cols-1 gap-4 sm:grid-cols-3">
        {TESTIMONIALS.map((t, i) => (
          <Reveal key={t.name} delayMs={i * 100}>
            <Card className="flex h-full flex-col gap-4">
              <div className="flex items-center gap-3">
                <span className="flex size-10 shrink-0 items-center justify-center rounded-full border border-glass-edge bg-surface-2 font-display text-sm text-ink-faint">
                  {t.name.replace(/"/g, "").slice(0, 1)}
                </span>
                <div className="min-w-0">
                  <p className="truncate text-sm font-semibold text-ink">
                    {t.name}
                  </p>
                  <p className="text-xs text-ink-faint">{t.rank}</p>
                </div>
              </div>
              <p className="text-sm leading-relaxed text-ink-muted">
                &ldquo;{t.quote}&rdquo;
              </p>
            </Card>
          </Reveal>
        ))}
      </div>
    </section>
  );
}
