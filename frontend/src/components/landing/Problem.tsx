import { Reveal } from "@/components/landing/Reveal";

export function Problem() {
  return (
    <section className="safe-x mx-auto max-w-3xl py-16 text-center lg:py-24">
      <Reveal className="flex flex-col gap-3 text-lg leading-relaxed text-ink-muted lg:text-xl">
        <p>You watch pro VODs. You read the guides. You know the timings.</p>
        <p>You still lose the same lane, the same way, three games in a row.</p>
        <p>And you can&rsquo;t tell if last night was bad luck or a pattern.</p>
      </Reveal>

      <Reveal
        delayMs={150}
        className="mt-8 font-display text-2xl tracking-wide text-ink sm:text-3xl"
      >
        You don&rsquo;t need more information.{" "}
        <span className="bg-gradient-to-r from-keyword to-function bg-clip-text text-transparent">
          You need to know what you keep doing wrong.
        </span>
      </Reveal>
    </section>
  );
}
