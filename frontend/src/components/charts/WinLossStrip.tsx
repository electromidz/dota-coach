/**
 * The last games as a row of marks, oldest to newest: win (teal, tall) and
 * loss (red, short). Height differs too, so it reads without colour.
 */
export function WinLossStrip({ results, label }: { results: boolean[]; label: string }) {
  const wins = results.filter(Boolean).length;
  return (
    <span
      role="img"
      aria-label={`${label}: ${wins} won, ${results.length - wins} lost, oldest first`}
      className="inline-flex h-3.5 items-end gap-0.5"
    >
      {results.map((won, i) => (
        <span
          key={i}
          className={won ? "h-3.5 w-1.5 rounded-sm bg-mark-win" : "h-2 w-1.5 rounded-sm bg-mark-loss"}
        />
      ))}
    </span>
  );
}
