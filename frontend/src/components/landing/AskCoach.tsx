import { Reveal } from "@/components/landing/Reveal";
import { ProBadge, SampleBadge } from "@/components/landing/SampleBadge";

/**
 * The part people pay for, shown rather than described: a short exchange in
 * the shape the real chat produces, answering from the player's own
 * percentiles and saying how many measurements it read.
 */
export function AskCoach() {
  return (
    <section id="ask-coach" className="safe-x mx-auto max-w-5xl scroll-mt-20 py-16 lg:py-24">
      <Reveal className="mx-auto max-w-2xl text-center">
        <h2 className="flex flex-wrap items-center justify-center gap-3 font-display text-3xl tracking-wide sm:text-4xl">
          Ask your coach <ProBadge className="text-xs" />
        </h2>
        <p className="mt-3 text-ink-muted">
          Questions answered from your own stats — percentiles, patterns and goals — not generic guides.
        </p>
      </Reveal>

      <Reveal delayMs={100} className="glass mx-auto mt-10 flex max-w-3xl flex-col gap-4 rounded-card p-4 sm:p-6">
        <div className="flex items-center justify-between gap-2">
          <p className="text-xs uppercase tracking-widest text-ink-faint">Coach chat · Soft Support</p>
          <SampleBadge />
        </div>

        <p className="ml-auto max-w-[85%] rounded-2xl rounded-br-md bg-you/15 px-4 py-2.5 text-sm text-ink">
          What should I focus on to reach Ancient?
        </p>

        <div className="max-w-[92%] rounded-2xl rounded-bl-md border border-glass-edge bg-surface-2/60 px-4 py-3 text-sm leading-relaxed text-ink-muted">
          <p>
            Your economy is already fine for a Soft Support — GPM sits at the{" "}
            <strong className="font-medium text-ink">74th percentile</strong> for Legend. The gap is
            elsewhere:
          </p>
          <ul className="mt-2 flex list-disc flex-col gap-1 pl-5">
            <li>
              Assists per minute is at the <strong className="font-medium text-ink">48th percentile</strong>,
              just under the Legend median.
            </li>
            <li>
              Tower damage is at the <strong className="font-medium text-ink">41st percentile</strong>, and{" "}
              <strong className="font-medium text-ink">77%</strong> below the Ancient median.
            </li>
          </ul>
          <p className="mt-2">
            Keep your deaths where they are and join the next two fights after you have placed vision.
          </p>
          <p className="mt-3 text-xs text-ink-faint">Read from 9 measurements</p>
        </div>
      </Reveal>
    </section>
  );
}
