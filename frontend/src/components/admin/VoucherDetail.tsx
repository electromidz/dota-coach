"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { DetailSkeleton } from "@/components/admin/Skeletons";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { CopyButton } from "@/components/ui/CopyButton";
import { Icon } from "@/components/ui/Icon";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { ApiError } from "@/lib/api";
import {
  VOUCHER_STATUS_LABELS,
  canDeactivate,
  durationLabel,
  exactTime,
  shortDate,
  showsExpiry,
  voucherStatus,
  voucherTone,
} from "@/lib/admin/format";
import { deactivateVoucher, getVoucher } from "@/lib/admin/queries";
import type { VoucherDetail as VoucherDetailData } from "@/lib/admin/types";

export function VoucherDetail({ id }: { id: string }) {
  const [data, setData] = useState<VoucherDetailData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    let cancelled = false;
    getVoucher(id)
      .then((v) => !cancelled && setData(v))
      .catch((e) => {
        if (cancelled) return;
        if (e instanceof ApiError && e.status === 404) setNotFound(true);
        else setError(e instanceof ApiError ? e.message : "Could not load this voucher.");
      });
    return () => {
      cancelled = true;
    };
  }, [id]);

  if (notFound) {
    return (
      <div className="flex flex-col gap-4">
        <BackLink />
        <Alert title="Not found">No such voucher.</Alert>
      </div>
    );
  }
  if (!data) return error ? <Alert>{error}</Alert> : <DetailSkeleton />;

  const status = voucherStatus(data);

  return (
    <div className="flex flex-col gap-6">
      <BackLink />
      {error ? <Alert>{error}</Alert> : null}

      <Card className="flex flex-col gap-4">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <p className="flex items-center gap-1 font-mono text-lg text-ink">
              {data.code}
              <CopyButton value={data.code} label="code" />
            </p>
            <p className="mt-1 text-xs text-ink-faint">
              Grants {durationLabel(data.duration_days)} · created{" "}
              <span title={exactTime(data.created_at)}>{shortDate(data.created_at)}</span>
              {data.note ? ` · ${data.note}` : ""}
            </p>
          </div>
          <StatusBadge label="Voucher status" tone={voucherTone(status)}>
            {VOUCHER_STATUS_LABELS[status]}
          </StatusBadge>
        </div>

        <dl className="grid grid-cols-2 gap-3 border-t border-glass-edge pt-4 text-sm sm:grid-cols-3">
          <div>
            <dt className="text-xs text-ink-faint">Uses</dt>
            <dd className="text-ink">
              {data.used_count} of {data.max_uses}
            </dd>
          </div>
          <div>
            <dt className="text-xs text-ink-faint">Expires</dt>
            <dd className="text-ink">
              {showsExpiry(data) && data.expires_at
                ? shortDate(data.expires_at)
                : data.expires_at === null
                  ? "Never"
                  : status === "expired"
                    ? `Expired ${shortDate(data.expires_at)}`
                    : "—"}
            </dd>
          </div>
        </dl>

        {canDeactivate(data) ? (
          <Button
            variant="ghost"
            onClick={() => setConfirming(true)}
            className="self-start border-error/40 px-5 text-sm text-error hover:border-error hover:text-error"
          >
            <Icon name="alert" className="size-4" />
            Deactivate…
          </Button>
        ) : null}
      </Card>

      <section className="flex flex-col gap-3">
        <h2 className="text-xs uppercase tracking-wider text-ink-faint">
          Redeemed by ({data.redemptions.length})
        </h2>
        {data.redemptions.length === 0 ? (
          <Card>
            <p className="text-sm text-ink-muted">Not redeemed yet.</p>
          </Card>
        ) : (
          <ul className="flex flex-col gap-2">
            {data.redemptions.map((r) => (
              <li key={r.id}>
                <Link
                  href={`/admin/users/${r.user_id}`}
                  className="glass focus-neon flex items-center justify-between gap-3 rounded-card p-3.5 transition-colors duration-200 ease-out hover:border-function/50"
                >
                  <span className="text-sm text-ink">{r.persona_name ?? r.steam_id}</span>
                  <time dateTime={r.redeemed_at} title={exactTime(r.redeemed_at)} className="text-xs text-ink-faint">
                    {shortDate(r.redeemed_at)}
                  </time>
                </Link>
              </li>
            ))}
          </ul>
        )}
      </section>

      <ConfirmDialog
        open={confirming}
        title={`Deactivate ${data.code}?`}
        description="Nobody will be able to redeem this code. Existing redemptions are not affected."
        confirmLabel="Deactivate"
        tone="danger"
        onClose={() => setConfirming(false)}
        onConfirm={async () => {
          await deactivateVoucher(id);
          setData(await getVoucher(id));
        }}
      />
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
      All vouchers
    </Link>
  );
}
