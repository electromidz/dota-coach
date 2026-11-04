"use client";

import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

type Patch = Record<string, string | number | null | undefined>;

/**
 * The page's query string as state. Filters, sort, page and date range live
 * in the URL, so a reload, a shared link or the back button lands on the same
 * view.
 *
 * `replace`, not `push`: typing a search should not leave one history entry
 * per keystroke. Components using this must sit inside a `<Suspense>`
 * boundary (Next requires it for `useSearchParams` in a production build).
 */
export function useUrlState() {
  const params = useSearchParams();
  const router = useRouter();
  const pathname = usePathname();

  const update = useCallback(
    (patch: Patch) => {
      const next = new URLSearchParams(params.toString());
      for (const [key, value] of Object.entries(patch)) {
        if (value === null || value === undefined || value === "") next.delete(key);
        else next.set(key, String(value));
      }
      const qs = next.toString();
      router.replace(qs ? `${pathname}?${qs}` : pathname, { scroll: false });
    },
    [params, router, pathname],
  );

  return [params, update] as const;
}

/** `value`, once it has stopped changing for `ms`. */
export function useDebounced<T>(value: T, ms = 300): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setSettled(value), ms);
    return () => window.clearTimeout(timer);
  }, [value, ms]);
  return settled;
}
