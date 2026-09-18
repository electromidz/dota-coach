"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { accountStatusLabel, displayName, planLabel } from "@/lib/admin";
import { ApiError, getAdminUsers } from "@/lib/api";
import type { AdminUserListResponse, AdminUserSummary, SubscriptionStatus } from "@/lib/types";
import { cn } from "@/lib/utils";

const PAGE_SIZE = 20;

const STATUS_OPTIONS: Array<{ value: ""; label: string } | { value: "active" | "disabled"; label: string }> = [
  { value: "", label: "Any status" },
  { value: "active", label: "Active" },
  { value: "disabled", label: "Disabled" },
];

const PLAN_OPTIONS: Array<{ value: "" | SubscriptionStatus; label: string }> = [
  { value: "", label: "Any plan" },
  { value: "trialing", label: "Trial" },
  { value: "active", label: "Active" },
  { value: "expired", label: "Expired" },
  { value: "cancelled", label: "Cancelled" },
  { value: "past_due", label: "Past due" },
];

export function UserTable() {
  const [data, setData] = useState<AdminUserListResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [status, setStatus] = useState("");
  const [plan, setPlan] = useState("");
  const [search, setSearch] = useState("");

  const load = useCallback(
    async (targetPage: number, targetStatus: string, targetPlan: string, targetSearch: string) => {
      try {
        setData(
          await getAdminUsers({
            page: targetPage,
            limit: PAGE_SIZE,
            status: targetStatus || undefined,
            plan: targetPlan || undefined,
            search: targetSearch || undefined,
          }),
        );
        setError(null);
      } catch (e) {
        setError(e instanceof ApiError ? e.message : "Could not load the user list.");
      }
    },
    [],
  );

  useEffect(() => {
    void load(1, "", "", "");
  }, [load]);

  function applyFilters(e: React.FormEvent) {
    e.preventDefault();
    setPage(1);
    void load(1, status, plan, search);
  }

  function goTo(target: number) {
    setPage(target);
    void load(target, status, plan, search);
  }

  return (
    <div className="flex flex-col gap-4 pb-4">
      <form
        onSubmit={applyFilters}
        className="flex flex-wrap items-end gap-3"
        aria-label="Filter accounts"
      >
        <label className="flex flex-col gap-1 text-xs text-ink-faint">
          Search
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Persona name or SteamID64"
            className="min-h-11 w-56 rounded-xl border border-glass-edge bg-surface-2/60 px-3 text-sm text-ink outline-none focus-neon"
          />
        </label>

        <label className="flex flex-col gap-1 text-xs text-ink-faint">
          Status
          <select
            value={status}
            onChange={(e) => setStatus(e.target.value)}
            className="min-h-11 rounded-xl border border-glass-edge bg-surface-2/60 px-3 text-sm text-ink outline-none focus-neon"
          >
            {STATUS_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>

        <label className="flex flex-col gap-1 text-xs text-ink-faint">
          Plan
          <select
            value={plan}
            onChange={(e) => setPlan(e.target.value)}
            className="min-h-11 rounded-xl border border-glass-edge bg-surface-2/60 px-3 text-sm text-ink outline-none focus-neon"
          >
            {PLAN_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>

        <Button type="submit" variant="ghost" className="px-5 text-sm">
          Apply
        </Button>
      </form>

      {error ? <Alert>{error}</Alert> : null}

      {!data ? (
        <TableSkeleton />
      ) : data.users.length === 0 ? (
        <Card>
          <p className="text-sm text-ink-muted">No accounts match these filters.</p>
        </Card>
      ) : (
        <>
          <p className="font-mono text-xs tabular-nums text-ink-faint" aria-live="polite">
            {data.total} account{data.total === 1 ? "" : "s"}
          </p>

          <ul className="flex flex-col gap-2">
            {data.users.map((user) => (
              <UserRow key={user.id} user={user} />
            ))}
          </ul>

          {data.total_pages > 1 ? (
            <nav
              aria-label="User list pages"
              className="flex items-center justify-between gap-3 pt-1 lg:justify-center lg:gap-6"
            >
              <Button
                variant="ghost"
                disabled={page <= 1}
                onClick={() => goTo(page - 1)}
                className="px-4 text-sm"
              >
                <Icon name="chevron-left" className="size-4" />
                Prev
              </Button>
              <span className="font-mono text-xs tabular-nums text-ink-faint">
                {page} / {data.total_pages}
              </span>
              <Button
                variant="ghost"
                disabled={page >= data.total_pages}
                onClick={() => goTo(page + 1)}
                className="px-4 text-sm"
              >
                Next
                <Icon name="chevron-right" className="size-4" />
              </Button>
            </nav>
          ) : null}
        </>
      )}
    </div>
  );
}

function UserRow({ user }: { user: AdminUserSummary }) {
  return (
    <li>
      <Link href={`/admin/users/${user.id}`}>
        <Card className="flex flex-wrap items-center justify-between gap-3 p-4 transition-colors duration-200 ease-out hover:border-function/50">
          <div className="min-w-0">
            <p className="truncate text-sm text-ink">
              {displayName(user)}
              {user.is_admin ? (
                <span className="ml-2 text-[0.6875rem] uppercase tracking-wider text-keyword">
                  Admin
                </span>
              ) : null}
            </p>
            <p className="mt-0.5 font-mono text-xs tabular-nums text-ink-faint">
              {user.steam_id}
            </p>
          </div>

          <div className="flex shrink-0 items-center gap-3">
            <span
              className={cn(
                "rounded-lg border px-2.5 py-1 text-xs",
                user.status === "disabled"
                  ? "border-error/40 text-error"
                  : "border-glass-edge text-ink-faint",
              )}
            >
              {accountStatusLabel(user.status)}
            </span>
            <span className="rounded-lg border border-glass-edge px-2.5 py-1 text-xs text-ink-faint">
              {planLabel(user.subscription_status)}
            </span>
          </div>
        </Card>
      </Link>
    </li>
  );
}

function TableSkeleton() {
  return (
    <div className="flex flex-col gap-2" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading accounts…</span>
      {[0, 1, 2, 3, 4].map((i) => (
        <div key={i} className="glass h-16 animate-pulse rounded-card" />
      ))}
    </div>
  );
}
