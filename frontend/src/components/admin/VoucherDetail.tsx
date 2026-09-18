"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { voucherStateLabel } from "@/lib/admin";
import { ApiError, deactivateVoucher, getAdminVoucher } from "@/lib/api";
import type { AdminVoucherDetail as AdminVoucherDetailResponse } from "@/lib/types";
import { cn, timeAgo } from "@/lib/utils";

export function VoucherDetail({ id }: { id: string }) {
  const [data, setData] = useState<AdminVoucherDetailResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [busy, setBusy] = useState(false);

  async function load() {
    try {
      setData(await getAdminVoucher(id));
      setError(null);
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) {
        setNotFound(true);
        return;
      }
      setError(e instanceof ApiError ? e.message : "Could not load this voucher.");
    }
  }

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  async function handleDeactivate() {
    if (!window.confirm("Deactivate this voucher? Existing redemptions are unaffected.")) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await deactivateVoucher(id);
      await load();
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not deactivate this voucher.");
    } finally {
      setBusy(false);
    }
  }

  if (notFound) {
    return (
      <div className="flex flex-col gap-4">
        <BackLink />
        <Alert title="Not found">No such voucher.</Alert>
      </div>
    );
  }
  if (!data) return error ? <Alert>{error}</Alert> : <DetailSkeleton />;

  const state = voucherStateLabel(data);

  return (
    <div className="flex flex-col gap-6 pb-4">
      <BackLink />
      {error ? <Alert>{error}</Alert> : null}

      <Card className="flex flex-col gap-4">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <p className="font-mono text-lg text-ink">{data.code}</p>
            <p className="mt-1 text-xs text-ink-faint">
              {data.duration_days} days · created {timeAgo(data.created_at)}
              {data.note ? ` · ${data.note}` : ""}
            </p>
          </div>
          <span
            className={cn(
              "rounded-lg border px-2.5 py-1 text-xs",
              state === "Active"
                ? "border-glass-edge text-ink-faint"
                : "border-error/40 text-error",
            )}
          >
            {state}
          </span>
        </div>

        <div className="flex flex-wrap items-center gap-4 border-t border-glass-edge pt-4 text-sm text-ink-muted">
          <span>
            {data.used_count} / {data.max_uses} used
          </span>
          {data.expires_at ? (
            <span>Expires {new Date(data.expires_at).toLocaleDateString()}</span>
          ) : (
            <span>Never expires</span>
          )}
        </div>

        {state === "Active" ? (
          <Button
            variant="ghost"
            disabled={busy}
            onClick={() => void handleDeactivate()}
            className="self-start border-error/40 px-5 text-sm text-error hover:border-error hover:text-error"
          >
            <Icon name="alert" className="size-4" />
            Deactivate
          </Button>
        ) : null}
      </Card>

      <section className="flex flex-col gap-3">
        <h2 className="text-xs uppercase tracking-wider text-ink-faint">
          Redeemed by ({data.redemptions.length})
        </h2>

        {data.redemptions.length === 0 ? (
          <p className="text-sm text-ink-faint">No redemptions yet.</p>
        ) : (
          <ul className="flex flex-col gap-2">
            {data.redemptions.map((r) => (
              <li key={r.id}>
                <Link href={`/admin/users/${r.user_id}`}>
                  <Card className="flex items-center justify-between gap-3 p-3.5 transition-colors duration-200 ease-out hover:border-function/50">
                    <span className="text-sm text-ink">{r.persona_name ?? r.steam_id}</span>
                    <span className="text-xs text-ink-faint">{timeAgo(r.redeemed_at)}</span>
                  </Card>
                </Link>
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
      href="/admin/vouchers"
      className="focus-neon inline-flex min-h-11 w-fit cursor-pointer items-center gap-1.5 rounded text-sm text-ink-muted transition-colors duration-200 ease-out hover:text-function"
    >
      <Icon name="chevron-left" className="size-5" />
      Vouchers
    </Link>
  );
}

function DetailSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading voucher…</span>
      <div className="h-32 animate-pulse rounded-card bg-surface-2" />
      <div className="h-24 animate-pulse rounded-card bg-surface-2" />
    </div>
  );
}
