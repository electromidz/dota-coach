"use client";

import { useEffect, useState } from "react";

import { DateRangePicker } from "@/components/admin/DateRangePicker";
import { KpiCard } from "@/components/admin/KpiCard";
import { DashboardSkeleton } from "@/components/admin/Skeletons";
import { TimeSeriesChart } from "@/components/charts/TimeSeriesChart";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { ApiError } from "@/lib/api";
import { formatCount, formatPercentPoints } from "@/lib/admin/format";
import { getStats } from "@/lib/admin/queries";
import { rangeLabel, rangeToApi, rangeToParams, resolveRange } from "@/lib/admin/range";
import type { AdminStats, Funnel } from "@/lib/admin/types";
import { useUrlState } from "@/lib/admin/url-state";
import { formatPrice } from "@/lib/billing";
import { cn } from "@/lib/utils";

const SERIES = [
  { key: "signups", label: "Signups", color: "var(--color-series-1)" },
  { key: "logins", label: "Active users", color: "var(--color-series-2)" },
  { key: "purchases", label: "Purchases", color: "var(--color-series-3)" },
];

/**
 * Usage, the trial funnel and revenue for a date range kept in the URL.
 *
 * Every figure, including the previous-period comparison and the funnel
 * rates, is computed by the backend; this screen formats and lays out.
 */
