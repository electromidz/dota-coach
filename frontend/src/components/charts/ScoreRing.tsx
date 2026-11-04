import { cn, ordinal } from "@/lib/utils";

/**
 * One 0-100 score as a large ring, with an optional second reading marked on
 * the same track (a hollow dot), e.g. this game against your average.
 */
export function ScoreRing({
  score,
  marker,
  markerLabel,
  label,
  caption,
  size = 104,
}: {
  /** 0-100. */
  score: number;
  /** 0-100, drawn as a hollow dot on the track. */
  marker?: number | null;
  /** Names the marker, e.g. "your avg". */
  markerLabel?: string;
  /** Names the ring for screen readers. */
  label: string;
  /** Small text under the number, e.g. "vs Legend". */
  caption?: string;
  size?: number;
}) {
  const r = 15.5;
  const c = 2 * Math.PI * r;
  const clamp = (v: number) => Math.max(0, Math.min(100, v));
  const filled = (clamp(score) / 100) * c;
  const rounded = Math.round(score);
  const stroke =
    rounded >= 70
      ? "var(--color-mark-win)"
      : rounded <= 30
        ? "var(--color-mark-loss)"
        : "var(--color-you)";

  // The ring starts at 12 o'clock and runs clockwise.
  const markAt =
    marker !== null && marker !== undefined
      ? (() => {
          const a = -Math.PI / 2 + (clamp(marker) / 100) * Math.PI * 2;
          return { x: 18 + Math.cos(a) * r, y: 18 + Math.sin(a) * r };
        })()
      : null;

  const aria = [label, `${ordinal(score)} percentile`];
  if (marker !== null && marker !== undefined && markerLabel) {
    aria.push(`${markerLabel} ${ordinal(marker)}`);
  }

  return (
    <div className="flex shrink-0 flex-col items-center gap-1">
      <span
        role="img"
        aria-label={aria.join(", ")}
        className="relative inline-flex items-center justify-center"
        style={{ width: size, height: size }}
      >
        <svg viewBox="0 0 36 36" className="absolute inset-0 overflow-visible" aria-hidden>
          <circle cx="18" cy="18" r={r} fill="none" stroke="var(--color-mark-track)" strokeWidth="3" />
          <circle
            cx="18"
            cy="18"
            r={r}
            fill="none"
            stroke={stroke}
            strokeWidth="3"
            strokeLinecap="round"
            strokeDasharray={`${filled} ${c}`}
            transform="rotate(-90 18 18)"
          />
          {markAt ? (
            <circle
              cx={markAt.x}
              cy={markAt.y}
              r={2.1}
              fill="var(--color-surface-2)"
              stroke="var(--color-ink-muted)"
              strokeWidth={1.1}
            />
          ) : null}
        </svg>
        <span className="flex flex-col items-center leading-none">
          <span
            className={cn(
              "font-display text-2xl tabular-nums",
              rounded >= 70 ? "text-string" : rounded <= 30 ? "text-error" : "text-ink",
            )}
          >
            {rounded}
            <span className="text-sm text-ink-faint">%</span>
          </span>
          {caption ? (
            <span className="mt-1 text-[0.5625rem] uppercase tracking-wider text-ink-faint">
              {caption}
            </span>
          ) : null}
        </span>
      </span>
      {marker !== null && marker !== undefined && markerLabel ? (
        <span className="flex items-center gap-1 font-mono text-[0.625rem] tabular-nums text-ink-faint">
          <span aria-hidden className="size-2 rounded-full border border-ink-muted bg-surface-2" />
          {markerLabel} p{Math.round(marker)}
        </span>
      ) : null}
    </div>
  );
}
