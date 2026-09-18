"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { voucherStateLabel, vouchersToCsv } from "@/lib/admin";
import { ApiError, createVouchers, deactivateVoucher, getAdminVouchers } from "@/lib/api";
import type { Voucher, VoucherListResponse } from "@/lib/types";
import { cn, timeAgo } from "@/lib/utils";

const PAGE_SIZE = 20;

export function VoucherTable() {
  const [data, setData] = useState<VoucherListResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [justCreated, setJustCreated] = useState<Voucher[] | null>(null);

  const load = useCallback(async (targetPage: number) => {
    try {
      setData(await getAdminVouchers(targetPage, PAGE_SIZE));
      setError(null);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not load vouchers.");
    }
  }, []);

  useEffect(() => {
    void load(1);
  }, [load]);

  function goTo(target: number) {
    setPage(target);
    void load(target);
  }

  async function handleCreated(vouchers: Voucher[]) {
    setJustCreated(vouchers);
    setPage(1);
    await load(1);
  }

  async function handleDeactivate(id: string) {
    if (!window.confirm("Deactivate this voucher? Existing redemptions are unaffected.")) {
      return;
    }
    try {
      await deactivateVoucher(id);
      await load(page);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not deactivate this voucher.");
    }
  }

  function exportCsv() {
    if (!data || data.vouchers.length === 0) return;

    const blob = new Blob([vouchersToCsv(data.vouchers)], { type: "text/csv" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `vouchers-page-${data.page}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }

  return (
    <div className="flex flex-col gap-6 pb-4">
      <CreateVoucherForm onCreated={handleCreated} />

      {justCreated ? (
        <NewCodesPanel vouchers={justCreated} onDismiss={() => setJustCreated(null)} />
      ) : null}

      {error ? <Alert>{error}</Alert> : null}

      {!data ? (
        <TableSkeleton />
      ) : (
        <>
          <div className="flex items-center justify-between gap-3">
            <p className="font-mono text-xs tabular-nums text-ink-faint" aria-live="polite">
              {data.total} voucher{data.total === 1 ? "" : "s"}
            </p>
            <Button
              variant="ghost"
              disabled={data.vouchers.length === 0}
              onClick={exportCsv}
              className="px-4 py-2 text-xs"
            >
              <Icon name="download" className="size-4" />
              Export page as CSV
            </Button>
          </div>

          {data.vouchers.length === 0 ? (
            <Card>
              <p className="text-sm text-ink-muted">No vouchers yet.</p>
            </Card>
          ) : (
            <ul className="flex flex-col gap-2">
              {data.vouchers.map((voucher) => (
                <VoucherRow
                  key={voucher.id}
                  voucher={voucher}
                  onDeactivate={() => void handleDeactivate(voucher.id)}
                />
              ))}
            </ul>
          )}

          {data.total_pages > 1 ? (
            <nav
              aria-label="Voucher pages"
              className="flex items-center justify-between gap-3 pt-1 lg:justify-center lg:gap-6"
            >
              <Button
                variant="ghost"
                disabled={page <= 1}
                onClick={() => goTo(page - 1)}
                className="px-4 text-sm"
              >
                <Icon name="chevron-left" className="size-4" />
                Prev
              </Button>
              <span className="font-mono text-xs tabular-nums text-ink-faint">
                {page} / {data.total_pages}
              </span>
              <Button
                variant="ghost"
                disabled={page >= data.total_pages}
                onClick={() => goTo(page + 1)}
                className="px-4 text-sm"
              >
                Next
                <Icon name="chevron-right" className="size-4" />
              </Button>
            </nav>
          ) : null}
        </>
      )}
    </div>
  );
}

function CreateVoucherForm({ onCreated }: { onCreated: (vouchers: Voucher[]) => void }) {
  const [durationDays, setDurationDays] = useState("30");
  const [maxUses, setMaxUses] = useState("1");
  const [count, setCount] = useState("1");
  const [note, setNote] = useState("");
  const [expiresAt, setExpiresAt] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const response = await createVouchers({
        duration_days: Number(durationDays),
        max_uses: Number(maxUses),
        count: Number(count),
        note: note.trim() || undefined,
        expires_at: expiresAt ? `${expiresAt}T23:59:59Z` : undefined,
      });
      onCreated(response.vouchers);
      setNote("");
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not create the voucher.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className="flex flex-col gap-4">
      <h2 className="text-xs uppercase tracking-wider text-ink-faint">Create vouchers</h2>

      {error ? <Alert>{error}</Alert> : null}

      <form onSubmit={handleSubmit} className="flex flex-wrap items-end gap-3">
        <Field label="Duration (days)">
          <input
            type="number"
            min={1}
            required
            value={durationDays}
            onChange={(e) => setDurationDays(e.target.value)}
            className="w-24"
          />
        </Field>
        <Field label="Max uses">
          <input
            type="number"
            min={1}
            required
            value={maxUses}
            onChange={(e) => setMaxUses(e.target.value)}
            className="w-24"
          />
        </Field>
        <Field label="How many codes">
          <input
            type="number"
            min={1}
            max={1000}
            required
            value={count}
            onChange={(e) => setCount(e.target.value)}
            className="w-24"
          />
        </Field>
        <Field label="Expires (optional)">
          <input
            type="date"
            value={expiresAt}
            onChange={(e) => setExpiresAt(e.target.value)}
            className="w-40"
          />
        </Field>
        <Field label="Note (optional)" className="min-w-48 flex-1">
          <input
            type="text"
            value={note}
            onChange={(e) => setNote(e.target.value)}
            placeholder="Not shown to the user who redeems it"
            className="w-full"
          />
        </Field>

        <Button type="submit" disabled={busy} className="px-5 text-sm">
          <Icon name="plus" className="size-4" />
          {busy ? "Creating…" : "Create"}
        </Button>
      </form>
    </Card>
  );
}

function Field({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <label className={cn("flex flex-col gap-1 text-xs text-ink-faint [&_input]:min-h-11 [&_input]:rounded-xl [&_input]:border [&_input]:border-glass-edge [&_input]:bg-surface-2/60 [&_input]:px-3 [&_input]:text-sm [&_input]:text-ink [&_input]:outline-none [&_input]:focus-neon", className)}>
      {label}
      {children}
    </label>
  );
}

function NewCodesPanel({
  vouchers,
  onDismiss,
}: {
  vouchers: Voucher[];
  onDismiss: () => void;
}) {
  return (
    <Card glow="string" className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-sm text-ink">
          {vouchers.length === 1 ? "Voucher created" : `${vouchers.length} vouchers created`}
        </h2>
        <button
          type="button"
          onClick={onDismiss}
          className="focus-neon cursor-pointer text-xs text-ink-faint transition-colors hover:text-ink"
        >
          Dismiss
        </button>
      </div>
      <ul className="flex flex-col gap-1.5">
        {vouchers.map((v) => (
          <li key={v.id} className="flex items-center justify-between gap-3">
            <CopyableCode code={v.code} />
          </li>
        ))}
      </ul>
    </Card>
  );
}

function CopyableCode({ code }: { code: string }) {
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  return (
    <button
      type="button"
      onClick={() => void handleCopy()}
      className="focus-neon flex min-h-9 cursor-pointer items-center gap-2 rounded-lg border border-glass-edge px-3 font-mono text-sm text-ink transition-colors duration-200 ease-out hover:border-function/60"
    >
      {code}
      <Icon name={copied ? "check" : "copy"} className={cn("size-3.5", copied && "text-string")} />
    </button>
  );
}

function VoucherRow({
  voucher,
  onDeactivate,
}: {
  voucher: Voucher;
  onDeactivate: () => void;
}) {
  const state = voucherStateLabel(voucher);

  return (
    <li>
      <Card className="flex flex-wrap items-center justify-between gap-3 p-4">
        <div className="flex min-w-0 items-center gap-3">
          <Link
            href={`/admin/vouchers/${voucher.id}`}
            className="focus-neon cursor-pointer rounded"
          >
            <CopyableCode code={voucher.code} />
          </Link>
          {voucher.note ? (
            <span className="truncate text-xs text-ink-faint">{voucher.note}</span>
          ) : null}
        </div>

        <div className="flex shrink-0 items-center gap-3 text-xs">
          <span className="font-mono tabular-nums text-ink-faint">
            {voucher.used_count} / {voucher.max_uses} used
          </span>
          <span className="text-ink-faint">{voucher.duration_days}d</span>
          {voucher.expires_at ? (
            <span className="text-ink-faint">
              expires {timeAgo(voucher.expires_at)}
            </span>
          ) : null}
          <span
            className={cn(
              "rounded-lg border px-2.5 py-1",
              state === "Active"
                ? "border-glass-edge text-ink-faint"
                : "border-error/40 text-error",
            )}
          >
            {state}
          </span>
          <Button
            variant="ghost"
            disabled={state !== "Active"}
            onClick={onDeactivate}
            className="px-3 py-1.5 text-xs"
          >
            Deactivate
          </Button>
        </div>
      </Card>
    </li>
  );
}

function TableSkeleton() {
  return (
    <div className="flex flex-col gap-2" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading vouchers…</span>
      {[0, 1, 2, 3].map((i) => (
        <div key={i} className="glass h-16 animate-pulse rounded-card" />
      ))}
    </div>
  );
}
