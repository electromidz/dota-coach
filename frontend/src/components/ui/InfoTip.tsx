"use client";

import { useEffect, useId, useRef, useState } from "react";

import { Icon, type IconName } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

/**
 * An (i) button that holds the explanation a card would otherwise print.
 *
 * Opens on hover for a mouse and on tap or keyboard for everyone else; Escape
 * or a tap outside closes it. The content stays in the DOM only while open.
 */
export function InfoTip({
  label,
  children,
  align = "start",
  className,
  icon = "info",
  iconClassName,
}: {
  /** Names the button for screen readers, e.g. "About this chart". */
  label: string;
  children: React.ReactNode;
  align?: "start" | "end";
  className?: string;
  /** The trigger glyph; "info" by default. */
  icon?: IconName;
  iconClassName?: string;
}) {
  const [open, setOpen] = useState(false);
  const id = useId();
  const root = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    function onPointer(e: PointerEvent) {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onPointer);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onPointer);
    };
  }, [open]);

  return (
    <span
      ref={root}
      className={cn("relative inline-flex", className)}
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
    >
      <button
        type="button"
        aria-label={label}
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => setOpen((shown) => !shown)}
        className="focus-neon -m-1.5 inline-flex cursor-pointer rounded-full p-1.5 text-ink-faint transition-colors duration-200 hover:text-ink"
      >
        <Icon name={icon} className={cn("size-3.5", iconClassName)} />
      </button>
      {open ? (
        <span
          id={id}
          role="note"
          className={cn(
            "absolute top-full z-30 mt-2 flex w-72 max-w-[calc(100vw-2rem)] flex-col gap-2",
            "rounded-xl border border-glass-edge bg-surface p-3 text-left text-xs font-normal normal-case leading-relaxed tracking-normal text-ink-muted shadow-xl",
            align === "end" ? "right-0" : "left-0",
          )}
        >
          {children}
        </span>
      ) : null}
    </span>
  );
}
