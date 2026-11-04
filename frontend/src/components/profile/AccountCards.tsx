"use client";

import Link from "next/link";

import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { entitlementLabel } from "@/lib/billing";
import { dateRange, subscriptionLine, trialEndingSoon } from "@/lib/profile";
import type { BillingResponse } from "@/lib/types";
import { cn } from "@/lib/utils";

/** A secondary action styled like `Button variant="ghost"`, but a real link. */
export const GHOST_LINK = cn(
  "glass focus-neon inline-flex min-h-11 cursor-pointer items-center justify-center gap-2 rounded-xl px-5 text-sm font-semibold tracking-wide text-ink",
  "transition-colors duration-200 ease-out hover:border-function/60 hover:text-function",
);

export function MatchesStoredCard({
  count,
  range,
}: {
  count: number;
  /** Oldest and newest stored match; null while loading, or with none stored. */
  range: { oldest: string; newest: string } | null | undefined;
}) {
  return (
    <Card className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-sm text-ink">Matches stored</h2>
          <p className="text-xs text-ink-faint">Every mode, in your history</p>
        </div>
        <span className="font-mono text-xl tabular-nums text-number">{count}</span>
      </div>

      {count > 0 ? (
        range === undefined ? (
          <div aria-busy="true" className="h-4 w-40 animate-pulse rounded bg-surface-2/60">
            <span className="sr-only">Loading date range…</span>
          </div>
        ) : range ? (
          <p className="text-xs text-ink-muted">
            <span className="sr-only">Covering </span>
            {dateRange(range.oldest, range.newest)}
          </p>
        ) : null
      ) : (
        <p className="text-xs text-ink-muted">Nothing stored yet. Sync to pull your games.</p>
      )}

      <Link
        href="/matches?mode=all"
        className="focus-neon inline-flex w-fit items-center gap-1 rounded text-sm font-medium text-function transition-colors duration-200 hover:text-ink"
      >
        View all matches
        <span aria-hidden>→</span>
      </Link>
    </Card>
  );
}

export function SubscriptionCard({
  billing,
  error,
}: {
  billing: BillingResponse | null;
  error: string | null;
}) {
  const soon = billing ? trialEndingSoon(billing) : false;

  return (
    <Card className="flex flex-col gap-3" glow={billing?.entitlement === "free" || soon ? "error" : undefined}>
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="text-sm text-ink">Subscription</h2>
          {billing ? (
            <p className="text-xs text-ink-faint">
              {entitlementLabel(billing.entitlement)} · {billing.plan.name} plan
            </p>
          ) : null}
        </div>
        {soon ? (
          <span className="inline-flex shrink-0 items-center gap-1 rounded-full bg-warn/15 px-2 py-0.5 text-[0.6875rem] font-semibold text-warn">
            <Icon name="clock" className="size-3.5" />
            Ends soon
          </span>
        ) : (
          <Icon name="coins" className="size-5 shrink-0 text-ink-faint" />
        )}
      </div>

      {error ? (
        <p className="text-xs text-ink-muted">{error}</p>
      ) : billing ? (
        <p className="text-sm text-ink-muted">{subscriptionLine(billing)}</p>
      ) : (
        <div aria-busy="true" className="h-4 w-44 animate-pulse rounded bg-surface-2/60">
          <span className="sr-only">Loading subscription…</span>
        </div>
      )}

      <Link href="/billing" className={GHOST_LINK}>
        Manage
      </Link>
    </Card>
  );
}
