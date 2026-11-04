"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { CopyButton } from "@/components/ui/CopyButton";
import { DataTable, type Column } from "@/components/ui/DataTable";
import { Icon } from "@/components/ui/Icon";
import { RelativeTime } from "@/components/ui/RelativeTime";
import { Segmented } from "@/components/ui/Segmented";
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
  vouchersToCsv,
} from "@/lib/admin/format";
import { createVouchers, deactivateVoucher, listAllVouchers, listVouchers } from "@/lib/admin/queries";
import type { Voucher, VoucherList, VoucherListItem, VoucherStatus } from "@/lib/admin/types";
import { useUrlState } from "@/lib/admin/url-state";
import { cn } from "@/lib/utils";

const PAGE_SIZE = 20;
const STATUSES: VoucherStatus[] = ["active", "used", "expired", "deactivated"];

export function VoucherTable() {
  const [params, setParams] = useUrlState();
  const statusParam = params.get("status") as VoucherStatus | null;
  const status: VoucherStatus | "" = statusParam && STATUSES.includes(statusParam) ? statusParam : "";
  const page = Math.max(1, Number(params.get("page")) || 1);

  const key = `${status}|${page}`;
  const [result, setResult] = useState<{ key: string; data: VoucherList } | null>(null);
  const [failure, setFailure] = useState<{ key: string; message: string } | null>(null);
  const [version, setVersion] = useState(0);
  const [created, setCreated] = useState<Voucher[] | null>(null);
  const [confirming, setConfirming] = useState<VoucherListItem | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    listVouchers({ page, limit: PAGE_SIZE, status })
      .then((data) => !cancelled && setResult({ key, data }))
      .catch((e) => {
        if (!cancelled) setFailure({ key, message: e instanceof ApiError ? e.message : "Could not load vouchers." });
      });
    return () => {
      cancelled = true;
    };
  }, [key, page, status, version]);

  const refresh = useCallback(() => setVersion((v) => v + 1), []);
  const error = failure?.key === key ? failure.message : null;
  const loading = result?.key !== key && !error;
  const data = result?.data ?? null;

  async function exportCsv() {
    setExporting(true);
    setExportError(null);
    try {
      const all = await listAllVouchers(status);
      const blob = new Blob([vouchersToCsv(all)], { type: "text/csv" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `vouchers-${status || "all"}-${new Date().toISOString().slice(0, 10)}.csv`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
    } catch (e) {
      setExportError(e instanceof ApiError ? e.message : "Could not export vouchers.");
    } finally {
      setExporting(false);
    }
  }

  const columns: Column<VoucherListItem>[] = [
    {
      id: "code",
      header: "Code",
      rowHeader: true,
      cell: (v) => (
        <span className="flex items-center gap-1">
          <Link
            href={`/admin/vouchers/${v.id}`}
            className="focus-neon rounded font-mono text-sm text-ink hover:text-function"
          >
            {v.code}
          </Link>
          <CopyButton value={v.code} label={`code ${v.code}`} />
        </span>
      ),
    },
    {
      id: "status",
      header: "Status",
      cell: (v) => (
        <StatusBadge label="Voucher status" tone={voucherTone(voucherStatus(v))}>
          {VOUCHER_STATUS_LABELS[voucherStatus(v)]}
        </StatusBadge>
      ),
    },
    {
      id: "duration",
      header: "Grants",
      cell: (v) => <span className="text-ink-muted">{durationLabel(v.duration_days)}</span>,
    },
    {
      id: "redeemed",
      header: "Redeemed by",
      cell: (v) =>
        v.redeemed_by ? (
          <span className="flex flex-col">
            <Link
              href={`/admin/users/${v.redeemed_by.user_id}`}
              className="focus-neon w-fit rounded text-ink hover:text-function"
            >
              {v.redeemed_by.persona_name ?? v.redeemed_by.steam_id}
            </Link>
            <time
              dateTime={v.redeemed_by.redeemed_at}
              title={exactTime(v.redeemed_by.redeemed_at)}
              className="text-xs text-ink-faint"
            >
              {shortDate(v.redeemed_by.redeemed_at)}
            </time>
          </span>
        ) : (
          <span className="text-ink-faint">—</span>
        ),
    },
    {
      id: "expires",
      header: "Expires",
      cell: (v) =>
        showsExpiry(v) && v.expires_at ? (
          <RelativeTime iso={v.expires_at} className="text-ink-muted" />
        ) : (
          <span className="text-ink-faint">{v.expires_at === null && voucherStatus(v) === "active" ? "Never" : "—"}</span>
        ),
    },
    {
      id: "note",
      header: "Note",
      cell: (v) => <span className="block max-w-48 truncate text-ink-faint" title={v.note ?? undefined}>{v.note ?? ""}</span>,
    },
    {
      id: "actions",
      header: "",
      align: "right",
      cell: (v) =>
        canDeactivate(v) ? (
          <Button
            variant="ghost"
            onClick={() => setConfirming(v)}
            className="min-h-9 px-3 text-xs"
            aria-label={`Deactivate ${v.code}`}
          >
            Deactivate
          </Button>
        ) : null,
    },
  ];

  return (
    <div className="flex flex-col gap-6">
      <CreateVoucherForm
        onCreated={(vouchers) => {
          setCreated(vouchers);
          // Back to the full list, so the new codes are on screen.
          setParams({ status: null, page: null });
          refresh();
        }}
      />
      {created ? <NewCodesPanel vouchers={created} onDismiss={() => setCreated(null)} /> : null}

      <div className="flex flex-wrap items-center justify-between gap-3">
        <Segmented<VoucherStatus | "">
          label="Voucher status"
          value={status}
          options={[
            { value: "", label: "All" },
            ...STATUSES.map((s) => ({ value: s, label: VOUCHER_STATUS_LABELS[s] })),
          ]}
          onChange={(next) => setParams({ status: next, page: null })}
        />
        <div className="flex items-center gap-3">
          {data ? (
            <span className="font-mono text-xs tabular-nums text-ink-faint" aria-live="polite">
              {data.total.toLocaleString("en-US")} voucher{data.total === 1 ? "" : "s"}
            </span>
          ) : null}
          <Button
            variant="ghost"
            disabled={!data || data.total === 0 || exporting}
            onClick={() => void exportCsv()}
            aria-busy={exporting}
            className="min-h-9 px-4 text-xs"
          >
            <Icon name="download" className="size-4" />
            {exporting ? "Exporting…" : `Export ${data?.total ?? ""} as CSV`}
          </Button>
        </div>
      </div>

      {error ? <Alert>{error}</Alert> : null}
      {exportError ? <Alert>{exportError}</Alert> : null}

      <DataTable
        caption="Vouchers"
        columns={columns}
        rows={data?.vouchers ?? null}
        rowKey={(v) => v.id}
        page={page}
        totalPages={data?.total_pages}
        onPage={(next) => setParams({ page: next === 1 ? null : next })}
        loading={loading}
        empty={status ? `No ${VOUCHER_STATUS_LABELS[status].toLowerCase()} vouchers.` : "No vouchers yet."}
      />

      <ConfirmDialog
        open={confirming !== null}
        title={`Deactivate ${confirming?.code ?? ""}?`}
        description="Nobody will be able to redeem this code. Existing redemptions are not affected."
        confirmLabel="Deactivate"
        tone="danger"
        onClose={() => setConfirming(null)}
        onConfirm={async () => {
          if (!confirming) return;
          await deactivateVoucher(confirming.id);
          refresh();
        }}
      />
    </div>
  );
}

function CreateVoucherForm({ onCreated }: { onCreated: (vouchers: Voucher[]) => void }) {
  const [durationDays, setDurationDays] = useState("30");
  const [count, setCount] = useState("1");
  const [note, setNote] = useState("");
  const [expiresAt, setExpiresAt] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const days = Number(durationDays);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      onCreated(
        await createVouchers({
          duration_days: days,
          count: Number(count),
          note: note.trim() || undefined,
          expires_at: expiresAt ? `${expiresAt}T23:59:59Z` : undefined,
        }),
      );
      setNote("");
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not create the vouchers.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className="flex flex-col gap-4">
      <div>
        <h2 className="text-xs uppercase tracking-wider text-ink-faint">Create vouchers</h2>
        <p className="mt-1 text-sm text-ink-faint">Each code can be redeemed once, by one account.</p>
      </div>
      {error ? <Alert>{error}</Alert> : null}
      <form onSubmit={handleSubmit} className="flex flex-wrap items-end gap-3">
        <Field label="Grants (days)" hint={days > 0 ? durationLabel(days) : undefined}>
          <input type="number" min={1} max={3650} required value={durationDays} onChange={(e) => setDurationDays(e.target.value)} className="w-28" />
        </Field>
        <Field label="How many codes">
          <input type="number" min={1} max={1000} required value={count} onChange={(e) => setCount(e.target.value)} className="w-28" />
        </Field>
        <Field label="Expires (optional)">
          <input type="date" value={expiresAt} onChange={(e) => setExpiresAt(e.target.value)} className="w-40" />
        </Field>
        <Field label="Note (optional)" className="min-w-48 flex-1">
          <input type="text" value={note} onChange={(e) => setNote(e.target.value)} placeholder="Only admins see this" className="w-full" />
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
  hint,
  className,
  children,
}: {
  label: string;
  hint?: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <label
      className={cn(
        "flex flex-col gap-1 text-xs text-ink-faint [&_input]:min-h-11 [&_input]:rounded-xl [&_input]:border [&_input]:border-glass-edge [&_input]:bg-surface-2/60 [&_input]:px-3 [&_input]:text-sm [&_input]:text-ink [&_input]:outline-none [&_input]:focus-neon",
        className,
      )}
    >
      <span>
        {label}
        {hint ? <span className="text-ink-muted"> · {hint}</span> : null}
      </span>
      {children}
    </label>
  );
}

