"use client";

import { useState } from "react";

import { ApiError } from "@/lib/api";
import type { CoachResponse } from "@/lib/types";

/**
 * The state around the one button in this product that spends money.
 *
 * Extracted because two pages present a generated analysis very differently —
 * the coach page as a report, the match page as a ranked list of mistakes — and
 * the *rules* around generating one are identical in both. Two copies of those
 * rules would drift, and the one that drifts is the paywall branch, which is
 * the one that must not.
 *
 * Three outcomes, kept apart on purpose:
 *
 *   - **success** — the response replaces what was on screen;
 *   - **paywalled** — a lapsed trial. Not a failure: every measured figure on
 *     the page is still correct and still shown, so this gets its own state
 *     rather than an error message;
 *   - **error** — everything else, reported rather than swallowed.
 */
export function useGeneration(
  initial: CoachResponse,
  onGenerate: () => Promise<CoachResponse>,
) {
  const [current, setCurrent] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [paywalled, setPaywalled] = useState(false);

  async function generate() {
    setBusy(true);
    setError(null);
    setPaywalled(false);
    try {
      setCurrent(await onGenerate());
    } catch (e) {
      if (e instanceof ApiError && e.isPaymentRequired) {
        setPaywalled(true);
        return;
      }
      setError(
        e instanceof ApiError ? e.message : "Could not reach the coach.",
      );
    } finally {
      setBusy(false);
    }
  }

  return { current, busy, error, paywalled, generate };
}
