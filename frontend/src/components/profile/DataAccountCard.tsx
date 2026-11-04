"use client";

import { useState } from "react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { ApiError, logout } from "@/lib/api";
import { buildExport, downloadJson } from "@/lib/profile";
import { clearSignedInHint } from "@/lib/session-context";

/**
 * What the player can take away, remove, or end.
 *
 * Deleting stored matches is shown but not offered: the backend has no
 * endpoint for it yet, and removing match rows alone would leave the player
 * model, training focus and coaching history built from them in place.
 */
export function DataAccountCard({ accountId }: { accountId: number }) {
  const [exporting, setExporting] = useState(false);
  const [message, setMessage] = useState<{ tone: "ok" | "error"; text: string } | null>(null);

  async function handleExport() {
    setExporting(true);
    setMessage(null);
    try {
      const data = await buildExport();
      const day = new Date().toISOString().slice(0, 10);
      downloadJson(data, `dota-coach-${accountId}-${day}.json`);
      setMessage({ tone: "ok", text: "Export downloaded." });
    } catch (error) {
      setMessage({
        tone: "error",
        text: error instanceof ApiError ? error.message : "Could not build your export. Try again.",
      });
    } finally {
      setExporting(false);
    }
  }

  async function handleLogout() {
    await logout().catch(() => undefined);
    clearSignedInHint();
    window.location.href = "/";
  }

  return (
    <section aria-labelledby="data-heading">
      <Card className="flex flex-col gap-4">
        <div>
          <h2 id="data-heading" className="text-sm text-ink">
            Data &amp; account
          </h2>
          <p className="text-xs text-ink-faint">Your data, and your session.</p>
        </div>

        <div className="flex flex-col gap-3 sm:flex-row sm:flex-wrap">
          <Button variant="ghost" onClick={handleExport} disabled={exporting} aria-busy={exporting}>
            <Icon name="download" className="size-5" />
            {exporting ? "Preparing export…" : "Export my data"}
          </Button>

          <Button
            variant="ghost"
            disabled
            aria-describedby="delete-unavailable"
            className="text-error"
          >
            <Icon name="skull" className="size-5" />
            Delete stored matches
          </Button>

          <Button variant="ghost" onClick={handleLogout} className="sm:ml-auto">
            <Icon name="logout" className="size-5" />
            Sign out
          </Button>
        </div>

        <p id="delete-unavailable" className="text-[0.6875rem] text-ink-faint">
          Export is a JSON file of your profile, stats, subscription and every
          stored match. Deleting stored matches is not available yet.
        </p>

        <p
          role="status"
          aria-live="polite"
          className={message?.tone === "error" ? "text-xs text-error" : "text-xs text-string"}
        >
          {message?.text ?? ""}
        </p>
      </Card>
    </section>
  );
}
