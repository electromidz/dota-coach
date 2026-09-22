"use client";

import { useEffect, useState } from "react";

import { getPlan } from "@/lib/api";
import type { Plan, PlanResponse } from "@/lib/types";

/**
 * The trial length and price, straight from the backend — the only place
 * either is configured. `null` both while loading and if the request fails,
 * so every caller's honest fallback is to render nothing rather than a
 * guessed figure.
 *
 * `initial` is the same payload already fetched on the server (see
 * `fetchPlan`). Seeding the state with it means the server-rendered landing
 * page and its first client render agree — no hydration mismatch, no price
 * popping in a beat after paint, and the figure is present in the HTML a
 * crawler reads. The effect still runs and still wins: a value cached for an
 * hour must not outrank a fresh one.
 */
export function usePlan(initial?: PlanResponse | null): {
  plan: Plan | null;
  checkoutAvailable: boolean;
} {
  const [plan, setPlan] = useState<Plan | null>(initial?.plan ?? null);
  const [checkoutAvailable, setCheckoutAvailable] = useState(
    initial?.checkout_available ?? false,
  );

  useEffect(() => {
    let cancelled = false;

    getPlan()
      .then((response) => {
        if (cancelled) return;
        setPlan(response.plan);
        setCheckoutAvailable(response.checkout_available);
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
    };
  }, []);

  return { plan, checkoutAvailable };
}
