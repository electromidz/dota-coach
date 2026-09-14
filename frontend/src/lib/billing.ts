import type { BillingResponse, Entitlement, Plan } from "./types";

/**
 * Presentation helpers for billing.
 *
 * As everywhere else in `lib`, nothing here decides anything: the entitlement,
 * the price and the days remaining are all computed server-side. What is left
 * is turning minor units into a price and a state into a sentence.
 */

/** `100, "usd"` -> `"$1.00"`. Unknown currencies fall back to a suffix. */
export function formatPrice(amountCents: number, currency: string): string {
  const amount = amountCents / 100;

  try {
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: currency.toUpperCase(),
    }).format(amount);
  } catch {
    // An unrecognised code is not worth throwing over on a billing page.
    return `${amount.toFixed(2)} ${currency.toUpperCase()}`;
  }
}

/** `"$1.00 / month"`, with the configured period spelled out when it is not 30 days. */
export function formatPlanPrice(plan: Plan): string {
  const price = formatPrice(plan.amount_cents, plan.currency);
  const period = plan.period_days === 30 ? "month" : `${plan.period_days} days`;

  return `${price} / ${period}`;
}

/** Short label for the current entitlement. */
export function entitlementLabel(entitlement: Entitlement): string {
  switch (entitlement) {
    case "pro":
      return "Subscribed";
    case "trial":
      return "Free trial";
    default:
      return "Trial ended";
  }
}

/**
 * The one sentence the billing page leads with.
 *
 * Reads the server's figures only — `days_remaining` is already floored and
 * already null once access has run out, so there is no clock arithmetic here.
 */
export function accessSummary(billing: BillingResponse): string {
  const { entitlement, days_remaining: days, plan } = billing;

  if (entitlement === "free") {
    return billing.checkout_available
      ? `Your trial has ended. Subscribe for ${formatPlanPrice(plan)} to keep AI coaching.`
      : "Your trial has ended. Subscriptions are not available on this server.";
  }

  const remaining =
    days === null ? "less than a day" : days === 1 ? "1 day" : `${days} days`;

  return entitlement === "trial"
    ? `${remaining} left in your free trial.`
    : `Subscribed. ${remaining} left in the current period.`;
}

/**
 * Whether the page should offer checkout.
 *
 * An account that is already paid up is not shown a buy button — it would
 * open a second charge for a month it already has.
 */
export function canSubscribe(billing: BillingResponse): boolean {
  return billing.checkout_available && billing.entitlement !== "pro";
}
