"use client";

import { useCallback, useEffect, useState } from "react";

import { BarList } from "@/components/charts/BarList";
import { MultiSparkline } from "@/components/charts/MultiSparkline";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { StatTile } from "@/components/dashboard/StatTile";
import { ApiError, getAdminStats } from "@/lib/api";
import { formatPercentPoints } from "@/lib/admin";
import { formatPrice } from "@/lib/billing";
import { formatWhole } from "@/lib/stats";
import type { AdminStats } from "@/lib/types";

/** `Date` -> `"2026-08-01"`, what an `<input type="date">` reads and writes. */
function toDateInputValue(date: Date): string {
  return date.toISOString().slice(0, 10);
}

/**
 * How many users use the panel, what happens during their trial, and how
 * many buy — the three questions this whole feature exists to answer, read
 * top to bottom: usage, then the funnel, then revenue.
 *
 * Every number here is computed in Rust (`services::admin::stats`); nothing
 * on this page does its own arithmetic beyond formatting.
 */
export function AdminDashboard() {
  const [data, setData] = useState<AdminStats | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Empty means "let the backend pick" — its own default is the last 30 days.
  const [fromDate, setFromDate] = useState("");
  const [toDate, setToDate] = useState("");

  const load = useCallback((from: string, to: string) => {
    // A date-only input is a whole day; `to` needs its last instant included,
    // or picking "today" would exclude every event that already happened today.
    const fromIso = from ? `${from}T00:00:00Z` : undefined;
    const toIso = to ? `${to}T23:59:59Z` : undefined;

    getAdminStats(fromIso, toIso)
      .then((response) => {
        setData(response);
        setError(null);
      })
      .catch((e) =>
        setError(e instanceof ApiError ? e.message : "Could not load admin stats."),
      );
  }, []);

  useEffect(() => {
    load(fromDate, toDate);
  }, [load, fromDate, toDate]);

  function resetRange() {
    setFromDate("");
    setToDate("");
  }

  if (error) return <Alert>{error}</Alert>;
  if (!data) return <DashboardSkeleton />;

  const rangeActive = fromDate !== "" || toDate !== "";

  return (
    <div className="flex flex-col gap-6 pb-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-xs text-ink-faint">{formatRangeLabel(data.from, data.to)}</p>

        <form
          className="flex flex-wrap items-center gap-2 text-xs text-ink-faint"
          aria-label="Date range"
        >
          <label className="flex items-center gap-1.5">
            From
            <input
              type="date"
              value={fromDate}
              max={toDate || toDateInputValue(new Date())}
              onChange={(e) => setFromDate(e.target.value)}
              className="min-h-9 rounded-lg border border-glass-edge bg-surface-2/60 px-2 text-ink outline-none focus-neon"
            />
          </label>
          <label className="flex items-center gap-1.5">
            To
            <input
              type="date"
              value={toDate}
              min={fromDate || undefined}
              max={toDateInputValue(new Date())}
              onChange={(e) => setToDate(e.target.value)}
              className="min-h-9 rounded-lg border border-glass-edge bg-surface-2/60 px-2 text-ink outline-none focus-neon"
            />
          </label>
          {rangeActive ? (
            <button
              type="button"
              onClick={resetRange}
              className="focus-neon cursor-pointer rounded text-function transition-colors duration-200 ease-out hover:text-ink"
            >
              Reset to last 30 days
            </button>
          ) : null}
        </form>
      </div>

      <section className="grid grid-cols-2 gap-3 lg:grid-cols-3 lg:gap-4">
        <StatTile label="Total users" value={formatWhole(data.total_users)} icon="users" />
        <StatTile label="Active today" value={formatWhole(data.dau)} icon="spark" />
        <StatTile label="Active this week" value={formatWhole(data.wau)} icon="spark" />
        <StatTile label="Active this month" value={formatWhole(data.mau)} icon="spark" />
        <StatTile
          label="Active trials"
          value={formatWhole(data.active_trials)}
          icon="clock"
          tone="function"
        />
        <StatTile
          label="Paid (all-time)"
          value={formatWhole(data.paid_users)}
          icon="coins"
          tone="string"
        />
      </section>

      <div className="grid gap-4 lg:grid-cols-3">
        <Card className="flex min-w-0 flex-col gap-4 lg:col-span-2">
          <h2 className="text-xs uppercase tracking-wider text-ink-faint">
            Signups, logins &amp; purchases
          </h2>
          <MultiSparkline
            data={data.daily.map((d) => ({
              date: d.date,
              signups: d.signups,
              logins: d.logins,
              purchases: d.purchases,
            }))}
            series={[
              { key: "signups", label: "Signups", color: "var(--color-function)", swatch: "bg-function" },
              {
                key: "logins",
                label: "Logins",
                color: "var(--color-keyword)",
                swatch: "bg-keyword",
                dashArray: "4 3",
              },
              {
                key: "purchases",
                label: "Purchases",
                color: "var(--color-string)",
                swatch: "bg-string",
                dashArray: "1 3",
              },
            ]}
          />
        </Card>

        <Card className="flex min-w-0 flex-col gap-4">
          <h2 className="text-xs uppercase tracking-wider text-ink-faint">
            Signup → trial → paid
          </h2>
          <BarList
            caption="Conversion funnel"
            data={[
              { label: "Signed up", value: data.total_users },
              { label: "Started a trial", value: data.trials_started },
              { label: "Purchased", value: data.paid_users },
            ]}
          />
          <div className="flex flex-col gap-1 border-t border-glass-edge pt-3 text-xs text-ink-faint">
            <p>
              {formatPercentPoints(data.trial_to_paid_conversion_pct)} of trials started in
              this window have converted so far.
            </p>
            {/* Kept apart from "Purchased" above: a voucher redemption is not
                a purchase, and folding the two together would overstate how
                many people actually paid. */}
            <p>
              {formatWhole(data.voucher_redemptions)} voucher redemption
              {data.voucher_redemptions === 1 ? "" : "s"} in this window.
            </p>
          </div>
        </Card>
      </div>

      <Card className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <p className="text-xs uppercase tracking-wider text-ink-faint">
            Revenue in this window
          </p>
          <p className="mt-1 font-mono text-2xl tabular-nums text-number">
            {formatPrice(data.revenue_cents, data.currency)}
          </p>
        </div>
        <p className="text-sm text-ink-faint sm:max-w-xs sm:text-right">
          {formatWhole(data.currently_paid)} account
          {data.currently_paid === 1 ? " is" : "s are"} on an active paid period
          right now.
        </p>
      </Card>
    </div>
  );
}

function formatRangeLabel(from: string, to: string): string {
  const fmt = (iso: string) =>
    new Date(iso).toLocaleDateString(undefined, { month: "short", day: "numeric" });

  return `${fmt(from)} – ${fmt(to)}`;
}

function DashboardSkeleton() {
  return (
    <div className="flex flex-col gap-6" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading admin stats…</span>
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 lg:gap-4">
        {[0, 1, 2, 3, 4, 5].map((i) => (
          <div key={i} className="glass h-20 animate-pulse rounded-card" />
        ))}
      </div>
      <div className="grid gap-4 lg:grid-cols-3">
        <div className="glass h-40 animate-pulse rounded-card lg:col-span-2" />
        <div className="glass h-40 animate-pulse rounded-card" />
      </div>
    </div>
  );
}
