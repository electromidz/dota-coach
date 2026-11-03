import { cn } from "@/lib/utils";

/**
 * A small full ring for one 0-1 share, with the value in the middle. Used
 * for fit scores and win rates on dense cards.
 */
export function Ring({
  share,
  label,
  text,
  tone = "you",
  size = 44,
}: {
  /** 0-1. */
  share: number;
  /** Names the ring for screen readers, e.g. "Fit score 72 of 100". */
  label: string;
  /** What is printed in the middle. */
  text: string;
  tone?: "you" | "good" | "bad" | "warn" | "neutral";
  size?: number;
}) {
  const r = 15.5;
  const c = 2 * Math.PI * r;
  const filled = Math.max(0, Math.min(1, share)) * c;
  const stroke = {
    you: "var(--color-you)",
    good: "var(--color-mark-win)",
    bad: "var(--color-mark-loss)",
    warn: "var(--color-warn)",
    neutral: "var(--color-rank)",
  }[tone];

  return (
    <span
      role="img"
      aria-label={label}
      className="relative inline-flex shrink-0 items-center justify-center"
      style={{ width: size, height: size }}
    >
      <svg viewBox="0 0 36 36" className="absolute inset-0 -rotate-90" aria-hidden>
        <circle cx="18" cy="18" r={r} fill="none" stroke="var(--color-mark-track)" strokeWidth="3.5" />
        <circle
          cx="18"
          cy="18"
          r={r}
          fill="none"
          stroke={stroke}
          strokeWidth="3.5"
          strokeLinecap="round"
          strokeDasharray={`${filled} ${c}`}
        />
      </svg>
      <span className={cn("font-mono tabular-nums text-ink", size >= 44 ? "text-xs" : "text-[0.625rem]")}>
        {text}
      </span>
    </span>
  );
}
