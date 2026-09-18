import { Counter } from "@/components/landing/Counter";
import { Reveal } from "@/components/landing/Reveal";

/**
 * Illustrative figures — not read from the backend. See the landing-page
 * placeholder notes: replace with real numbers (e.g. from `/api/admin/stats`
 * exposed publicly, or a hand-updated constant) before launch.
 */
const STATS: { value: number; label: string; prefix?: string }[] = [
  { value: 48200, label: "Matches analyzed" },
  { value: 312, label: "Average MMR gained", prefix: "+" },
  { value: 1140, label: "Active players this week" },
];

export function SocialProof() {
  return (
    <section
      aria-label="Usage statistics"
      className="safe-x mx-auto max-w-7xl py-10"
    >
      <Reveal className="glass grid grid-cols-1 divide-y divide-border rounded-card p-6 sm:grid-cols-3 sm:divide-y-0 sm:divide-x">
        {STATS.map((stat) => (
          <div
            key={stat.label}
            className="flex flex-col items-center gap-1 py-4 text-center sm:py-0"
          >
            <p className="font-display text-3xl tracking-wide text-ink sm:text-4xl">
              {stat.prefix}
              <Counter value={stat.value} />
            </p>
            <p className="text-sm text-ink-faint">{stat.label}</p>
          </div>
        ))}
      </Reveal>
    </section>
  );
}
