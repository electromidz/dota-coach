import { describe, expect, it } from "vitest";

import {
  activityFeed,
  activityLabel,
  canDeactivate,
  delta,
  deltaLabel,
  durationLabel,
  formatPercentPoints,
  groupByDay,
  showsExpiry,
  subscriptionLabel,
  voucherStatus,
  vouchersToCsv,
} from "./format";
import type { AdminEventRecord, AuditLogEntry, VoucherListItem } from "./types";

function voucher(overrides: Partial<VoucherListItem> = {}): VoucherListItem {
  return {
    id: "v1",
    code: "DOTA-AAAA-BBBB",
    duration_days: 30,
    max_uses: 1,
    used_count: 0,
    expires_at: null,
    active: true,
    note: null,
    created_by: null,
    created_at: "2026-11-01T00:00:00Z",
    redeemed_by: null,
    ...overrides,
  };
}

describe("labels", () => {
  it("never calls a paid subscription 'Active', so it can't be confused with the account", () => {
    expect(subscriptionLabel("active")).toBe("Paid");
    expect(subscriptionLabel("trialing")).toBe("Trial");
    expect(subscriptionLabel(null)).toBe("None");
  });

  it("names durations in words", () => {
    expect(durationLabel(1)).toBe("1 day");
    expect(durationLabel(30)).toBe("30 days");
    expect(durationLabel(365)).toBe("1 year");
    expect(durationLabel(730)).toBe("2 years");
  });

  it("turns event slugs into sentences, including ones it has never seen", () => {
    expect(activityLabel("trial_expired")).toBe("Trial expired");
    expect(activityLabel("disable_user")).toBe("Account disabled by an admin");
    expect(activityLabel("brand_new_event")).toBe("Brand new event");
  });
});

describe("percentages and deltas", () => {
  it("says n/a when there was nothing to divide by, and keeps real zeros", () => {
    expect(formatPercentPoints(null)).toBe("n/a");
    expect(formatPercentPoints(0)).toBe("0%");
    expect(formatPercentPoints(0.4)).toBe("<1%");
    expect(formatPercentPoints(42.6)).toBe("43%");
  });

  it("compares against the previous period", () => {
    expect(deltaLabel(delta(12, 10))).toBe("+20% vs previous");
    expect(deltaLabel(delta(8, 10))).toBe("−20% vs previous");
    expect(deltaLabel(delta(5, 5))).toBe("No change");
    // No baseline: a percentage would be infinite, so show the count.
    expect(deltaLabel(delta(3, 0))).toBe("+3 vs previous");
  });
});

describe("voucher status", () => {
  const now = Date.parse("2026-11-04T00:00:00Z");

  it("follows the same priority as the backend filter", () => {
    expect(voucherStatus(voucher(), now)).toBe("active");
    expect(voucherStatus(voucher({ used_count: 1 }), now)).toBe("used");
    expect(voucherStatus(voucher({ expires_at: "2026-11-01T00:00:00Z" }), now)).toBe("expired");
    expect(voucherStatus(voucher({ active: false, used_count: 1 }), now)).toBe("deactivated");
  });

  it("only offers deactivation, and an expiry countdown, while a code is usable", () => {
    const used = voucher({ used_count: 1, expires_at: "2099-01-01T00:00:00Z" });
    expect(canDeactivate(used)).toBe(false);
    expect(showsExpiry(used)).toBe(false);
    expect(canDeactivate(voucher())).toBe(true);
    expect(showsExpiry(voucher({ expires_at: "2099-01-01T00:00:00Z" }))).toBe(true);
  });

  it("exports who redeemed each code, escaping quotes", () => {
    const csv = vouchersToCsv([
      voucher({
        note: 'said "thanks"',
        used_count: 1,
        redeemed_by: {
          id: "r",
          user_id: "u",
          steam_id: "76561198000000000",
          persona_name: "Midz",
          redeemed_at: "2026-11-02T10:00:00Z",
        },
      }),
    ]);
    const [header, row] = csv.split("\n");
    expect(header).toContain('"redeemed_by"');
    expect(row).toContain('"used"');
    expect(row).toContain('"Midz"');
    expect(row).toContain('"said ""thanks"""');
  });
});

describe("activity feed", () => {
  const events: AdminEventRecord[] = [
    { id: "1", type: "login", metadata: {}, created_at: "2026-11-03T09:00:00Z" },
    { id: "2", type: "trial_expired", metadata: {}, created_at: "2026-11-01T09:00:00Z" },
  ];
  const actions: AuditLogEntry[] = [
    {
      id: "a",
      admin_id: "x",
      admin_persona_name: "XoniX",
      action: "extend_access",
      target_type: "user",
      target_id: "u",
      metadata: { days: 7, reason: "goodwill" },
      created_at: "2026-11-03T12:00:00Z",
    },
  ];

  it("merges account events and admin actions, newest first, with actor and reason", () => {
    const feed = activityFeed({ events, admin_actions: actions });
    expect(feed.map((e) => e.type)).toEqual(["extend_access", "login", "trial_expired"]);
    expect(feed[0]).toMatchObject({ kind: "admin", actor: "XoniX", reason: "goodwill" });
  });

  it("groups by calendar day", () => {
    const groups = groupByDay(activityFeed({ events, admin_actions: actions }));
    expect(groups.map((g) => g.rows.length)).toEqual([2, 1]);
  });
});
