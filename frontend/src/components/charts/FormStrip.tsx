import { cn } from "@/lib/utils";

/**
 * Recent results, oldest to newest.
 *
 * Win and loss are *status*, not a category pair, so each cell carries its
 * initial as well as its colour — the state survives a colour-vision
 * deficiency, a greyscale print and a screenshot alike. Cells are separated by
 * a surface gap rather than a border.
 */
export function FormStrip({
  results,
  className,
}: {
  /** Oldest first. */
  results: boolean[];
  className?: string;
}) {
  if (results.length === 0) {
    return <p className={cn("text-sm text-ink-faint", className)}>No matches yet.</p>;
  }

  const wins = results.filter(Boolean).length;

  return (
    <figure className={cn("m-0", className)}>
      <ol
        className="flex gap-0.5"
        aria-label={`Last ${results.length} matches, oldest first: ${wins} wins, ${results.length - wins} losses.`}
      >
        {results.map((won, i) => (
          <li
            // Results are a positional sequence with no stable id; the index
            // is the identity here.
            key={i}
            title={won ? "Win" : "Loss"}
            className={cn(
              "flex h-7 flex-1 items-center justify-center rounded text-[0.625rem] font-semibold",
              // 15% keeps both labels well clear of 4.5:1 against their own
              // tinted cell (measured: win 7.0:1, loss 5.2:1).
              won
                ? "bg-mark-win/15 text-string"
                : "bg-mark-loss/15 text-error",
            )}
          >
            {won ? "W" : "L"}
          </li>
        ))}
      </ol>
    </figure>
  );
}
