import { render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import type { PlanResponse } from "@/lib/types";

import { SignedOut } from "./SignedOut";

const getPlan = vi.hoisted(() => vi.fn());

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
 * `Reveal`/`Counter` use `IntersectionObserver`, which jsdom does not
 * implement. A no-op stub is enough for a smoke test: nothing here asserts on
 * the post-scroll-reveal state, only that the tree mounts and renders without
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
      screen.getByRole("heading", { level: 1, name: /Stop losing to the/ }),
    ).toBeDefined();
    expect(screen.getAllByText(/Start free trial/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/One plan\. No surprises\./)).toBeDefined();
    expect(screen.getByText(/Questions, answered/)).toBeDefined();
    expect(screen.getByText(/Not affiliated with Valve Corporation\./)).toBeDefined();
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
