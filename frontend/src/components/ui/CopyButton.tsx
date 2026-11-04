"use client";

import { useEffect, useState } from "react";

import { Icon } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

/**
 * Selection-based copy first, synchronously, while the click still counts as
 * a user gesture; then the async clipboard API. The async API alone can sit
 * pending behind a permission prompt and leave the button silent, and the
 * selection copy only works before anything has been awaited.
 */
function copyBySelection(text: string): boolean {
  const field = document.createElement("textarea");
  field.value = text;
  field.setAttribute("readonly", "");
  field.style.position = "fixed";
  field.style.opacity = "0";
  document.body.appendChild(field);
  field.select();
  try {
    return document.execCommand("copy");
  } catch {
    return false;
  } finally {
    field.remove();
  }
}

async function writeClipboard(text: string): Promise<boolean> {
  if (copyBySelection(text)) return true;
  if (!navigator.clipboard?.writeText) return false;

  const result = await Promise.race([
    navigator.clipboard.writeText(text).then(
      () => true,
      () => false,
    ),
    new Promise<false>((resolve) => window.setTimeout(() => resolve(false), 1500)),
  ]);
  return result;
}

/**
 * Copies `value` and confirms with a small "Copied" chip beside the icon.
 *
 * The confirmation is announced through a polite live region, so a screen
 * reader hears it without focus moving. A refused clipboard (insecure origin,
 * denied permission) says so instead of pretending.
 */
export function CopyButton({ value, label }: { value: string; label: string }) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");

  useEffect(() => {
    if (state === "idle") return;
    const timer = window.setTimeout(() => setState("idle"), 1800);
    return () => window.clearTimeout(timer);
  }, [state]);

  async function copy() {
    setState((await writeClipboard(value)) ? "copied" : "failed");
  }

  return (
    <span className="relative inline-flex items-center">
      <button
        type="button"
        onClick={copy}
        aria-label={`Copy ${label}`}
        title={`Copy ${label}`}
        className="focus-neon inline-flex size-8 cursor-pointer items-center justify-center rounded-lg text-ink-faint transition-colors duration-200 hover:bg-ink/10 hover:text-ink"
      >
        <Icon name={state === "copied" ? "check" : "copy"} className="size-4" />
      </button>
      <span
        role="status"
        aria-live="polite"
        className={cn(
          "pointer-events-none absolute left-full z-10 ml-1 whitespace-nowrap rounded-md border border-glass-edge bg-surface px-2 py-0.5 text-[0.6875rem]",
          state === "idle" && "invisible",
          state === "failed" ? "text-error" : "text-string",
        )}
      >
        {state === "copied" ? "Copied" : state === "failed" ? "Copy failed" : ""}
      </span>
    </span>
  );
}
