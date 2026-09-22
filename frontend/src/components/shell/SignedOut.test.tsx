import { render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { FAQ_ITEMS } from "@/lib/faq";
import type { PlanResponse } from "@/lib/types";

import { SignedOut } from "./SignedOut";

const getPlan = vi.hoisted(() => vi.fn());

// `SessionHandoff` calls `useRouter`, which throws outside an app router.
// Nothing here asserts on the handoff — it has its own test file.
vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh: vi.fn() }),
}));

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getPlan };
});

const PLAN: PlanResponse = {
  plan: {
    name: "pro",
    amount_cents: 998,
    currency: "usd",
    period_days: 30,
    trial_days: 14,
  },
  checkout_available: true,
};

/**
 * `Reveal` uses `IntersectionObserver`, which jsdom does not implement. A
 * no-op stub is enough for a smoke test: nothing here asserts on the
 * post-scroll-reveal state, only that the tree mounts and renders without
 * throwing.
 */
beforeAll(() => {
  class FakeIntersectionObserver {
    observe() {}
    disconnect() {}
  }
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
});

afterEach(() => {
  getPlan.mockReset();
});

describe("SignedOut", () => {
  it("renders the marketing landing page without a session", async () => {
    getPlan.mockResolvedValue(PLAN);

    render(<SignedOut />);

    expect(screen.getByRole("banner")).toBeDefined();
    expect(
      screen.getByRole("heading", {
        level: 1,
        name: /The AI Dota 2 coach that finds/,
      }),
    ).toBeDefined();
    expect(screen.getAllByText(/Start free trial/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/One plan\. No surprises\./)).toBeDefined();
    expect(screen.getByText(/Dota 2 coaching questions, answered/)).toBeDefined();
    expect(screen.getByText(/Not affiliated with Valve Corporation\./)).toBeDefined();
  });

  /**
   * The whole point of the SEO refactor: this tree has to be renderable
   * without a session, with its headings and FAQ text present in the markup
   * rather than arriving after a fetch. A regression here — someone marking
   * the page `"use client"` behind a session gate again — is invisible in the
   * browser and fatal to indexing.
   */
  it("exposes one h1 and a heading outline a crawler can read", () => {
    getPlan.mockResolvedValue(PLAN);

    render(<SignedOut />);

    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    expect(
      screen.getByRole("heading", {
        level: 2,
        name: /What an AI Dota 2 coach actually does/,
      }),
    ).toBeDefined();
  });

  it("renders every FAQ question, so the JSON-LD cannot claim one the page hides", () => {
    getPlan.mockResolvedValue(PLAN);

    render(<SignedOut />);

    for (const item of FAQ_ITEMS) {
      // By role, not by text: each question is an `<h3>` inside its
      // `<summary>`, so a plain text query matches both wrappers.
      expect(
        screen.getByRole("heading", { level: 3, name: item.q }),
      ).toBeDefined();
      expect(screen.getByText(item.a)).toBeDefined();
    }
  });

  it("puts the backend's price and trial length in the server-rendered markup", () => {
    getPlan.mockResolvedValue(PLAN);

    render(<SignedOut plan={PLAN} />);

    // Seeded from the `plan` prop, so these are present on first render
    // rather than after `usePlan` resolves.
    expect(screen.getByText("$9.98 / month")).toBeDefined();
    expect(screen.getByText("14 days")).toBeDefined();
  });

  it("surfaces a login error passed back from the Steam redirect", () => {
    getPlan.mockResolvedValue(PLAN);

    render(<SignedOut loginError="steam_rejected" />);

    expect(screen.getByText(/Sign-in did not complete/)).toBeDefined();
    expect(
      screen.getByText(/Steam could not verify that sign-in/),
    ).toBeDefined();
  });
});
