import { describe, expect, it } from "vitest";

import {
  dateRange,
  dotabuffUrl,
  openDotaUrl,
  readDevicePreferences,
  subscriptionLine,
  topHeroes,
  trialEndingSoon,
  writeDevicePreferences,
} from "./profile";
import type { BillingResponse, Entitlement, HeroStats } from "./types";

function billing(
  entitlement: Entitlement,
  days: number | null,
  accessEndsAt: string | null = null,
): BillingResponse {
  return {
    entitlement,
    days_remaining: days,
    access_ends_at: accessEndsAt,
    checkout_available: true,
    payments: [],
    plan: { name: "pro", amount_cents: 499, currency: "usd", period_days: 30, trial_days: 14 },
    subscription: {
      id: "s",
      status: entitlement === "pro" ? "active" : entitlement === "trial" ? "trialing" : "expired",
      status_label: "",
      source: "trial",
      plan: "pro",
      trial_started_at: "2026-10-01T00:00:00Z",
      trial_ends_at: "2026-10-15T00:00:00Z",
      current_period_start: null,
      current_period_end: accessEndsAt,
      provider: null,
      created_at: "2026-10-01T00:00:00Z",
    },
  };
}

function hero(id: number, matches: number, winRate: number): HeroStats {
  return {
    hero_id: id,
    hero_name: `Hero ${id}`,
    matches,
    wins: Math.round(matches * winRate),
    win_rate: winRate,
    avg_kda: 3,
    avg_gpm: 500,
    last_played_at: "2026-11-01T00:00:00Z",
  };
}

describe("external profile links", () => {
  it("key on the Dota account id", () => {
    expect(openDotaUrl(12345)).toBe("https://www.opendota.com/players/12345");
    expect(dotabuffUrl(12345)).toBe("https://www.dotabuff.com/players/12345");
  });
});

describe("topHeroes", () => {
  it("takes the three most played, win rate breaking ties", () => {
    const top = topHeroes([hero(1, 5, 0.4), hero(2, 9, 0.5), hero(3, 5, 0.6), hero(4, 2, 1)]);
    expect(top.map((h) => h.hero_id)).toEqual([2, 3, 1]);
  });

  it("does not reorder the caller's array", () => {
    const heroes = [hero(1, 1, 0.5), hero(2, 9, 0.5)];
    topHeroes(heroes);
    expect(heroes[0].hero_id).toBe(1);
  });
});

describe("trialEndingSoon", () => {
  it("flags a trial with fewer than three days left", () => {
    expect(trialEndingSoon(billing("trial", 2))).toBe(true);
    expect(trialEndingSoon(billing("trial", 0))).toBe(true);
    expect(trialEndingSoon(billing("trial", 3))).toBe(false);
  });

  it("never flags paid or ended access", () => {
    expect(trialEndingSoon(billing("pro", 1))).toBe(false);
    expect(trialEndingSoon(billing("free", null))).toBe(false);
  });
});

describe("subscriptionLine", () => {
  it("reads the server's day count for a trial", () => {
    expect(subscriptionLine(billing("trial", 5))).toBe("5 days left in your free trial");
    expect(subscriptionLine(billing("trial", 1))).toBe("1 day left in your free trial");
    expect(subscriptionLine(billing("trial", 0))).toBe("less than a day left in your free trial");
  });

  it("says when paid access ends, not that it renews", () => {
    const line = subscriptionLine(billing("pro", 20, "2026-12-04T10:00:00Z"));
    expect(line).toBe("Access until 4 Dec 2026");
  });

  it("is plain about an ended trial", () => {
    expect(subscriptionLine(billing("free", null))).toBe("Your trial has ended");
  });
});

describe("dateRange", () => {
  it("joins two days, or collapses to one", () => {
    expect(dateRange("2026-03-12T10:00:00Z", "2026-11-04T10:00:00Z")).toBe(
      "12 Mar 2026 → 4 Nov 2026",
    );
    expect(dateRange("2026-11-04T08:00:00Z", "2026-11-04T12:00:00Z")).toBe("4 Nov 2026");
  });
});

describe("device preferences", () => {
  it("round-trip through storage and survive junk", () => {
    expect(writeDevicePreferences({ language: "ru", timezone: "Europe/Moscow" })).toBe(true);
    expect(readDevicePreferences()).toEqual({ language: "ru", timezone: "Europe/Moscow" });

    window.localStorage.setItem("dc_profile_prefs", "{not json");
    expect(readDevicePreferences().language).toBe("en");
  });
});
