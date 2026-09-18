"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { accountStatusLabel, displayName, planLabel } from "@/lib/admin";
import { ApiError, disableUser, enableUser, extendAccess, getAdminUser } from "@/lib/api";
import type { AdminUserDetail as AdminUserDetailResponse } from "@/lib/types";
import { cn, timeAgo } from "@/lib/utils";

const EXTEND_PRESETS = [7, 14, 30];

export function UserDetail({ id }: { id: string }) {
  const [data, setData] = useState<AdminUserDetailResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [busy, setBusy] = useState(false);

  async function load() {
    try {
      setData(await getAdminUser(id));
      setError(null);
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) {
        setNotFound(true);
        return;
      }
      setError(e instanceof ApiError ? e.message : "Could not load this account.");
    }
  }

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  async function handleExtend(days: number) {
    setBusy(true);
    setError(null);
    try {
      await extendAccess(id, days);
      await load();
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not extend access.");
    } finally {
      setBusy(false);
    }
  }

  async function handleDisable() {
    if (!window.confirm("Disable this account? It will be refused on its very next request.")) {
      return;
    }

    setBusy(true);
    setError(null);
    try {
      await disableUser(id);
      await load();
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not disable this account.");
    } finally {
      setBusy(false);
    }
  }

  async function handleEnable() {
    setBusy(true);
    setError(null);
    try {
      await enableUser(id);
      await load();
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not enable this account.");
    } finally {
      setBusy(false);
    }
  }

  if (notFound) {
    return (
      <div className="flex flex-col gap-4">
        <BackLink />
        <Alert title="Not found">No such account.</Alert>
      </div>
    );
  }
  if (!data) return error ? <Alert>{error}</Alert> : <DetailSkeleton />;

  return (
    <div className="flex flex-col gap-6 pb-4">
      <BackLink />
      {error ? <Alert>{error}</Alert> : null}

      <Card className="flex flex-col gap-4">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <p className="text-lg text-ink">{displayName(data)}</p>
            <p className="mt-1 font-mono text-xs tabular-nums text-ink-faint">
              {data.steam_id} · joined {timeAgo(data.created_at)}
            </p>
            <p className="mt-1 text-xs text-ink-faint">
              {data.last_login_at
                ? `Last seen ${timeAgo(data.last_login_at)}`
                : "Never logged in"}
            </p>
          </div>

          <div className="flex items-center gap-2">
            {data.is_admin ? (
              <span className="rounded-lg border border-keyword/40 px-2.5 py-1 text-xs text-keyword">
                Admin
              </span>
            ) : null}
            <span
              className={cn(
                "rounded-lg border px-2.5 py-1 text-xs",
                data.status === "disabled"
                  ? "border-error/40 text-error"
                  : "border-glass-edge text-ink-faint",
              )}
            >
              {accountStatusLabel(data.status)}
            </span>
          </div>
        </div>
      </Card>

      <Card className="flex flex-col gap-4">
        <h2 className="text-xs uppercase tracking-wider text-ink-faint">Subscription</h2>

        <div className="flex flex-wrap items-center justify-between gap-3">
          <span className="rounded-lg border border-glass-edge px-2.5 py-1 text-xs text-ink-faint">
            {planLabel(data.subscription_status)}
          </span>
          {data.trial_ends_at ? (
            <span className="text-xs text-ink-faint">
              Trial ends {new Date(data.trial_ends_at).toLocaleDateString()}
            </span>
          ) : null}
          {data.current_period_end ? (
            <span className="text-xs text-ink-faint">
              Paid through {new Date(data.current_period_end).toLocaleDateString()}
            </span>
          ) : null}
        </div>

        <div className="flex flex-wrap items-center gap-2 border-t border-glass-edge pt-4">
          <span className="text-sm text-ink-muted">Extend access:</span>
          {EXTEND_PRESETS.map((days) => (
            <Button
              key={days}
              variant="ghost"
              disabled={busy}
              onClick={() => void handleExtend(days)}
              className="px-4 py-2 text-sm"
            >
              <Icon name="clock" className="size-4" />+{days}d
            </Button>
          ))}
        </div>
      </Card>

      <Card className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h2 className="text-xs uppercase tracking-wider text-ink-faint">Danger zone</h2>
          <p className="mt-1 text-sm text-ink-muted">
            {data.status === "disabled"
              ? "This account is disabled. Re-enabling takes effect on its very next request."
              : "Disabling refuses every future request from this account, checked fresh each time — no separate session revocation needed."}
          </p>
        </div>
        {data.status === "disabled" ? (
          <Button
            variant="ghost"
            disabled={busy}
            onClick={() => void handleEnable()}
            className="border-string/40 px-5 text-sm text-string hover:border-string hover:text-string"
          >
            <Icon name="check" className="size-4" />
            Enable account
          </Button>
        ) : (
          <Button
            variant="ghost"
            disabled={busy}
            onClick={() => void handleDisable()}
            className="border-error/40 px-5 text-sm text-error hover:border-error hover:text-error"
          >
            <Icon name="alert" className="size-4" />
            Disable account
          </Button>
        )}
      </Card>

      <section className="flex flex-col gap-3">
        <h2 className="text-xs uppercase tracking-wider text-ink-faint">Recent activity</h2>

        {data.events.length === 0 ? (
          <p className="text-sm text-ink-faint">No recorded events yet.</p>
        ) : (
          <ul className="flex flex-col gap-2">
            {data.events.map((event) => (
              <li key={event.id}>
                <Card className="flex items-center justify-between gap-3 p-3.5">
                  <span className="font-mono text-xs text-ink">{event.type}</span>
                  <span className="text-xs text-ink-faint">{timeAgo(event.created_at)}</span>
                </Card>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

function BackLink() {
  return (
    <Link
      href="/admin/users"
      className="focus-neon inline-flex min-h-11 w-fit cursor-pointer items-center gap-1.5 rounded text-sm text-ink-muted transition-colors duration-200 ease-out hover:text-function"
    >
      <Icon name="chevron-left" className="size-5" />
      Users
    </Link>
  );
}

function DetailSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading account…</span>
      <div className="h-28 animate-pulse rounded-card bg-surface-2" />
      <div className="h-40 animate-pulse rounded-card bg-surface-2" />
      <div className="h-24 animate-pulse rounded-card bg-surface-2" />
    </div>
  );
}
