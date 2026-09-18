import { describe, expect, it } from "vitest";

import {
  accessSummary,
  canSubscribe,
  entitlementLabel,
  formatPlanPrice,
  formatPrice,
} from "./billing";
import type { BillingResponse, Entitlement, Plan } from "./types";

const PLAN: Plan = {
  name: "pro",
  amount_cents: 100,
  currency: "usd",
  period_days: 30,
  trial_days: 14,
};

function billing(
  entitlement: Entitlement,
  days: number | null,
  checkout = true,
): BillingResponse {
  return {
    entitlement,
    subscription: {
      id: "00000000-0000-0000-0000-000000000000",
      status: entitlement === "pro" ? "active" : "trialing",
      status_label: "Trial",
      source: entitlement === "pro" ? "payment" : "trial",
      plan: "pro",
      trial_started_at: "2026-01-01T00:00:00Z",
      trial_ends_at: "2026-01-15T00:00:00Z",
      current_period_start: null,
      current_period_end: null,
      provider: null,
      created_at: "2026-01-01T00:00:00Z",
    },
    plan: PLAN,
    days_remaining: days,
    access_ends_at: null,
    payments: [],
    checkout_available: checkout,
  };
}

describe("formatPrice", () => {
  it("renders minor units as money", () => {
    expect(formatPrice(100, "usd")).toBe("$1.00");
    expect(formatPrice(1999, "usd")).toBe("$19.99");
  });

  it("renders an unfamiliar but well-formed code with its symbol slot", () => {
    // `Intl` accepts any three-letter code, prefixes it, and separates it with
    // a non-breaking space — which is correct typography, not a stray byte.
    expect(formatPrice(100, "xyz").replace(/ /g, " ")).toBe("XYZ 1.00");
  });

  it("falls back rather than throwing on a code Intl rejects", () => {
    // A crypto ticker is not ISO-4217, and a billing page must not blow up on
    // one.
    expect(formatPrice(100, "usdttrc20")).toBe("1.00 USDTTRC20");
  });
});

describe("formatPlanPrice", () => {
  it("says month for a 30-day period and spells out anything else", () => {
    expect(formatPlanPrice(PLAN)).toBe("$1.00 / month");
    expect(formatPlanPrice({ ...PLAN, period_days: 7 })).toBe("$1.00 / 7 days");
  });
});

describe("entitlementLabel", () => {
  it("names each state", () => {
    expect(entitlementLabel("pro")).toBe("Subscribed");
    expect(entitlementLabel("trial")).toBe("Free trial");
    expect(entitlementLabel("free")).toBe("Trial ended");
  });
});

describe("accessSummary", () => {
  it("counts down the trial without doing its own arithmetic", () => {
    expect(accessSummary(billing("trial", 13))).toBe(
      "13 days left in your free trial.",
    );
    expect(accessSummary(billing("trial", 1))).toBe(
      "1 day left in your free trial.",
    );
    expect(accessSummary(billing("trial", null))).toBe(
      "less than a day left in your free trial.",
    );
  });

  it("offers the price once the trial has ended", () => {
    expect(accessSummary(billing("free", null))).toContain("$1.00 / month");
  });

  it("does not offer a subscription the server cannot sell", () => {
    const summary = accessSummary(billing("free", null, false));

    expect(summary).toContain("not available");
    expect(summary).not.toContain("$1.00");
  });
});

describe("canSubscribe", () => {
  it("offers checkout to trial and lapsed accounts", () => {
    expect(canSubscribe(billing("trial", 5))).toBe(true);
    expect(canSubscribe(billing("free", null))).toBe(true);
  });

  it("never offers a second month to an account that already paid", () => {
    expect(canSubscribe(billing("pro", 20))).toBe(false);
  });

  it("stays silent when the server has no payment provider", () => {
    expect(canSubscribe(billing("free", null, false))).toBe(false);
  });
});
