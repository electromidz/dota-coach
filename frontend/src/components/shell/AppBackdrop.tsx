"use client";

import { useSession } from "@/lib/session-context";

/**
 * The map art behind the signed-in screens.
 *
 * Gated on the session for the same reason `AppChrome` is, and on the same
 * condition: the signed-out screen paints its own, brighter backdrop, and two
 * of them stacked would be twice the art at twice the opacity. Waiting for the
 * session to resolve also stops it flashing in behind a screen that is about
 * to redirect.
 *
 * `fixed` so it neither scrolls nor repaints as the page moves under it, and
 * `-z-20` so it sits beneath the ambient wash the app already paints — which
 * then tints the green art towards this product's palette instead of leaving
 * a screenshot of Dota behind the numbers. `aria-hidden` because it is
 * decoration; there is nothing here for a screen reader to announce.
 */
export function AppBackdrop() {
  const { session } = useSession();
  if (session.kind !== "signed-in") return null;

  return (
    <div aria-hidden className="pointer-events-none fixed inset-0 -z-20">
      <div className="app-backdrop absolute inset-0" />
      <div className="app-scrim absolute inset-0" />
    </div>
  );
}