export function AdminDashboard() {
  const [params, setParams] = useUrlState();
  const range = resolveRange(params);
  const { from, to } = range;
  const key = `${from}|${to}`;

  // Results are stored with the range they answer, so "loading" is simply
  // "the latest result is for a different range" — the old numbers stay up
  // (dimmed) until the new ones land, instead of flashing a skeleton.
  const [result, setResult] = useState<{ key: string; stats: AdminStats } | null>(null);
  const [failure, setFailure] = useState<{ key: string; message: string } | null>(null);

  useEffect(() => {
    let cancelled = false;
    getStats(rangeToApi({ preset: "custom", from, to }))
      .then((stats) => {
        if (!cancelled) setResult({ key, stats });
      })
      .catch((e) => {
        if (!cancelled) {
          setFailure({ key, message: e instanceof ApiError ? e.message : "Could not load admin stats." });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [key, from, to]);

  const error = failure?.key === key ? failure.message : null;
  const loading = result?.key !== key && !error;

  if (!result) return error ? <Alert>{error}</Alert> : <DashboardSkeleton />;

  const s = result.stats;
  const money = (cents: number) => formatPrice(cents, s.currency);

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-sm text-ink-muted" aria-live="polite">
          {rangeLabel(range)}
          <span className="text-ink-faint">
            {" "}· compared with {rangeLabel({ from: s.previous_from.slice(0, 10), to: s.previous_to.slice(0, 10) })}
          </span>
          {loading ? <span className="ml-2 text-xs text-ink-faint">Updating…</span> : null}
        </p>
        <DateRangePicker value={range} onChange={(next) => setParams(rangeToParams(next))} />
      </div>

      {error ? <Alert>{error}</Alert> : null}

      <div className={cn("flex flex-col gap-6 transition-opacity duration-200", loading && "opacity-60")}>
        <section aria-labelledby="period-heading" className="flex flex-col gap-3">
          <h2 id="period-heading" className="text-xs uppercase tracking-wider text-ink-faint">
            This period
          </h2>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">
            <KpiCard label="Signups" icon="user" value={s.period.signups} previous={s.previous.signups} />
            <KpiCard
              label="Active users"
              icon="spark"
              value={s.period.active_users}
              previous={s.previous.active_users}
              hint="Distinct accounts that signed in during the period."
            />
            <KpiCard
              label="Trials started"
              icon="clock"
              value={s.period.trials_started}
              previous={s.previous.trials_started}
            />
            <KpiCard
              label="Purchases"
              icon="coins"
              value={s.period.purchases}
              previous={s.previous.purchases}
            />
            <KpiCard
              label="Revenue"
              icon="coins"
              value={s.period.revenue_cents}
              previous={s.previous.revenue_cents}
              format={money}
            />
            <KpiCard
              label="Trials expired"
              icon="alert"
              value={s.period.trials_expired}
              previous={s.previous.trials_expired}
              higherIsBetter={false}
            />
          </div>
        </section>

        <section aria-labelledby="now-heading" className="flex flex-col gap-3">
          <h2 id="now-heading" className="text-xs uppercase tracking-wider text-ink-faint">
            Right now
          </h2>
          <dl className="glass grid grid-cols-2 gap-x-6 gap-y-3 rounded-card p-4 sm:grid-cols-3 xl:grid-cols-6">
            {[
              ["Total users", s.total_users],
              ["Active trials", s.active_trials],
              ["Currently paid", s.currently_paid],
              ["Active today", s.dau],
              ["Active 7 days", s.wau],
              ["Active 30 days", s.mau],
            ].map(([label, value]) => (
              <div key={label} className="flex flex-col gap-0.5">
                <dt className="text-xs text-ink-faint">{label}</dt>
                <dd className="font-mono text-lg tabular-nums text-ink">{formatCount(value as number)}</dd>
              </div>
            ))}
          </dl>
        </section>

        <div className="grid gap-4 lg:grid-cols-3">
          <Card className="flex min-w-0 flex-col gap-3 lg:col-span-2">
            <h2 className="text-xs uppercase tracking-wider text-ink-faint">
              Signups, active users &amp; purchases per day
            </h2>
            <TimeSeriesChart
              caption="Signups, active users and purchases per day"
              data={s.daily.map((d) => ({ ...d }))}
              series={SERIES}
            />
          </Card>

          <FunnelCard
            funnel={s.funnel}
            cohortPct={s.trial_to_paid_conversion_pct}
            cohortTrials={s.period.trials_started}
            vouchers={s.period.voucher_redemptions}
          />
        </div>
      </div>
    </div>
  );
}

/**
 * The all-time funnel, with the rate between each step. Bar lengths are
 * relative to signups; the rates are the backend's.
 */
function FunnelCard({
  funnel,
  cohortPct,
  cohortTrials,
  vouchers,
}: {
  funnel: Funnel;
  cohortPct: number | null;
  cohortTrials: number;
  vouchers: number;
}) {
  const steps = [
    { label: "Signed up", value: funnel.signed_up, rate: null as number | null, of: "" },
    {
      label: "Started a trial",
      value: funnel.trial_started,
      rate: funnel.signup_to_trial_pct,
      of: "of signups",
    },
    {
      label: "Purchased",
      value: funnel.purchased,
      rate: funnel.trial_to_paid_pct,
      of: "of trials",
    },
  ];
  const top = Math.max(1, funnel.signed_up);

  return (
    <Card className="flex min-w-0 flex-col gap-4">
      <div>
        <h2 className="text-xs uppercase tracking-wider text-ink-faint">Signup → trial → paid</h2>
        <p className="mt-1 text-xs text-ink-faint">All time</p>
      </div>

      <ol className="flex flex-col gap-3">
        {steps.map((step, i) => (
          <li key={step.label} className="flex flex-col gap-1.5">
            {i > 0 ? (
              <p className="text-xs text-ink-faint">
                ↓ <span className="font-mono tabular-nums text-ink">{formatPercentPoints(step.rate)}</span>{" "}
                {step.of}
              </p>
            ) : null}
            <div className="flex items-baseline justify-between gap-3 text-sm">
              <span className="text-ink">{step.label}</span>
              <span className="font-mono tabular-nums text-ink">{formatCount(step.value)}</span>
            </div>
            <div className="h-2 overflow-hidden rounded-full bg-mark-track/60" aria-hidden>
              <div
                className="h-full rounded-full bg-[var(--color-series-1)]"
                style={{ width: `${Math.max(step.value > 0 ? 2 : 0, (step.value / top) * 100)}%` }}
              />
            </div>
          </li>
        ))}
      </ol>

      <div className="flex flex-col gap-1.5 border-t border-glass-edge pt-3 text-xs text-ink-faint">
        <p>
          Overall, <span className="text-ink">{formatPercentPoints(funnel.signup_to_paid_pct)}</span> of
          signups have purchased.
        </p>
        <p>
          {cohortTrials === 0 || cohortPct === null ? (
            "No trials started in this period, so there is no conversion to report yet."
          ) : (
            <>
              <span className="text-ink">{formatPercentPoints(cohortPct)}</span> of the{" "}
              {formatCount(cohortTrials)} trials started in this period have converted so far.
            </>
          )}
        </p>
        {/* A voucher is not a purchase; counting it as one would overstate sales. */}
        <p>
          {formatCount(vouchers)} voucher redemption{vouchers === 1 ? "" : "s"} in this period (not
          counted as purchases).
        </p>
      </div>
    </Card>
  );
}
