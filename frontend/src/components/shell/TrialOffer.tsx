"use client";

import { useEffect, useState } from "react";

import { getPlan } from "@/lib/api";
import { formatPlanPrice } from "@/lib/billing";
import type { Plan } from "@/lib/types";

/**
 * The offer line on the signed-out page: "Start your 14-day free trial. Then
 * $1/month."
 *
 * Both figures come from the backend, which is the only place they are
 * configured. Until they arrive — or if the API is unreachable, which is
 * exactly when a visitor is least served by a broken sentence — nothing is
 * rendered rather than a guessed price.
 */
export function TrialOffer({ className }: { className?: string }) {
  const [plan, setPlan] = useState<Plan | null>(null);

  useEffect(() => {
    let cancelled = false;

    getPlan()
      .then((response) => {
        if (!cancelled) setPlan(response.plan);
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
    };
  }, []);

  if (!plan) return null;

  return (
    <p className={className}>
      <span className="font-semibold text-ink">
        Start your {plan.trial_days}-day free trial.
      </span>{" "}
      <span className="text-ink-muted">
        Then {formatPlanPrice(plan)} for AI coaching — everything measured stays
        free.
      </span>
    </p>
  );
}
