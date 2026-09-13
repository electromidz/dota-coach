import { describe, expect, it } from "vitest";

import { HERO_SLUGS, heroPortraitUrl } from "./heroes";

describe("hero portraits", () => {
  it("covers the full hero roster", () => {
    expect(Object.keys(HERO_SLUGS).length).toBeGreaterThan(120);
  });

  it("keeps the irregular internal names, which cannot be derived", () => {
    // The whole reason this map is generated rather than computed.
    expect(HERO_SLUGS[11]).toBe("nevermore"); // Shadow Fiend
    expect(HERO_SLUGS[21]).toBe("windrunner"); // Windranger
    expect(HERO_SLUGS[22]).toBe("zuus"); // Zeus
    expect(HERO_SLUGS[97]).toBe("magnataur"); // Magnus
  });

  it("builds a CDN url", () => {
    expect(heroPortraitUrl(5)).toBe(
      "https://cdn.cloudflare.steamstatic.com/apps/dota2/images/dota_react/heroes/crystal_maiden.png",
    );
  });

  it("returns null for an unknown hero so the caller can fall back", () => {
    expect(heroPortraitUrl(99999)).toBeNull();
  });

  it("never emits a slug that would break a url", () => {
    for (const slug of Object.values(HERO_SLUGS)) {
      expect(slug).toMatch(/^[a-z0-9_]+$/);
    }
  });
});
