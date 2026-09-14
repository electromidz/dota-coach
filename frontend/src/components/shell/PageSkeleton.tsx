/**
 * The shared loading shape for a whole route.
 *
 * Every feature component already has its own skeleton for the moment its data
 * is in flight; this is the earlier moment, before the route's code has even
 * arrived. Same language — pulsing panels at roughly the size of the real
 * thing — so the transition between the two is not a second visible jump.
 */
export function PageSkeleton({ rows = 3 }: { rows?: number }) {
  // Descending heights read as "a header, then content", which is what almost
  // every screen in this app actually is.
  const heights = ["h-28", "h-40", "h-64", "h-40", "h-32"];

  return (
    <div className="flex flex-col gap-4" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading…</span>
      {Array.from({ length: rows }, (_, index) => (
        <div
          key={index}
          className={`${heights[index % heights.length]} animate-pulse rounded-card bg-surface-2`}
        />
      ))}
    </div>
  );
}
