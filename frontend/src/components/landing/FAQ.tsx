import { Reveal } from "@/components/landing/Reveal";
import { Icon } from "@/components/ui/Icon";

const ITEMS = [
  {
    q: "Do I need to install anything?",
    a: "No. Sign in with Steam and the coach reads your public match history through OpenDota — nothing runs on your machine, no overlay, no client mod.",
  },
  {
    q: "Is this allowed by Valve?",
    a: "Yes. This only reads publicly available match data through OpenDota's API, the same data Dotabuff and Stratz use. It never touches the game client, memory or files, and it never automates play.",
  },
  {
    q: "What data do you actually read?",
    a: "Your public match history: heroes played, KDA, gold and experience, items, and objective participation. We never ask for your Steam password, and Steam's own login never shares it with us.",
  },
  {
    q: "What if I'm a low-MMR or new player?",
    a: "The coach still works — benchmarks are scoped to your bracket, not the pro scene, and a small sample size is stated plainly rather than turned into a false percentile.",
  },
  {
    q: "How do I cancel?",
    a: "From your account's billing page, any time. Cancelling stops future charges; every measured feature (stats, benchmarks, hero intelligence, patterns) keeps working — only new AI analysis is gated.",
  },
  {
    q: "How do voucher codes work?",
    a: "A voucher adds a fixed number of days of subscription access to your account once redeemed. Sign in first, then redeem the code from your billing page — one code, one account.",
  },
];

export function FAQ() {
  return (
    <section id="faq" className="safe-x mx-auto max-w-3xl py-16 lg:py-24">
      <Reveal className="text-center">
        <h2 className="font-display text-3xl tracking-wide sm:text-4xl">
          Questions, answered
        </h2>
      </Reveal>

      <div className="mt-10 flex flex-col gap-3">
        {ITEMS.map((item, i) => (
          <Reveal key={item.q} delayMs={i * 60}>
            <details className="group glass rounded-card px-5 py-4 open:pb-5">
              <summary className="focus-neon flex cursor-pointer list-none items-center justify-between gap-4 rounded-lg py-1 text-left font-medium text-ink [&::-webkit-details-marker]:hidden">
                {item.q}
                <Icon
                  name="chevron-down"
                  className="size-5 shrink-0 text-ink-faint transition-transform duration-200 group-open:rotate-180"
                />
              </summary>
              <p className="mt-3 text-sm leading-relaxed text-ink-muted">
                {item.a}
              </p>
            </details>
          </Reveal>
        ))}
      </div>
    </section>
  );
}
