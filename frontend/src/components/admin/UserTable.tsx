"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { CopyButton } from "@/components/ui/CopyButton";
import { DataTable, type Column } from "@/components/ui/DataTable";
import { Icon } from "@/components/ui/Icon";
import { RelativeTime } from "@/components/ui/RelativeTime";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { ApiError } from "@/lib/api";
import {
  accountStatusLabel,
  displayName,
  planName,
  subscriptionLabel,
  subscriptionTone,
} from "@/lib/admin/format";
import { listUsers } from "@/lib/admin/queries";
import type {
  AccountStatus,
  AdminUser,
  AdminUserList,
  SortDirection,
  UserSortKey,
} from "@/lib/admin/types";
import { useDebounced, useUrlState } from "@/lib/admin/url-state";
import type { SubscriptionStatus } from "@/lib/types";

const PAGE_SIZE = 20;

const SORT_KEYS: UserSortKey[] = ["name", "steam_id", "status", "subscription", "plan", "joined", "last_seen"];

const STATUS_OPTIONS: { value: "" | AccountStatus; label: string }[] = [
  { value: "", label: "Any account status" },
  { value: "active", label: "Active accounts" },
  { value: "disabled", label: "Disabled accounts" },
];

const SUBSCRIPTION_OPTIONS: { value: "" | SubscriptionStatus; label: string }[] = [
  { value: "", label: "Any subscription" },
  { value: "trialing", label: "Trial" },
  { value: "active", label: "Paid" },
  { value: "expired", label: "Expired" },
  { value: "cancelled", label: "Cancelled" },
  { value: "past_due", label: "Past due" },
];

const SELECT =
  "min-h-11 rounded-xl border border-glass-edge bg-surface-2/60 px-3 text-sm text-ink outline-none focus-neon";

const COLUMNS: Column<AdminUser, UserSortKey>[] = [
  {
    id: "name",
    header: "Name",
    sortKey: "name",
    rowHeader: true,
    cell: (u) => (
      <Link
        href={`/admin/users/${u.id}`}
        className="focus-neon flex min-w-0 items-center gap-2.5 rounded text-ink hover:text-function"
      >
        {u.avatar_url ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img src={u.avatar_url} alt="" className="size-7 shrink-0 rounded-full border border-glass-edge" />
        ) : (
          <span className="flex size-7 shrink-0 items-center justify-center rounded-full border border-glass-edge bg-surface-2 text-xs text-ink-faint">
            {displayName(u).slice(0, 1).toUpperCase()}
          </span>
        )}
        <span className="max-w-48 truncate">{displayName(u)}</span>
        {u.is_admin ? <StatusBadge tone="accent">Admin</StatusBadge> : null}
      </Link>
    ),
  },
  {
    id: "steam_id",
    header: "SteamID64",
    sortKey: "steam_id",
    cell: (u) => (
      <span className="flex items-center gap-1 font-mono text-xs tabular-nums text-ink-muted">
        {u.steam_id}
        <CopyButton value={u.steam_id} label={`SteamID64 of ${displayName(u)}`} />
      </span>
    ),
  },
  {
    id: "status",
    header: "Account",
    sortKey: "status",
    cell: (u) => (
      <StatusBadge label="Account status" tone={u.status === "disabled" ? "bad" : "neutral"}>
        {accountStatusLabel(u.status)}
      </StatusBadge>
    ),
  },
  {
    id: "subscription",
    header: "Subscription",
    sortKey: "subscription",
    cell: (u) => (
      <StatusBadge label="Subscription status" tone={subscriptionTone(u.subscription_status)}>
        {subscriptionLabel(u.subscription_status)}
      </StatusBadge>
    ),
  },
  {
    id: "plan",
    header: "Plan",
    sortKey: "plan",
    cell: (u) => <span className="text-ink-muted">{planName(u.subscription_plan)}</span>,
  },
  {
    id: "joined",
    header: "Joined",
    sortKey: "joined",
    cell: (u) => <RelativeTime iso={u.created_at} className="text-ink-muted" />,
  },
  {
    id: "last_seen",
    header: "Last seen",
    sortKey: "last_seen",
    cell: (u) =>
      u.last_login_at ? (
        <RelativeTime iso={u.last_login_at} className="text-ink-muted" />
      ) : (
        <span className="text-ink-faint">Never</span>
      ),
  },
];

