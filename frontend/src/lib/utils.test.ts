import { describe, expect, it } from "vitest";

import { cn, formatDuration, formatRank, kda, timeAgo } from "./utils";

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
