"use client";

import { useEffect, useState } from "react";

import { ApiError, getHealth } from "@/lib/api";
import type { HealthResponse } from "@/lib/types";
import { cn } from "@/lib/utils";

type State =
  | { kind: "loading" }
  | { kind: "ready"; health: HealthResponse }
  | { kind: "error"; message: string };

/**
 * Live view of the backend chain (frontend -> Rust API -> Postgres).
 * It exists so a fresh checkout can be verified in the browser, not just curl.
 */
export function SystemStatus() {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;

    getHealth()
      .then((health) => {
        if (!cancelled) setState({ kind: "ready", health });
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setState({
          kind: "error",
          message:
            error instanceof ApiError
              ? error.message
              : "Could not reach the coaching service.",
        });
      });

    return () => {
      cancelled = true;
    };
  }, []);

  if (state.kind === "loading") {
    return <p className="text-sm text-ink-faint">Checking services…</p>;
  }

  if (state.kind === "error") {
    return (
      <div className="space-y-1">
        <Row label="Coaching API" ok={false} value="unreachable" />
        <p className="text-sm text-ink-muted">{state.message}</p>
      </div>
    );
  }

  const { health } = state;
  const dbUp = health.database === "up";

  return (
    <div className="space-y-1">
      <Row label="Coaching API" ok value={`v${health.version}`} />
      <Row label="Database" ok={dbUp} value={dbUp ? "connected" : "unreachable"} />
      <Row
        label="AI provider"
        ok={health.llm_configured}
        value={health.llm_configured ? "configured" : "not configured"}
      />
    </div>
  );
}

function Row({ label, ok, value }: { label: string; ok: boolean; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 py-1 text-sm">
      <span className="text-ink-muted">{label}</span>
      <span className="flex items-center gap-2">
        {/* Glowing status dot; the adjacent word carries the same information
            for anyone who cannot separate the two hues. */}
        <span
          aria-hidden
          className={cn(
            "size-2 rounded-full",
            ok
              ? "bg-string shadow-[0_0_8px_var(--color-string)]"
              : "bg-error shadow-[0_0_8px_var(--color-error)]",
          )}
        />
        <span
          className={cn(
            "font-mono text-xs tabular-nums",
            ok ? "text-string" : "text-error",
          )}
        >
          {value}
        </span>
      </span>
    </div>
  );
}
