"use client";

import { useEffect, useState } from "react";

import { getPlan } from "@/lib/api";
import type { Plan } from "@/lib/types";

/**
 * The trial length and price, straight from the backend — the only place
 * either is configured. `null` both while loading and if the request fails,
 * so every caller's honest fallback is to render nothing rather than a
 * guessed figure.
 */
export function usePlan(): { plan: Plan | null; checkoutAvailable: boolean } {
  const [plan, setPlan] = useState<Plan | null>(null);
  const [checkoutAvailable, setCheckoutAvailable] = useState(false);

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
