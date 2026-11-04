import type { ItemTiming, TimingVerdict } from "@/lib/types";
import { cn, formatDuration } from "@/lib/utils";

const DOT: Record<TimingVerdict, string> = {
  late: "bg-mark-loss",
  early: "bg-mark-win",
  on_time: "bg-you",
};

const WORD: Record<TimingVerdict, string> = {
  late: "late",
  early: "early",
  on_time: "on time",
};

const SHORT: Record<ItemTiming["item"], string> = {
  black_king_bar: "BKB",
  blink_dagger: "Blink",
  hand_of_midas: "Midas",
};

/**
 * Core items on one time axis: a filled dot when this match completed it,
 * a hollow ghost at the peer median, a line between them. Red is late, green
 * early; the verdict is also printed, so colour is never the only signal.
 */
export function ItemTimeline({
  timings,
  durationSeconds,
  heroName,
}: {
  timings: ItemTiming[];
  durationSeconds: number;
  heroName: string;
}) {
  const end = Math.max(
    durationSeconds,
    ...timings.map((t) => Math.max(t.seconds, t.peer_median_seconds ?? 0)),
  );
  const at = (s: number) => `${Math.max(1, Math.min(99, (s / end) * 100))}%`;
  const ticks = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90].filter((m) => m * 60 <= end);

  return (
    <div className="flex flex-col gap-3">
      <ul className="m-0 flex list-none flex-col gap-3 p-0">
        {timings.map((t) => {
          const median = t.peer_median_seconds;
          const from = median !== null ? Math.min(t.seconds, median) : t.seconds;
          const to = median !== null ? Math.max(t.seconds, median) : t.seconds;
          return (
            <li key={t.item} className="grid grid-cols-[3rem_1fr_7rem] items-center gap-3">
              <span className="truncate text-xs text-ink-muted" title={t.label}>
                {SHORT[t.item]}
              </span>
              <div
                role="img"
                aria-label={
                  median !== null && t.verdict
                    ? `${t.label} at ${formatDuration(t.seconds)}, peer median on ${heroName} ${formatDuration(median)}, ${WORD[t.verdict]}.`
                    : `${t.label} at ${formatDuration(t.seconds)}, not compared.`
                }
                className="relative h-5"
              >
                <div className="absolute inset-x-0 top-1/2 h-px -translate-y-1/2 bg-mark-track" />
                {median !== null ? (
                  <>
                    <span
                      aria-hidden
                      className={cn(
                        "absolute top-1/2 h-0.5 -translate-y-1/2",
                        t.verdict === "late"
                          ? "bg-mark-loss/50"
                          : t.verdict === "early"
                            ? "bg-mark-win/50"
                            : "bg-you/40",
                      )}
                      style={{ left: at(from), width: `calc(${at(to)} - ${at(from)})` }}
                    />
                    <span
                      aria-hidden
                      title={`Peer median ${formatDuration(median)}`}
                      className="absolute top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-dashed border-ink-faint bg-surface-2"
                      style={{ left: at(median) }}
                    />
                  </>
                ) : null}
                <span
                  aria-hidden
                  className={cn(
                    "absolute top-1/2 size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-surface-2",
                    t.verdict ? DOT[t.verdict] : "bg-mark-line",
                  )}
                  style={{ left: at(t.seconds) }}
                />
              </div>
              <span className="flex flex-col items-end whitespace-nowrap font-mono text-[0.6875rem] tabular-nums leading-tight">
                <span className="text-ink">{formatDuration(t.seconds)}</span>
                <span
                  className={cn(
                    t.verdict === "late"
                      ? "text-error"
                      : t.verdict === "early"
                        ? "text-string"
                        : "text-ink-faint",
                  )}
                >
                  {median !== null && t.verdict
                    ? `${WORD[t.verdict]} · p50 ${formatDuration(median)}`
                    : "not compared"}
                </span>
              </span>
            </li>
          );
        })}
      </ul>

      {/* Minute axis, aligned with the track column above. */}
      <div className="grid grid-cols-[3rem_1fr_7rem] gap-3" aria-hidden>
        <span />
        <div className="relative h-3 font-mono text-[0.5625rem] text-ink-faint">
          {ticks.map((m) => (
            <span
              key={m}
              className="absolute -translate-x-1/2"
              style={{ left: at(m * 60) }}
            >
              {m}
            </span>
          ))}
        </div>
        <span className="text-right text-[0.5625rem] text-ink-faint">min</span>
      </div>

      <p className="flex flex-wrap gap-3 text-[0.6875rem] text-ink-faint">
        <span className="inline-flex items-center gap-1.5">
          <span aria-hidden className="size-2.5 rounded-full bg-you" />
          this match
        </span>
        <span className="inline-flex items-center gap-1.5">
          <span aria-hidden className="size-2.5 rounded-full border-2 border-dashed border-ink-faint" />
          peer median, all ranks
        </span>
      </p>
    </div>
  );
}
