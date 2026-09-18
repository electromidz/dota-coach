import { Reveal } from "@/components/landing/Reveal";
import { ButtonLink } from "@/components/ui/Button";
import { Icon } from "@/components/ui/Icon";
import { steamLoginUrl } from "@/lib/api";

export function FinalCta() {
  return (
    <section className="safe-x relative mx-auto max-w-7xl py-16 lg:py-24">
      <Reveal className="relative overflow-hidden rounded-card border border-glass-edge px-6 py-16 text-center sm:px-12">
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 -z-10"
          style={{
            background:
              "radial-gradient(60% 100% at 50% 0%, oklch(from #bb9af7 l c h / 0.22), transparent 70%)," +
              "radial-gradient(50% 80% at 90% 100%, oklch(from #7aa2f7 l c h / 0.16), transparent 70%)",
          }}
        />

        <h2 className="font-display text-4xl tracking-wide sm:text-5xl">
          Stop guessing.{" "}
          <span className="bg-gradient-to-r from-keyword to-function bg-clip-text text-transparent">
            Start climbing.
          </span>
        </h2>

        <div className="mt-8 flex justify-center">
          <ButtonLink href={steamLoginUrl()}>
            <Icon name="steam" className="size-5" />
            Start free trial — no card
          </ButtonLink>
        </div>

        <p className="mt-4 text-sm text-ink-faint">
          Takes about a minute. Cancel anytime.
        </p>
      </Reveal>
    </section>
  );
}
