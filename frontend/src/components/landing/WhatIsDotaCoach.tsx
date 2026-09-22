import { Reveal } from "@/components/landing/Reveal";
import { Card } from "@/components/ui/Card";

/**
 * The page's long-form section, and the only one written primarily to be
 * read by a search engine as well as a visitor.
 *
 * The rest of the landing page is short, declarative marketing copy — good
 * for conversion, thin for ranking, because there is almost no text for a
 * query like "ai dota 2 coach" or "how to climb MMR" to match against. This
 * section carries that weight: real prose, headed by the question people
 * type, with the product's actual mechanics as the answer rather than
 * repeated keywords.
 *
 * It replaced the fabricated usage counters and testimonials that used to sit
 * here. Invented social proof on a page being pushed for search is a
 * liability twice over — it is a consumer-protection problem, and it is the
 * exact pattern search quality raters are told to flag.
 */

const BRACKETS = [
  "Herald & Guardian players who want the basics measured, not guessed",
  "Crusader to Legend players stuck on the same plateau for months",
  "Ancient & Divine players whose mistakes are now too subtle to self-diagnose",
  "Returning players catching up on a patch they did not play",
];

export function WhatIsDotaCoach() {
  return (
    <section
      id="what-is-a-dota-coach"
      className="safe-x mx-auto max-w-4xl py-16 lg:py-24"
    >
      <Reveal className="flex flex-col gap-4">
        <h2 className="font-display text-3xl tracking-wide sm:text-4xl">
          What an AI Dota 2 coach actually does
        </h2>
        <p className="text-lg leading-relaxed text-ink-muted">
          Most Dota 2 stats sites hand you a wall of numbers and leave the
          interpretation to you. A coach does the opposite: it reads the same
          match history, works out which numbers are actually costing you MMR,
          and tells you what to do differently next game.
        </p>
      </Reveal>

      <div className="mt-10 flex flex-col gap-8">
        <Reveal delayMs={80} className="flex flex-col gap-3">
          <h3 className="font-display text-xl tracking-wide text-ink">
            It measures, then interprets — in that order
          </h3>
          <p className="leading-relaxed text-ink-muted">
            Every figure on your dashboard — GPM, XPM, KDA, last hits at ten
            minutes, deaths per ten, kill participation, item timings — is
            calculated deterministically from your match data on the server.
            The AI never invents a statistic. It reads the finished numbers and
            explains what the pattern behind them means, which is the part a
            spreadsheet cannot do for you.
          </p>
        </Reveal>

        <Reveal delayMs={120} className="flex flex-col gap-3">
          <h3 className="font-display text-xl tracking-wide text-ink">
            It compares you to your bracket, not to pros
          </h3>
          <p className="leading-relaxed text-ink-muted">
            A 480 GPM carry game means something very different in Archon than
            in Divine. Benchmarks are scoped to your rank, your role, your hero
            and the current patch, so &ldquo;behind&rdquo; means behind the
            players you are actually queuing against. Where the sample is too
            small to support a percentile, the coach says so instead of
            inventing precision.
          </p>
        </Reveal>

        <Reveal delayMs={160} className="flex flex-col gap-3">
          <h3 className="font-display text-xl tracking-wide text-ink">
            It finds patterns, not one-off bad games
          </h3>
          <p className="leading-relaxed text-ink-muted">
            One disaster game is noise. The same death to the same rotation in
            nine of your last twenty is a habit, and habits are what a coach is
            for. Dota Coach only calls something a weakness once it has
            repeated across enough matches to be evidence, then picks a single
            training focus from those findings and charts it match over match
            so you can see whether the fix is working.
          </p>
        </Reveal>

        <Reveal delayMs={200} className="flex flex-col gap-3">
          <h3 className="font-display text-xl tracking-wide text-ink">
            It picks heroes that fit you, not just the meta
          </h3>
          <p className="leading-relaxed text-ink-muted">
            The strongest hero on the patch is a bad recommendation if you have
            eleven games on it and lose most of them. Hero recommendations
            combine current meta strength with your own hero pool, your
            performance and experience on each hero, your recent form and your
            current training focus — so the suggestion is one you can actually
            execute this week.
          </p>
        </Reveal>
      </div>

      <Reveal delayMs={240} className="mt-10">
        <Card className="flex flex-col gap-4">
          <h3 className="font-display text-xl tracking-wide text-ink">
            Who it is for
          </h3>
          <ul className="flex flex-col gap-2.5">
            {BRACKETS.map((bracket) => (
              <li
                key={bracket}
                className="flex items-start gap-2.5 text-sm leading-relaxed text-ink-muted"
              >
                <span
                  aria-hidden
                  className="mt-2 size-1.5 shrink-0 rounded-full bg-keyword"
                />
                {bracket}
              </li>
            ))}
          </ul>
        </Card>
      </Reveal>
    </section>
  );
}