/** The codes just created, while they are easy to hand out. */
function NewCodesPanel({ vouchers, onDismiss }: { vouchers: Voucher[]; onDismiss: () => void }) {
  const all = vouchers.map((v) => v.code).join("\n");

  return (
    <Card glow="string" className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-sm text-ink" aria-live="polite">
          {vouchers.length === 1 ? "1 voucher created" : `${vouchers.length} vouchers created`}
          <span className="text-ink-faint"> · {durationLabel(vouchers[0]?.duration_days ?? 0)} each</span>
        </h2>
        <div className="flex items-center gap-2">
          <span className="flex items-center gap-1 rounded-lg border border-glass-edge pl-3 text-xs text-ink">
            Copy all
            <CopyButton value={all} label={`all ${vouchers.length} codes`} />
          </span>
          <button
            type="button"
            onClick={onDismiss}
            className="focus-neon min-h-9 cursor-pointer rounded px-2 text-xs text-ink-faint transition-colors hover:text-ink"
          >
            Dismiss
          </button>
        </div>
      </div>
      <ul className="grid grid-cols-1 gap-1.5 sm:grid-cols-2 lg:grid-cols-3">
        {vouchers.map((v) => (
          <li key={v.id} className="flex items-center justify-between gap-2 rounded-lg border border-glass-edge px-3 py-1.5">
            <span className="font-mono text-sm text-ink">{v.code}</span>
            <CopyButton value={v.code} label={`code ${v.code}`} />
          </li>
        ))}
      </ul>
    </Card>
  );
}
