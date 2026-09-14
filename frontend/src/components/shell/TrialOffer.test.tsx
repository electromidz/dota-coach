import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiError } from "@/lib/api";
import type { PlanResponse } from "@/lib/types";

import { TrialOffer } from "./TrialOffer";

const getPlan = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getPlan };
});

const PLAN: PlanResponse = {
  plan: {
    name: "pro",
    amount_cents: 100,
    currency: "usd",
    period_days: 30,
    trial_days: 14,
  },
  checkout_available: true,
};

afterEach(() => {
  getPlan.mockReset();
});

describe("TrialOffer", () => {
  it("quotes the trial and the price the backend actually charges", async () => {
    getPlan.mockResolvedValue(PLAN);

    render(<TrialOffer />);

    expect(
      await screen.findByText(/Start your 14-day free trial\./),
    ).toBeDefined();
    expect(screen.getByText(/\$1\.00 \/ month/)).toBeDefined();
  });

  it("follows the configured figures rather than a hard-coded offer", async () => {
    getPlan.mockResolvedValue({
      ...PLAN,
      plan: { ...PLAN.plan, amount_cents: 500, trial_days: 7 },
    });

    render(<TrialOffer />);

    expect(
      await screen.findByText(/Start your 7-day free trial\./),
    ).toBeDefined();
    expect(screen.getByText(/\$5\.00 \/ month/)).toBeDefined();
  });

  it("says nothing at all when the price cannot be read", async () => {
    // A visitor is least served by a guessed price at exactly the moment the
    // backend is unreachable.
    getPlan.mockRejectedValue(new ApiError("NETWORK_ERROR", "offline", 0));

    const { container } = render(<TrialOffer />);

    await waitFor(() => expect(getPlan).toHaveBeenCalled());
    expect(container.textContent).toBe("");
  });
});
