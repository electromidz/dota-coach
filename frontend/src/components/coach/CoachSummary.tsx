import { Gauge } from "@/components/charts/Gauge";
import { StackedBar } from "@/components/charts/StackedBar";
import { InfoTip } from "@/components/ui/InfoTip";
import { topInsights } from "@/lib/coach";
import { formatComponentValue } from "@/lib/roles";
import type { Insight, RolePerformance } from "@/lib/types";
import { cn } from "@/lib/utils";

/** Mirrors `SampleConfidence::for_matches`: limited below 20, strong from 60. */
export const MODERATE_AT = 20;
export const STRONG_AT = 60;

/**
 * The answer at a glance: the role score, how much it can be trusted, what
 * it is made of, and the two strengths, two weaknesses and one focus.
 */
export function CoachSummary({
  performance,
  insights,
  focusTitle,
}: {
  performance?: RolePerformance;
  insights: Insight[];
  focusTitle?: string | null;
}) {
  const { strengths, weaknesses } = topInsights(insights);

  return (
    <section aria-label="Summary" className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
      {performance ? (
        <>
          <Tile
            title="Role score"
            info={
              <>
                Pulled toward 50 while the sample is small (under {MODERATE_AT}{" "}
                games). Measured: {Math.round(performance.raw_performance)}.
              </>
            }
          >
            <div className="flex justify-center">
              <Gauge
                value={performance.performance}
                marker={
                  Math.round(performance.raw_performance) !==
                  Math.round(performance.performance)
                    ? performance.raw_performance
                    : null
                }
                label="Role score"
              />
            </div>
          </Tile>

          <Tile title="Confidence">
            <ConfidenceBar matches={performance.matches} />
          </Tile>

          <Tile title="Score made of">
            <StackedBar
              label="Score composition"
              segments={performance.components.map((c) => ({
                key: c.key,
                label: SHORT[c.key] ?? c.label,
                value: c.weight,
                display: `${Math.round(c.weight * 100)}`,
              }))}
            />
            <p className="line-clamp-2 font-mono text-[0.6875rem] leading-relaxed text-ink-faint">
              {performance.components
                .map((c) => `${SHORT[c.key] ?? c.label} ${formatComponentValue(c)}`)
                .join(" · ")}
            </p>
          </Tile>
        </>
      ) : null}

      <Tile title="This week">
        <ul className="flex flex-wrap gap-1.5">
          {strengths.map((i) => (
            <Chip key={`s-${i.title}`} tone="good" label={i.title} />
          ))}
          {weaknesses.map((i) => (
            <Chip key={`w-${i.title}`} tone="bad" label={i.title} />
          ))}
          {focusTitle ? <Chip tone="focus" label={`Focus: ${focusTitle}`} /> : null}
          {strengths.length + weaknesses.length === 0 && !focusTitle ? (
            <li className="text-xs text-ink-faint">No analysis yet</li>
          ) : null}
        </ul>
      </Tile>
    </section>
  );
}

const SHORT: Record<string, string> = {
  win_rate: "Win rate",
  kill_participation: "KP",
  kda: "KDA",
  deaths: "Deaths",
  deaths_per_10: "Deaths",
};

function Tile({
  title,
  info,
  children,
}: {
  title: string;
  info?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="glass flex min-w-0 flex-col gap-3 rounded-card p-4">
      <h3 className="flex items-center gap-1.5 text-[0.6875rem] font-semibold uppercase tracking-[0.16em] text-ink-faint">
        {title}
        {info ? <InfoTip label={`About ${title}`}>{info}</InfoTip> : null}
      </h3>
      {children}
    </div>
  );
}

/** Games toward the next confidence level, as a bar: "9 / 20 games". */
export function ConfidenceBar({ matches }: { matches: number }) {
  const goal = matches < MODERATE_AT ? MODERATE_AT : STRONG_AT;
  const level =
    matches < MODERATE_AT ? "Limited" : matches < STRONG_AT ? "Moderate" : "Strong";
  const share = Math.min(1, matches / goal);

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-baseline justify-between gap-2">
        <span className="font-display text-2xl text-ink">
          {matches}
          <span className="font-sans text-sm text-ink-faint">
            {" "}
            / {goal} games
          </span>
        </span>
        <span
          className={cn(
            "text-[0.625rem] uppercase tracking-wider",
            level === "Limited" ? "text-warn" : "text-string",
          )}
        >
          {level}
        </span>
      </div>
      <div
        role="meter"
        aria-label="Games toward the next confidence level"
        aria-valuenow={matches}
        aria-valuemin={0}
        aria-valuemax={goal}
        className="h-2 overflow-hidden rounded-full bg-mark-track"
      >
        <div
          className={cn("h-full rounded-full", level === "Limited" ? "bg-warn" : "bg-mark-win")}
          style={{ width: `${share * 100}%` }}
        />
      </div>
    </div>
  );
}

function Chip({ tone, label }: { tone: "good" | "bad" | "focus"; label: string }) {
  const style = {
    good: "border-string/40 bg-string/10 text-string",
    bad: "border-error/40 bg-error/10 text-error",
    focus: "border-warn/40 bg-warn/10 text-warn",
  }[tone];
  const glyph = { good: "▲", bad: "▼", focus: "◎" }[tone];
  return (
    <li
      title={label}
      className={cn("max-w-full truncate rounded-full border px-2.5 py-1 text-[0.6875rem]", style)}
    >
      <span aria-hidden className="mr-1">{glyph}</span>
      {label}
    </li>
  );
}
