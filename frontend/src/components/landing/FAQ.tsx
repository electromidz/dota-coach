import { Reveal } from "@/components/landing/Reveal";
import { Icon } from "@/components/ui/Icon";
import { FAQ_ITEMS } from "@/lib/faq";

/**
 * Reads its questions from `FAQ_ITEMS`, the same array `LandingJsonLd` marks
 * up as a `FAQPage`. Structured data that says something the visible page
 * does not is a spam signal, so there is exactly one copy of this text.
 */
export function FAQ() {
  return (
    <section id="faq" className="safe-x mx-auto max-w-3xl py-16 lg:py-24">
      <Reveal className="text-center">
        <h2 className="font-display text-3xl tracking-wide sm:text-4xl">
          Dota 2 coaching questions, answered
        </h2>
      </Reveal>

      <div className="mt-10 flex flex-col gap-3">
        {FAQ_ITEMS.map((item, i) => (
          <Reveal key={item.q} delayMs={i * 60}>
            <details className="group glass rounded-card px-5 py-4 open:pb-5">
              <summary className="focus-neon flex cursor-pointer list-none items-center justify-between gap-4 rounded-lg py-1 text-left font-medium text-ink [&::-webkit-details-marker]:hidden">
                <h3 className="font-medium">{item.q}</h3>
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
