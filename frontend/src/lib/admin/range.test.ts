import { describe, expect, it } from "vitest";

import { ALL_TIME_START, rangeToApi, rangeToParams, resolveRange } from "./range";

const today = new Date("2026-11-04T15:00:00Z");
const params = (qs: string) => new URLSearchParams(qs);

describe("resolveRange", () => {
  it("defaults to the last 30 days, today included", () => {
    expect(resolveRange(params(""), today)).toEqual({ preset: "30d", from: "2026-10-06", to: "2026-11-04" });
  });

  it("resolves presets to concrete days, so the inputs can show them", () => {
    expect(resolveRange(params("range=7d"), today)).toMatchObject({ from: "2026-10-29", to: "2026-11-04" });
    expect(resolveRange(params("range=all"), today).from).toBe(ALL_TIME_START);
  });

  it("treats from/to as a custom range, and repairs a reversed one", () => {
    expect(resolveRange(params("from=2026-10-01&to=2026-10-10"), today)).toEqual({
      preset: "custom",
      from: "2026-10-01",
      to: "2026-10-10",
    });
    expect(resolveRange(params("from=2026-10-10&to=2026-10-01"), today)).toMatchObject({
      from: "2026-10-01",
      to: "2026-10-10",
    });
  });

  it("ignores junk", () => {
    expect(resolveRange(params("range=1y&from=yesterday"), today).preset).toBe("30d");
  });
});

describe("range ↔ URL and API", () => {
  it("covers whole days", () => {
    expect(rangeToApi({ preset: "custom", from: "2026-10-01", to: "2026-10-10" })).toEqual({
      from: "2026-10-01T00:00:00Z",
      to: "2026-10-10T23:59:59Z",
    });
  });

  it("keeps the default out of the URL and a custom range fully in it", () => {
    expect(rangeToParams({ preset: "30d", from: "a", to: "b" })).toEqual({ range: null, from: null, to: null });
    expect(rangeToParams({ preset: "90d", from: "a", to: "b" })).toEqual({ range: "90d", from: null, to: null });
    expect(rangeToParams({ preset: "custom", from: "2026-10-01", to: "2026-10-10" })).toEqual({
      range: null,
      from: "2026-10-01",
      to: "2026-10-10",
    });
  });
});
