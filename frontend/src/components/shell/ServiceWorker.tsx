"use client";

import { useEffect } from "react";

/**
 * Registers the app-shell service worker.
 *
 * In development the worker is deliberately *unregistered* instead: a cached
 * shell in front of a hot-reloading dev server produces exactly the class of
 * bug where the fix is "hard refresh", and nobody should have to know that.
 *
 * Registration failures are swallowed. A missing service worker costs an
 * offline fallback; it must never cost the page.
 */
export function ServiceWorker() {
  useEffect(() => {
    if (!("serviceWorker" in navigator)) return;

    if (process.env.NODE_ENV !== "production") {
      navigator.serviceWorker
        .getRegistrations()
        .then((registrations) =>
          registrations.forEach((registration) => registration.unregister()),
        )
        .catch(() => undefined);
      return;
    }

    // After load, so registration never competes with the first paint for
    // bandwidth on a phone.
    const register = () => {
      navigator.serviceWorker.register("/sw.js").catch(() => undefined);
    };

    if (document.readyState === "complete") {
      register();
      return;
    }

    window.addEventListener("load", register);
    return () => window.removeEventListener("load", register);
  }, []);

  return null;
}
