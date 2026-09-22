import type { TrajectoryPoint } from "@/lib/types";
import { cn } from "@/lib/utils";

const W = 320;
const H = 96;
const PAD_X = 10;
const PAD_Y = 14;

/**
 * Rank over time, with measured and modeled points rendered differently.
 *
 * **The distinction is the whole point of this chart.** The only real points
 * are the ones that came from a rank reading taken during a sync; everything
 * between two of them is a disclosed model, because Valve stopped publishing
 * per-match MMR. So the two never share an appearance:
 *
 *   * measured — solid segment, filled marker
 *   * modeled  — dashed segment, hollow marker
 *
 * That is deliberately a *shape* difference, not a colour one. One hue carries
 * the whole series, so certainty survives a colour-vision deficiency, a
 * greyscale print and a screenshot alike — the same rule `FormStrip` follows
 * for win and loss. The legend below names both states, and the accessible
 * description says how many of each there are.
 *
 * The y axis is `rank_tier` — `medal * 10 + stars`, so one unit is one star.
 * It is plotted against its own padded range rather than the full 11-80 ladder,
 * because nobody's trajectory spans Herald to Immortal and a fixed axis would
 * flatten every real one into a straight line.
 */
export function TrajectoryChart({
  points,
  className,
}: {
  /** Oldest first. */
  points: TrajectoryPoint[];
  className?: string;
}) {
  if (points.length < 2) {
    return (
      <p className={cn("text-sm text-ink-faint", className)}>
        One rank reading so far. A second gives this a shape — sync again
        tomorrow.
      </p>
    );
  }

  const tiers = points.map((p) => p.rank_tier);
  const min = Math.min(...tiers);
  const max = Math.max(...tiers);
  // A player who has not moved a star would divide by zero; give them a
  // centred band instead of a flat line pinned to the top of the frame.
  const span = max - min || 2;
  const lo = max === min ? min - 1 : min;

  const x = (i: number) => PAD_X + (i / (points.length - 1)) * (W - PAD_X * 2);
  const y = (tier: number) =>
    H - PAD_Y - ((tier - lo) / span) * (H - PAD_Y * 2);

  // One segment per adjacent pair, so a run of modeled points is dashed and
  // the two ends that touch a reading are not. Drawing a single polyline
  // could only be all-solid or all-dashed, which is exactly the claim this
  // chart must not make.
  const segments = points.slice(1).map((point, i) => {
    const previous = points[i];
    return {
      key: `${previous.at}-${point.at}`,
      x1: x(i),
      y1: y(previous.rank_tier),
      x2: x(i + 1),
      y2: y(point.rank_tier),
      // A segment is only as certain as its least certain end.
      estimated: previous.estimated || point.estimated,
    };
  });

  const measured = points.filter((p) => !p.estimated);
  const modeled = points.length - measured.length;

  return (
    <figure className={cn("m-0", className)}>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="h-24 w-full overflow-visible"
        role="img"
        aria-label={describe(points, measured.length, modeled)}
      >
        {segments.map((s) => (
          <line
            key={s.key}
            x1={s.x1}
            y1={s.y1}
            x2={s.x2}
            y2={s.y2}
            stroke="var(--color-mark-line)"
            strokeWidth={2}
            strokeLinecap="round"
            strokeDasharray={s.estimated ? "4 4" : undefined}
            strokeOpacity={s.estimated ? 0.6 : 1}
          />
        ))}

        {points.map((point, i) => (
          <circle
            key={point.at}
            cx={x(i)}
            cy={y(point.rank_tier)}
            // 4 gives the 8px minimum for a measured point; modeled ones sit a
            // step smaller so they read as the lighter claim they are.
            r={point.estimated ? 3 : 4}
            // Hollow for modeled: the surface shows through, so the marker is
            // an outline rather than a solid assertion.
            fill={
              point.estimated
                ? "var(--color-surface-2)"
                : "var(--color-mark-line)"
            }
            stroke={
              point.estimated
                ? "var(--color-mark-line)"
                : "var(--color-surface-2)"
            }
            strokeWidth={2}
            strokeOpacity={point.estimated ? 0.7 : 1}
          >
            <title>
              {`${tierName(point)} · ${formatDate(point.at)} · ${
                point.estimated ? "estimated" : "recorded rank"
              }`}
            </title>
          </circle>
        ))}
      </svg>

      <figcaption className="mt-3 flex flex-wrap items-center justify-between gap-x-4 gap-y-2 text-[0.6875rem] text-ink-faint">
        {/* Two states, so a legend is always present — identity is never
            carried by appearance alone. */}
        <ul className="flex items-center gap-4">
          <li className="flex items-center gap-1.5">
            <Swatch />
            Recorded rank
          </li>
          <li className="flex items-center gap-1.5">
            <Swatch estimated />
            Estimated
          </li>
        </ul>

        {/* Where the line runs from and to. Without this the reader sees a
            shape rising but not between which medals — the chart has no axis,
            by the same house convention `Sparkline` and `SeriesChart` follow,
            so the endpoints carry the scale. */}
        <span className="text-ink-muted">
          {tierName(points[0])}
          <span className="px-1 text-ink-faint">→</span>
          {tierName(points[points.length - 1])}
          <span className="ml-2 font-mono tabular-nums text-ink-faint">
            {formatDate(points[0].at)}–
            {formatDate(points[points.length - 1].at)}
          </span>
        </span>
      </figcaption>
    </figure>
  );
}

/** The legend mark, drawn with the same geometry the chart uses. */
function Swatch({ estimated = false }: { estimated?: boolean }) {
  return (
    <svg aria-hidden viewBox="0 0 24 10" className="h-2.5 w-6 shrink-0">
      <line
        x1={1}
        y1={5}
        x2={23}
        y2={5}
        stroke="var(--color-mark-line)"
        strokeWidth={2}
        strokeLinecap="round"
        strokeDasharray={estimated ? "4 4" : undefined}
        strokeOpacity={estimated ? 0.6 : 1}
      />
      <circle
        cx={12}
        cy={5}
        r={estimated ? 3 : 4}
        fill={estimated ? "var(--color-surface-2)" : "var(--color-mark-line)"}
        stroke={estimated ? "var(--color-mark-line)" : "var(--color-surface-2)"}
        strokeWidth={2}
        strokeOpacity={estimated ? 0.7 : 1}
      />
    </svg>
  );
}

/** The server names every medal; the raw tier is the last resort, not a
 *  second medal table living in the client. */
function tierName(point: TrajectoryPoint): string {
  return point.label ?? `Tier ${point.rank_tier}`;
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

/**
 * The description a screen reader gets.
 *
 * It states the measured/modeled split in words, because that distinction is
 * carried visually by line style — and a reader who never sees the line still
 * has to know which points are real.
 */
function describe(
  points: TrajectoryPoint[],
  measured: number,
  modeled: number,
): string {
  const first = points[0];
  const last = points[points.length - 1];

  const movement =
    last.rank_tier === first.rank_tier
      ? `held at ${tierName(first)}`
      : `from ${tierName(first)} to ${tierName(last)}`;

  return (
    `Rank over time, ${movement}, between ${formatDate(first.at)} and ${formatDate(last.at)}. ` +
    `${measured} recorded rank ${measured === 1 ? "reading" : "readings"}; ` +
    `${modeled} estimated ${modeled === 1 ? "point" : "points"} modeled between them.`
  );
}
