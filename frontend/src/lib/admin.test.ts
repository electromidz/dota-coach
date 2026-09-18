import { describe, expect, it } from "vitest";

import {
  accountStatusLabel,
  displayName,
  formatPercentPoints,
  planLabel,
  voucherState,
  voucherStateLabel,
  vouchersToCsv,
} from "./admin";
import type { Voucher } from "./types";

describe("planLabel", () => {
  it("names each billing lifecycle stage", () => {
    expect(planLabel("trialing")).toBe("Trial");
    expect(planLabel("active")).toBe("Active");
    expect(planLabel("expired")).toBe("Expired");
    expect(planLabel("cancelled")).toBe("Cancelled");
    expect(planLabel("past_due")).toBe("Past due");
  });

  it("says so when the account has never had a subscription row", () => {
    expect(planLabel(null)).toBe("No subscription");
  });
});

describe("accountStatusLabel", () => {
  it("names each account status", () => {
    expect(accountStatusLabel("active")).toBe("Active");
    expect(accountStatusLabel("disabled")).toBe("Disabled");
  });
});

describe("formatPercentPoints", () => {
  it("rounds a 0-100 figure, unlike the 0-1 formatPercent in lib/stats", () => {
    expect(formatPercentPoints(42.857)).toBe("43%");
    expect(formatPercentPoints(0)).toBe("0%");
    expect(formatPercentPoints(100)).toBe("100%");
  });

  it("renders an em dash for an empty cohort", () => {
    expect(formatPercentPoints(null)).toBe("—");
  });
});

describe("displayName", () => {
  it("prefers the Steam persona name", () => {
    expect(displayName({ persona_name: "Midz", steam_id: "123" })).toBe("Midz");
  });

  it("falls back to the SteamID64 when there is no persona name", () => {
    expect(displayName({ persona_name: null, steam_id: "76561198000000000" })).toBe(
      "76561198000000000",
    );
  });
});

function voucher(overrides: Partial<Voucher> = {}): Voucher {
  return {
    id: "00000000-0000-0000-0000-000000000000",
    code: "DOTA-ABCD-2345",
    duration_days: 30,
    max_uses: 1,
    used_count: 0,
    expires_at: null,
    active: true,
    note: null,
    created_by: null,
    created_at: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

describe("voucherState", () => {
  it("is active when none of the stop conditions apply", () => {
    expect(voucherState(voucher())).toBe("active");
  });

  it("is deactivated once an admin turns it off, even if uses remain", () => {
    expect(voucherState(voucher({ active: false, used_count: 0 }))).toBe("deactivated");
  });

  it("is used_up once used_count reaches max_uses, checked before expiry", () => {
    expect(voucherState(voucher({ used_count: 1, max_uses: 1 }))).toBe("used_up");
    // Both conditions true: used-up wins over expired, by design.
    expect(
      voucherState(
        voucher({ used_count: 1, max_uses: 1, expires_at: "2020-01-01T00:00:00Z" }),
      ),
    ).toBe("used_up");
  });

  it("is expired once the expiry date has passed, with uses remaining", () => {
    expect(voucherState(voucher({ expires_at: "2020-01-01T00:00:00Z" }))).toBe("expired");
  });

  it("a null expires_at never expires on its own", () => {
    expect(voucherState(voucher({ expires_at: null }))).toBe("active");
  });
});

describe("voucherStateLabel", () => {
  it("names each state", () => {
    expect(voucherStateLabel(voucher())).toBe("Active");
    expect(voucherStateLabel(voucher({ active: false }))).toBe("Deactivated");
    expect(voucherStateLabel(voucher({ used_count: 1, max_uses: 1 }))).toBe("Used up");
    expect(voucherStateLabel(voucher({ expires_at: "2020-01-01T00:00:00Z" }))).toBe(
      "Expired",
    );
  });
});

describe("vouchersToCsv", () => {
  it("renders a header and one row per voucher", () => {
    const csv = vouchersToCsv([voucher({ code: "DOTA-ABCD-2345", note: "giveaway" })]);
    const lines = csv.split("\n");

    expect(lines).toHaveLength(2);
    expect(lines[0]).toContain("code");
    expect(lines[1]).toContain("DOTA-ABCD-2345");
    expect(lines[1]).toContain("giveaway");
  });

  it("escapes an embedded quote rather than corrupting the column boundary", () => {
    const csv = vouchersToCsv([voucher({ note: 'says "hi"' })]);

    expect(csv).toContain('"says ""hi"""');
  });

  it("renders an empty string, not the literal word null, for absent fields", () => {
    const csv = vouchersToCsv([voucher({ note: null, expires_at: null })]);
    const dataLine = csv.split("\n")[1];

    expect(dataLine).not.toContain("null");
  });
});
