import type { Momentum } from "@/lib/types";
import { cn } from "@/lib/utils";

const W = 320;
const H = 132;
const PAD_X = 12;
/** Vertical room for a delta label above the highest point and below the
 *  lowest, so neither is clipped by the frame. */
const PAD_Y = 26;

/** Distance from a marker to its delta label, in viewBox units. */
const LABEL_GAP = 7;

/**
 * Modeled MMR movement across the most recent ranked matches.
 *
 * The form other rank trackers use for this — a cumulative line over the last
 * twenty games — because it is the right one: it answers "which way am I
 * going, and how hard" in a shape, where a row of W/L squares makes you count
 * it out yourself.
 *
 * What this does differently is the **axis origin**. The curve starts at zero
 * and plots movement since; it never shows an absolute rating. The win/loss
 * sequence behind it is real, but the per-match magnitude is the server's
 * disclosed model, and Valve publishes no per-match MMR that could confirm it.
 * "+85 across your last 20 ranked games" is arithmetic over results the player
 * actually got. "Your MMR is 4230" would be a specific claim about a figure
 * nobody outside Valve can see, and it is the one thing this product does not
 * do.
 *
 * Direction is status, not category, so the fill and the net figure carry the
 * same win/loss semantics `FormStrip` uses, and the net is labelled in words
 * as well as colour.
 *
 * Every point carries its own delta, which is a deliberate departure from the
 * house rule of labelling selectively — a number on every point is usually
 * noise. Here it is the content: the per-match movement is what the chart is
 * *about*, and burying it in a hover tooltip hides it from touch entirely.
 * Twenty of them stay legible because a win's label sits above its point and a
 * loss's below, so two labels only crowd when two consecutive matches went the
 * same way, and the alternating offset breaks up those runs.
 */
export function MomentumChart({
  momentum,
  className,
}: {
  momentum: Momentum;
  className?: string;
}) {
  const { points } = momentum;

  if (points.length < 2) {
    return (
      <p className={cn("text-sm text-ink-faint", className)}>
        {points.length === 0
          ? "No recent ranked matches to plot."
          : "One ranked match so far. A second gives this a shape."}
      </p>
    );
  }

  const values = points.map((p) => p.cumulative);
  // Zero is always on the axis: the whole point is which side of it the player
  // is on, and a window that only ever climbed would otherwise look like it
  // started from its own first loss.
  const max = Math.max(0, ...values);
  const min = Math.min(0, ...values);
  const span = max - min || 1;

  const x = (i: number) => PAD_X + (i / (points.length - 1)) * (W - PAD_X * 2);
  const y = (v: number) => H - PAD_Y - ((v - min) / span) * (H - PAD_Y * 2);

  const zero = y(0);
  const line = points.map((p, i) => `${x(i).toFixed(1)},${y(p.cumulative).toFixed(1)}`);
  // Closed against the zero line rather than the frame bottom, so the wash
  // reads as "above/below where you started".
  const area = `${x(0)},${zero} ${line.join(" ")} ${x(points.length - 1)},${zero}`;

  const up = momentum.net >= 0;
  const stroke = up ? "var(--color-mark-win)" : "var(--color-mark-loss)";

  return (
    <figure className={cn("m-0", className)}>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="h-[110px] w-full overflow-visible"
        role="img"
        aria-label={describe(momentum)}
      >
        {/* The baseline the whole chart is relative to. */}
        <line
          x1={PAD_X}
          x2={W - PAD_X}
          y1={zero}
          y2={zero}
          stroke="var(--color-ink-faint)"
          strokeOpacity={0.4}
          strokeDasharray="3 3"
          strokeWidth={1}
        />

        <polyline points={area} fill={stroke} fillOpacity={0.12} stroke="none" />
        <polyline
          points={line.join(" ")}
          fill="none"
          stroke={stroke}
          strokeWidth={2}
          strokeLinecap="round"
          strokeLinejoin="round"
        />

        {points.map((p, i) => (
          <circle
            key={p.match_id}
            cx={x(i)}
            cy={y(p.cumulative)}
            r={i === points.length - 1 ? 4 : 2.5}
            fill={p.won ? "var(--color-mark-win)" : "var(--color-mark-loss)"}
            stroke="var(--color-surface-2)"
            strokeWidth={1.5}
          >
            <title>
              {`${p.won ? "Win" : "Loss"} · ${p.hero_name} · ${formatSigned(
                p.delta,
              )} modeled · running ${formatSigned(p.cumulative)}`}
            </title>
          </circle>
        ))}

        {/* Each match's own modeled movement, on the side it moved the curve:
            a win's label sits above its point, a loss's below. Putting them on
            opposite sides is what keeps twenty of them legible — consecutive
            labels only collide when consecutive matches went the same way, and
            the offset alternation below breaks those up too. */}
        {points.map((p, i) => (
          <text
            key={`d-${p.match_id}`}
            x={x(i)}
            y={
              p.won
                ? y(p.cumulative) - LABEL_GAP - (i % 2) * 7
                : y(p.cumulative) + LABEL_GAP + 4 + (i % 2) * 7
            }
            textAnchor="middle"
            className="font-mono"
            fontSize={8}
            fill={p.won ? "var(--color-string)" : "var(--color-error)"}
          >
            {formatSigned(p.delta)}
          </text>
        ))}
      </svg>

      <figcaption className="mt-3 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 text-xs text-ink-faint">
        <span>
          Last {points.length} ranked {points.length === 1 ? "match" : "matches"}
          <span className="px-1">·</span>
          <span className="text-string">{momentum.wins}W</span>
          <span className="px-0.5">/</span>
          <span className="text-error">{momentum.losses}L</span>
        </span>

        <span className={cn("font-semibold", up ? "text-string" : "text-error")}>
          {formatSigned(momentum.net)}
          <span className="ml-1 font-normal text-ink-faint">
            modeled, {up ? "up" : "down"} from where the window started
          </span>
        </span>
      </figcaption>
    </figure>
  );
}

/** Always signed: the sign is the information. */
function formatSigned(value: number): string {
  const rounded = Math.round(value);
  return `${rounded >= 0 ? "+" : "−"}${Math.abs(rounded)}`;
}

function describe(momentum: Momentum): string {
  const direction = momentum.net >= 0 ? "up" : "down";

  return (
    `Modeled MMR movement across the last ${momentum.points.length} ranked matches, ` +
    `${momentum.wins} won and ${momentum.losses} lost: ` +
    `${direction} ${Math.abs(Math.round(momentum.net))} from where the window started. ` +
    `A modeled figure, relative to the start of the window — not an absolute rating.`
  );
}
