"use client";

import { useEffect, useState } from "react";

import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { ApiError, getBilling, startCheckout } from "@/lib/api";
import {
  accessSummary,
  canSubscribe,
  entitlementLabel,
  formatPlanPrice,
  formatPrice,
} from "@/lib/billing";
import { useSession } from "@/lib/session-context";
import type { BillingResponse, Payment } from "@/lib/types";
import { timeAgo } from "@/lib/utils";

/**
 * Trial, subscription and charges.
 *
 * Every figure on this page — the entitlement, the days left, the price — is
 * read from the backend. Nothing here decides whether the account is entitled;
 * it only says what the backend already decided.
 */
export function Billing() {
  const { session } = useSession();
  const [data, setData] = useState<BillingResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    if (session.kind !== "signed-in") return;

    getBilling()
      .then((response) => {
        setData(response);
        setError(null);
      })
      .catch((e) =>
        setError(
          e instanceof ApiError ? e.message : "Could not load your billing.",
        ),
      );
  }, [session.kind]);

  async function handleSubscribe() {
    setStarting(true);
    setError(null);

    try {
      const { payment } = await startCheckout();

      if (payment.payment_url) {
        // The provider's hosted page. A full navigation, not a fetch: the
        // checkout is theirs, and no card or wallet detail ever passes
        // through this app.
        window.location.href = payment.payment_url;
        return;
      }

      // A charge with no hosted page is still a charge; show it rather than
      // pretending nothing happened.
      setData(await getBilling());
    } catch (e) {
      setError(
        e instanceof ApiError ? e.message : "Could not start the checkout.",
      );
    } finally {
      setStarting(false);
    }
  }

  if (session.kind === "loading") return <BillingSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;
  if (session.kind === "error") {
    return <Alert title="Cannot reach the service">{session.message}</Alert>;
  }

  if (!data) return error ? <Alert>{error}</Alert> : <BillingSkeleton />;

  const { plan, subscription, entitlement, payments } = data;

  return (
    <div className="flex flex-col gap-6 pb-4">
      {error && <Alert>{error}</Alert>}

      <Card
        glow={entitlement === "free" ? "error" : "keyword"}
        className="flex flex-col gap-5"
      >
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <p className="text-xs uppercase tracking-widest text-ink-faint">
              {entitlementLabel(entitlement)}
            </p>
            <p className="mt-2 text-lg leading-relaxed text-ink">
              {accessSummary(data)}
            </p>
          </div>

          <span className="rounded-xl border border-border px-3 py-1 font-mono text-xs uppercase tracking-widest text-ink-faint">
            {subscription.status_label}
          </span>
        </div>

        {canSubscribe(data) && (
          <div className="flex flex-wrap items-center gap-4">
            <Button onClick={handleSubscribe} disabled={starting}>
              <Icon name="coins" className="size-5" />
              {starting ? "Opening checkout…" : "Subscribe"}
            </Button>
            <p className="text-sm text-ink-faint">
              {formatPlanPrice(plan)} · paid in crypto · cancel any time
            </p>
          </div>
        )}

        {!data.checkout_available && (
          <p className="text-sm text-ink-faint">
            This deployment has no payment provider configured, so checkout is
            unavailable.
          </p>
        )}
      </Card>

      {/* What the trial covers, stated plainly: an expired account keeps every
          measured feature, and loses only what costs money per use. */}
      <Card className="flex flex-col gap-2">
        <p className="text-sm text-ink">What the subscription covers</p>
        <p className="text-sm leading-relaxed text-ink-faint">
          AI coaching — asking the model to interpret your matches. Your stats,
          benchmarks, hero intelligence, patterns and training focus are
          measured by the backend and keep working whether or not you subscribe.
        </p>
      </Card>

      <section className="flex flex-col gap-3">
        <h2 className="text-sm uppercase tracking-widest text-ink-faint">
          Payments
        </h2>

        {payments.length === 0 ? (
          <p className="text-sm text-ink-faint">No charges yet.</p>
        ) : (
          <ul className="flex flex-col gap-3">
            {payments.map((payment) => (
              <PaymentRow key={payment.id} payment={payment} />
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

function PaymentRow({ payment }: { payment: Payment }) {
  const open = payment.status === "pending" || payment.status === "confirming";

  return (
    <li>
      <Card className="flex flex-wrap items-center justify-between gap-3 p-4 sm:p-4">
        <div className="min-w-0">
          <p className="font-mono text-sm tabular-nums text-number">
            {formatPrice(payment.amount_cents, payment.currency)}
            {payment.pay_currency && (
              <span className="ml-2 text-xs uppercase text-ink-faint">
                in {payment.pay_currency}
              </span>
            )}
          </p>
          <p className="mt-1 text-xs text-ink-faint">
            {payment.status_label} · {timeAgo(payment.created_at)}
          </p>
        </div>

        {open && payment.payment_url && (
          <a
            href={payment.payment_url}
            className="inline-flex items-center gap-2 text-sm text-function underline-offset-4 hover:underline focus-neon"
          >
            <Icon name="external" className="size-4" />
            Finish payment
          </a>
        )}
      </Card>
    </li>
  );
}

function BillingSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading your subscription…</span>
      <div className="h-40 animate-pulse rounded-card bg-surface-2" />
      <div className="h-28 animate-pulse rounded-card bg-surface-2" />
    </div>
  );
}
