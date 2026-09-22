"use client";

import { useRouter } from "next/navigation";
import { useEffect } from "react";

import { ApiError, getMe } from "@/lib/api";
import { clearSignedInHint, markSignedInHint } from "@/lib/session-context";

/**
 * Closes the loop that lets a signed-in visitor off the marketing page.
 *
 * `/` decides server-side which of its two pages to render, using the
 * `dc_signed_in` hint cookie — the real session cookie is `HttpOnly` and
 * belongs to the backend host, so the Next server cannot see it. That left an
 * ordering problem with no exit: the hint was only ever written by
 * `SessionProvider`, `SessionProvider` only mounts inside `AppShell`, and
 * `AppShell` only renders once the hint exists. A visitor coming back from
 * Steam with a perfectly good session landed on the landing page and stayed
 * there, because nothing on it ever asked whether they were signed in.
 *
 * So this asks. It renders nothing, resolves the session once, and on a hit
 * writes the hint and re-runs the server component — which now takes the
 * dashboard branch. A miss is the overwhelmingly common case (a real visitor
 * reading the pitch) and costs one `401`, which is what this page spent before
 * it was server-rendered at all.
 *
 * No loop is possible: the refresh only happens when the session resolves
 * signed-in, and after it the server no longer renders this component.
 */
export function SessionHandoff() {
  const router = useRouter();

  useEffect(() => {
    let cancelled = false;

    getMe()
      .then(() => {
        if (cancelled) return;
        markSignedInHint();
        router.refresh();
      })
      .catch((error: unknown) => {
        // A confirmed 401 is the normal answer here. Clearing on it as well
        // means a hint left behind by an expired session cannot keep sending
        // this visitor to a dashboard that will only 401 again.
        if (
          !cancelled &&
          error instanceof ApiError &&
          error.isUnauthenticated
        ) {
          clearSignedInHint();
        }
        // Anything else is a transport or server failure, which says nothing
        // about the session — leave the hint alone and let the page be a page.
      });

    return () => {
      cancelled = true;
    };
  }, [router]);

  return null;
}
