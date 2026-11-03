/**
 * A 0-100 score on a half ring, with an optional faint marker for a second
 * reading (e.g. the raw score before a small-sample adjustment).
 */
export function Gauge({
  value,
  marker,
  label,
  max = 100,
}: {
  value: number;
  /** A second reading on the same scale, drawn as a faint tick. */
  marker?: number | null;
  /** Names the gauge for screen readers. */
  label: string;
  max?: number;
}) {
  const R = 52;
  const C = 64;
  const clamp = (v: number) => Math.max(0, Math.min(max, v)) / max;
  // Half ring from 180deg (left) to 0deg (right).
  const point = (share: number, r = R) => {
    const angle = Math.PI * (1 - share);
    return [C + Math.cos(angle) * r, C - Math.sin(angle) * r] as const;
  };
  const arc = (share: number) => {
    const [x, y] = point(share);
    return `M ${C - R} ${C} A ${R} ${R} 0 0 1 ${x.toFixed(2)} ${y.toFixed(2)}`;
  };
  const shown = Math.round(value);

  return (
    <svg
      viewBox="0 0 128 74"
      className="w-full max-w-[11rem]"
      role="img"
      aria-label={`${label}: ${shown} of ${max}${
        marker != null ? `, measured ${Math.round(marker)}` : ""
      }.`}
    >
      <path
        d={arc(1)}
        fill="none"
        stroke="var(--color-mark-track)"
        strokeWidth={10}
        strokeLinecap="round"
      />
      {value > 0 ? (
        <path
          d={arc(clamp(value))}
          fill="none"
          stroke="var(--color-you)"
          strokeWidth={10}
          strokeLinecap="round"
        />
      ) : null}
      {marker != null ? (
        (() => {
          const [x1, y1] = point(clamp(marker), R - 9);
          const [x2, y2] = point(clamp(marker), R + 9);
          return (
            <line
              x1={x1}
              y1={y1}
              x2={x2}
              y2={y2}
              stroke="var(--color-ink-faint)"
              strokeWidth={2}
              strokeLinecap="round"
              strokeDasharray="2 2"
            />
          );
        })()
      ) : null}
      <text
        x={C}
        y={C - 4}
        textAnchor="middle"
        className="fill-ink font-display text-[26px]"
      >
        {shown}
      </text>
      <text x={C} y={C + 9} textAnchor="middle" className="fill-ink-faint text-[9px]">
        / {max}
      </text>
    </svg>
  );
}
