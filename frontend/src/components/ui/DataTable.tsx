"use client";

import { Button } from "@/components/ui/Button";
import { Icon } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

export interface Column<Row, SortKey extends string = string> {
  id: string;
  header: string;
  cell: (row: Row) => React.ReactNode;
  /** Present when the column can be sorted, and what to sort it by. */
  sortKey?: SortKey;
  align?: "left" | "right";
  className?: string;
  /** The row's name: rendered as a row header for screen readers. */
  rowHeader?: boolean;
}

export interface SortState<SortKey extends string> {
  key: SortKey;
  dir: "asc" | "desc";
}

/**
 * A server-backed table: the rows on screen are one page, and sorting and
 * paging ask the server again rather than shuffling what is already here.
 *
 * Sorting is announced through `aria-sort`, and every header button says
 * what clicking it will do. While `loading`, the previous rows stay visible
 * (dimmed) instead of collapsing to a skeleton, so the page does not jump.
 */
export function DataTable<Row, SortKey extends string = string>({
  caption,
  columns,
  rows,
  rowKey,
  sort,
  onSort,
  page,
  totalPages,
  onPage,
  loading = false,
  empty,
}: {
  caption: string;
  columns: Column<Row, SortKey>[];
  /** `null` before the first load. */
  rows: Row[] | null;
  rowKey: (row: Row) => string;
  sort?: SortState<SortKey>;
  onSort?: (next: SortState<SortKey>) => void;
  page?: number;
  totalPages?: number;
  onPage?: (page: number) => void;
  loading?: boolean;
  empty: React.ReactNode;
}) {
  if (rows === null) return <TableSkeleton columns={columns.length} />;

  return (
    <div className="flex flex-col gap-3">
      <div
        className={cn(
          "glass overflow-x-auto rounded-card transition-opacity duration-200",
          loading && "opacity-60",
        )}
        aria-busy={loading}
      >
        <table className="w-full min-w-[44rem] border-collapse text-left text-sm">
          <caption className="sr-only">{caption}</caption>
          <thead>
            <tr className="border-b border-glass-edge text-xs text-ink-faint">
              {columns.map((column) => {
                const sorted = sort && column.sortKey === sort.key ? sort.dir : null;
                return (
                  <th
                    key={column.id}
                    scope="col"
                    aria-sort={
                      sorted === "asc" ? "ascending" : sorted === "desc" ? "descending" : undefined
                    }
                    className={cn(
                      "whitespace-nowrap px-4 py-3 font-normal",
                      column.align === "right" && "text-right",
                    )}
                  >
                    {column.sortKey && onSort ? (
                      <button
                        type="button"
                        onClick={() =>
                          onSort({
                            key: column.sortKey as SortKey,
                            dir: sorted === "desc" ? "asc" : "desc",
                          })
                        }
                        className={cn(
                          "focus-neon -mx-1 inline-flex cursor-pointer items-center gap-1 rounded px-1 transition-colors hover:text-ink",
                          sorted && "text-ink",
                        )}
                      >
                        {column.header}
                        <Icon
                          name={sorted === "asc" ? "chevron-up" : "chevron-down"}
                          className={cn("size-3.5", !sorted && "opacity-30")}
                        />
                        <span className="sr-only">
                          {sorted === "desc"
                            ? ", sorted descending. Activate to sort ascending."
                            : sorted === "asc"
                              ? ", sorted ascending. Activate to sort descending."
                              : ". Activate to sort."}
                        </span>
                      </button>
                    ) : (
                      column.header
                    )}
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 ? (
              <tr>
                <td colSpan={columns.length} className="px-4 py-10 text-center text-sm text-ink-muted">
                  {empty}
                </td>
              </tr>
            ) : (
              rows.map((row) => (
                <tr
                  key={rowKey(row)}
                  className="border-b border-glass-edge/60 transition-colors last:border-b-0 hover:bg-surface-2/40"
                >
                  {columns.map((column) => {
                    const Cell = column.rowHeader ? "th" : "td";
                    return (
                      <Cell
                        key={column.id}
                        scope={column.rowHeader ? "row" : undefined}
                        className={cn(
                          "px-4 py-3 align-middle font-normal",
                          column.align === "right" && "text-right",
                          column.className,
                        )}
                      >
                        {column.cell(row)}
                      </Cell>
                    );
                  })}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {page !== undefined && totalPages !== undefined && totalPages > 1 && onPage ? (
        <nav
          aria-label={`${caption} pages`}
          className="flex items-center justify-between gap-3 lg:justify-center lg:gap-6"
        >
          <Button
            variant="ghost"
            disabled={page <= 1 || loading}
            onClick={() => onPage(page - 1)}
            className="px-4 text-sm"
          >
            <Icon name="chevron-left" className="size-4" />
            Prev
          </Button>
          <span className="font-mono text-xs tabular-nums text-ink-faint" aria-live="polite">
            Page {page} of {totalPages}
          </span>
          <Button
            variant="ghost"
            disabled={page >= totalPages || loading}
            onClick={() => onPage(page + 1)}
            className="px-4 text-sm"
          >
            Next
            <Icon name="chevron-right" className="size-4" />
          </Button>
        </nav>
      ) : null}
    </div>
  );
}

export function TableSkeleton({ columns = 5, rows = 6 }: { columns?: number; rows?: number }) {
  return (
    <div className="glass flex flex-col gap-3 rounded-card p-4" aria-busy="true">
      <span className="sr-only">Loading…</span>
      <div className="h-4 w-1/3 animate-pulse rounded bg-surface-2/70" />
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="grid gap-4" style={{ gridTemplateColumns: `repeat(${columns}, 1fr)` }}>
          {Array.from({ length: columns }, (_, j) => (
            <div key={j} className="h-5 animate-pulse rounded bg-surface-2/50" />
          ))}
        </div>
      ))}
    </div>
  );
}
