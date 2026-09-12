import { Card } from "@/components/ui/Card";
import { SystemStatus } from "@/components/ui/SystemStatus";

const STEPS = [
  {
    title: "Connect your Dota account",
    body: "Enter your Dota player ID. No login, no password, nothing to install.",
  },
  {
    title: "Analyze your matches",
    body: "Every game is turned into hard numbers first — farming, fighting, survival, objectives.",
  },
  {
    title: "Discover recurring mistakes",
    body: "The coach compares games over time and finds the habits that keep costing you.",
  },
  {
    title: "Train one thing at a time",
    body: "You get a single focus to work on, and we track it across your next games.",
  },
];

export default function LandingPage() {
  return (
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-14 px-5 py-14 sm:py-20">
      <section className="flex flex-col gap-6">
        <p className="text-xs font-semibold uppercase tracking-[0.2em] text-accent">
          AI Dota Coach
        </p>

        <h1 className="text-4xl font-bold leading-tight tracking-tight sm:text-5xl">
          Your personal
          <br />
          Dota 2 coach
        </h1>

        <p className="max-w-md text-lg leading-relaxed text-ink-muted">
          AI that watches your games, learns your habits, and tells you what to
          improve next.
        </p>

        <div className="flex flex-col gap-2">
          <button
            type="button"
            disabled
            className="w-full rounded-xl bg-accent px-6 py-3.5 text-base font-semibold text-ink transition disabled:cursor-not-allowed disabled:opacity-40 sm:w-auto"
          >
            Analyze My Games
          </button>
          <p className="text-xs text-ink-faint">
            Match syncing is not wired up yet — this build verifies the service
            chain only.
          </p>
        </div>
      </section>

      <section className="flex flex-col gap-4">
        <h2 className="text-sm font-semibold uppercase tracking-wider text-ink-faint">
          How it works
        </h2>

        <ol className="flex flex-col gap-3">
          {STEPS.map((step, index) => (
            <li key={step.title}>
              <Card className="flex gap-4">
                <span className="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-full bg-accent-soft text-sm font-semibold text-accent">
                  {index + 1}
                </span>
                <div className="flex flex-col gap-1">
                  <h3 className="font-semibold">{step.title}</h3>
                  <p className="text-sm leading-relaxed text-ink-muted">
                    {step.body}
                  </p>
                </div>
              </Card>
            </li>
          ))}
        </ol>
      </section>

      <section className="flex flex-col gap-4">
        <h2 className="text-sm font-semibold uppercase tracking-wider text-ink-faint">
          Service status
        </h2>
        <Card>
          <SystemStatus />
        </Card>
      </section>

      <footer className="text-xs leading-relaxed text-ink-faint">
        Scores shown anywhere in this app are estimates derived from public match
        data. They are not an official rating and do not promise MMR gains.
      </footer>
    </main>
  );
}
