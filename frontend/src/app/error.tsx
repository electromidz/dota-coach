"use client";

import { useEffect } from "react";

import { AppShell } from "@/components/shell/AppShell";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Icon } from "@/components/ui/Icon";

/**
 * The route-level boundary: a render that threw, rather than a request that
 * failed.
 *
 * Failed requests are handled where they happen — every feature component turns
 * an `ApiError` into a message in place, because losing the whole page for one
 * unavailable panel is a worse answer. What reaches here is a bug, so it offers
 * the two things that actually help: retry, and the digest to quote.
 */
export default function RouteError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    // The server log has the stack; the browser console is where a user can be
    // asked to look.
    console.error("[dota-coach] render failed", error);
  }, [error]);

  return (
    <AppShell title="Something broke" eyebrow="Error">
      <div className="flex flex-col gap-5 pb-4">
        <Alert title="This page did not render">
          The rest of the app still works. Try again — if it keeps happening,
          quote the reference below.
        </Alert>

        <div className="flex flex-wrap items-center gap-4">
          <Button onClick={reset}>
            <Icon name="refresh" className="size-5" />
            Try again
          </Button>

          {error.digest && (
            <p className="font-mono text-xs text-ink-faint">
              Reference: {error.digest}
            </p>
          )}
        </div>
      </div>
    </AppShell>
  );
}
