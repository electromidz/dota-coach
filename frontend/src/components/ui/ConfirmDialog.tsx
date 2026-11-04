"use client";

import { useEffect, useId, useRef, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { cn } from "@/lib/utils";

/**
 * A confirmation step for an action that is hard to take back.
 *
 * Built on the native `<dialog>` in modal mode, which gives focus trapping,
 * Escape to cancel, an inert page behind it and focus returned to the trigger
 * — all without re-implementing any of it.
 *
 * `withReason` adds an optional free-text reason that is passed to
 * `onConfirm`. Errors from `onConfirm` are shown inside the dialog, which
 * stays open so the admin can retry or cancel.
 */
export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel,
  tone = "default",
  withReason = false,
  onConfirm,
  onClose,
  children,
}: {
  open: boolean;
  title: string;
  description?: React.ReactNode;
  confirmLabel: string;
  tone?: "default" | "danger";
  withReason?: boolean;
  /** Resolve to close; throw to keep the dialog open with the message. */
  onConfirm: (reason: string) => Promise<void>;
  onClose: () => void;
  /** Extra controls, e.g. a choice of how many days. */
  children?: React.ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const id = useId();
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    // `showModal` is missing in jsdom and very old browsers; fall back to a
    // plain open dialog rather than throwing.
    if (open && !dialog.open) {
      if (typeof dialog.showModal === "function") dialog.showModal();
      else dialog.setAttribute("open", "");
    }
    if (!open && dialog.open) {
      if (typeof dialog.close === "function") dialog.close();
      else dialog.removeAttribute("open");
    }
  }, [open]);

  function reset() {
    setReason("");
    setError(null);
    setBusy(false);
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await onConfirm(reason.trim());
      reset();
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Something went wrong.");
      setBusy(false);
    }
  }

  return (
    <dialog
      ref={ref}
      aria-labelledby={`${id}-title`}
      aria-describedby={description ? `${id}-desc` : undefined}
      // Escape and the backdrop both arrive here; keep React state in step.
      onClose={() => {
        reset();
        onClose();
      }}
      onCancel={(e) => {
        if (busy) e.preventDefault();
      }}
      className="m-auto w-[min(28rem,calc(100vw-2rem))] rounded-card border border-glass-edge bg-surface p-0 text-ink shadow-2xl backdrop:bg-base/70 backdrop:backdrop-blur-sm"
    >
      <form onSubmit={submit} className="flex flex-col gap-4 p-5 sm:p-6">
        <h2 id={`${id}-title`} className="font-display text-lg tracking-wide">
          {title}
        </h2>
        {description ? (
          <div id={`${id}-desc`} className="text-sm text-ink-muted">
            {description}
          </div>
        ) : null}

        {children}

        {withReason ? (
          <div className="flex flex-col gap-1.5 text-xs text-ink-muted">
            <label htmlFor={`${id}-reason`}>
              Reason <span className="text-ink-faint">(optional, shown in the activity feed)</span>
            </label>
            <textarea
              id={`${id}-reason`}
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              maxLength={500}
              rows={3}
              className="rounded-xl border border-glass-edge bg-surface-2/60 px-3 py-2 text-sm text-ink outline-none focus-neon"
            />
          </div>
        ) : null}

        {error ? <Alert>{error}</Alert> : null}

        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button
            variant="ghost"
            onClick={() => {
              reset();
              onClose();
            }}
            disabled={busy}
            className="px-5 text-sm"
          >
            Cancel
          </Button>
          <Button
            type="submit"
            disabled={busy}
            aria-busy={busy}
            className={cn(
              "px-5 text-sm",
              tone === "danger" && "bg-error shadow-none hover:shadow-none",
            )}
          >
            {busy ? "Working…" : confirmLabel}
          </Button>
        </div>
      </form>
    </dialog>
  );
}
