import { describe, expect, it } from "vitest";

import { cn, formatDuration, formatRank, kda, timeAgo, ordinal, fixOrdinals } from "./utils";

describe("cn", () => {
  it("joins truthy class names", () => {
    expect(cn("a", "b")).toBe("a b");
  });

  it("drops falsy values", () => {
    expect(cn("a", false, null, undefined, "b")).toBe("a b");
  });

  it("returns an empty string when nothing is truthy", () => {
    expect(cn(false, undefined)).toBe("");
  });
});

describe("formatDuration", () => {
  it("renders minutes and zero-padded seconds", () => {
    expect(formatDuration(2400)).toBe("40:00");
    expect(formatDuration(2405)).toBe("40:05");
    expect(formatDuration(59)).toBe("0:59");
  });

  it("never renders a negative clock", () => {
    expect(formatDuration(-10)).toBe("0:00");
    expect(formatDuration(0)).toBe("0:00");
  });
});

describe("kda", () => {
  it("counts kills and assists against deaths", () => {
    expect(kda(8, 4, 12)).toBe("5.0");
  });

  it("treats zero deaths as one, rather than dividing by zero", () => {
    expect(kda(5, 0, 5)).toBe("10.0");
  });
});

describe("formatRank", () => {
  it("splits the medal from the star", () => {
    expect(formatRank(55)).toBe("Legend 5");
    expect(formatRank(31)).toBe("Crusader 1");
  });

  it("omits stars where the medal has none", () => {
    expect(formatRank(80)).toBe("Immortal");
    expect(formatRank(50)).toBe("Legend");
  });

  it("returns null when the rank is unknown", () => {
    expect(formatRank(null)).toBeNull();
    expect(formatRank(0)).toBeNull();
    // Beyond the known medals rather than a wrong guess.
    expect(formatRank(999)).toBeNull();
  });
});

describe("timeAgo", () => {
  const now = new Date("2026-01-10T12:00:00Z");

  it("describes recent times in the largest useful unit", () => {
    expect(timeAgo("2026-01-10T11:59:30Z", now)).toBe("just now");
    expect(timeAgo("2026-01-10T11:30:00Z", now)).toBe("30m ago");
    expect(timeAgo("2026-01-10T09:00:00Z", now)).toBe("3h ago");
    expect(timeAgo("2026-01-08T12:00:00Z", now)).toBe("2d ago");
  });

  it("falls back to a date once the relative form stops helping", () => {
    expect(timeAgo("2025-01-10T12:00:00Z", now)).toContain("2025");
  });

  it("does not throw on an unparseable timestamp", () => {
    expect(timeAgo("not-a-date", now)).toBe("unknown");
  });
});

describe("ordinal", () => {
  it("uses st, nd, rd and th, with the teens always th", () => {
    const cases: Array<[number, string]> = [
      [0, "0th"],
      [1, "1st"],
      [2, "2nd"],
      [3, "3rd"],
      [4, "4th"],
      [11, "11th"],
      [12, "12th"],
      [13, "13th"],
      [21, "21st"],
      [22, "22nd"],
      [23, "23rd"],
      [72, "72nd"],
      [100, "100th"],
      [101, "101st"],
      [111, "111th"],
      [112, "112th"],
    ];
    for (const [value, expected] of cases) {
      expect(ordinal(value)).toBe(expected);
    }
  });

  it("rounds before choosing a suffix and never goes negative", () => {
    expect(ordinal(71.6)).toBe("72nd");
    expect(ordinal(10.6)).toBe("11th");
    expect(ordinal(-4)).toBe("0th");
  });
});

describe("fixOrdinals", () => {
  it("corrects wrong suffixes in stored text and leaves correct ones", () => {
    expect(fixOrdinals("at the 72th percentile")).toBe("at the 72nd percentile");
    expect(fixOrdinals("61th and 51th, then 11th and 23rd")).toBe(
      "61st and 51st, then 11th and 23rd",
    );
    expect(fixOrdinals("won 5-3 in 2026")).toBe("won 5-3 in 2026");
  });
});