/**
 * Every account, as a sortable table. Search, filters, sort and page all live
 * in the URL and apply as they change; search waits for typing to pause.
 */
export function UserTable() {
  const [params, setParams] = useUrlState();

  const q = params.get("q") ?? "";
  const status = (params.get("status") ?? "") as "" | AccountStatus;
  const plan = (params.get("plan") ?? "") as "" | SubscriptionStatus;
  const sortParam = params.get("sort") as UserSortKey | null;
  const sort: UserSortKey = sortParam && SORT_KEYS.includes(sortParam) ? sortParam : "joined";
  const dir: SortDirection = params.get("dir") === "asc" ? "asc" : "desc";
  const page = Math.max(1, Number(params.get("page")) || 1);

  // The box is local so typing is instant; the URL follows once it settles.
  const [search, setSearch] = useState(q);
  const settled = useDebounced(search, 300);
  useEffect(() => {
    if (settled.trim() !== q) setParams({ q: settled.trim(), page: null });
    // Only a settled edit should write; `q` changing from the URL must not loop.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settled]);

  const key = JSON.stringify({ q, status, plan, sort, dir, page });
  const [result, setResult] = useState<{ key: string; data: AdminUserList } | null>(null);
  const [failure, setFailure] = useState<{ key: string; message: string } | null>(null);

  useEffect(() => {
    let cancelled = false;
    listUsers({ page, limit: PAGE_SIZE, status, plan, search: q || undefined, sort, dir })
      .then((data) => !cancelled && setResult({ key, data }))
      .catch((e) => {
        if (!cancelled) {
          setFailure({ key, message: e instanceof ApiError ? e.message : "Could not load the user list." });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [key, page, status, plan, q, sort, dir]);

  const error = failure?.key === key ? failure.message : null;
  const loading = result?.key !== key && !error;
  const data = result?.data ?? null;
  const filtered = Boolean(q || status || plan);

  return (
    <div className="flex flex-col gap-4">
      <div role="search" aria-label="Filter accounts" className="flex flex-wrap items-end gap-3">
        <label className="flex min-w-0 flex-1 flex-col gap-1 text-xs text-ink-faint sm:max-w-sm">
          Search
          <span className="relative">
            <Icon name="search" className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-ink-faint" />
            <input
              type="search"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Name or SteamID64"
              className={`${SELECT} w-full pl-9`}
            />
          </span>
        </label>

        <label className="flex flex-col gap-1 text-xs text-ink-faint">
          Account status
          <select
            value={status}
            onChange={(e) => setParams({ status: e.target.value, page: null })}
            className={SELECT}
          >
            {STATUS_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>

        <label className="flex flex-col gap-1 text-xs text-ink-faint">
          Subscription
          <select
            value={plan}
            onChange={(e) => setParams({ plan: e.target.value, page: null })}
            className={SELECT}
          >
            {SUBSCRIPTION_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>

        {filtered ? (
          <button
            type="button"
            onClick={() => {
              setSearch("");
              setParams({ q: null, status: null, plan: null, page: null });
            }}
            className="focus-neon min-h-11 cursor-pointer rounded px-1 text-sm text-function transition-colors hover:text-ink"
          >
            Clear filters
          </button>
        ) : null}
      </div>

      {error ? <Alert>{error}</Alert> : null}

      {data ? (
        <p className="font-mono text-xs tabular-nums text-ink-faint" aria-live="polite">
          {data.total.toLocaleString("en-US")} account{data.total === 1 ? "" : "s"}
          {filtered ? " match" : ""}
        </p>
      ) : null}

      <DataTable
        caption="Accounts"
        columns={COLUMNS}
        rows={data?.users ?? null}
        rowKey={(u) => u.id}
        sort={{ key: sort, dir }}
        onSort={(next) =>
          setParams({
            sort: next.key === "joined" ? null : next.key,
            dir: next.dir === "desc" ? null : next.dir,
            page: null,
          })
        }
        page={page}
        totalPages={data?.total_pages}
        onPage={(next) => setParams({ page: next === 1 ? null : next })}
        loading={loading}
        empty={filtered ? "No accounts match these filters." : "No accounts yet."}
      />
    </div>
  );
}
