import { TableSkeleton } from "@/components/ui/DataTable";

/** Content-only placeholders: the admin header and tabs stay on screen. */

export function DashboardSkeleton() {
  return (
    <div className="flex flex-col gap-6" aria-busy="true">
      <span className="sr-only">Loading dashboard…</span>
      <div className="h-9 w-80 max-w-full animate-pulse rounded-lg bg-surface-2/60" />
      <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">
        {Array.from({ length: 6 }, (_, i) => (
          <div key={i} className="glass h-28 animate-pulse rounded-card" />
        ))}
      </div>
      <div className="grid gap-4 lg:grid-cols-3">
        <div className="glass h-80 animate-pulse rounded-card lg:col-span-2" />
        <div className="glass h-80 animate-pulse rounded-card" />
      </div>
    </div>
  );
}

export function ListSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true">
      <div className="h-11 w-full max-w-xl animate-pulse rounded-xl bg-surface-2/60" />
      <TableSkeleton columns={6} rows={8} />
    </div>
  );
}

export function DetailSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true">
      <span className="sr-only">Loading…</span>
      <div className="h-6 w-24 animate-pulse rounded bg-surface-2/60" />
      <div className="glass h-32 animate-pulse rounded-card" />
      <div className="glass h-40 animate-pulse rounded-card" />
      <div className="glass h-64 animate-pulse rounded-card" />
    </div>
  );
}
