"use client";

import { useEffect, useId, useRef, useState } from "react";

import { cn } from "@/lib/utils";

export interface ComboOption {
  value: string;
  label: string;
  /** Shown beside the label, e.g. a game count. */
  hint?: string;
}

/**
 * A searchable single-select. Type to narrow, arrow keys to move, Enter to
 * pick, Escape to close. The first option is "any".
 */
export function Combobox({
  label,
  value,
  options,
  anyLabel,
  onChange,
}: {
  label: string;
  value: string | undefined;
  options: ComboOption[];
  /** The option meaning "no filter". */
  anyLabel: string;
  onChange: (value: string | undefined) => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  const id = useId();

  const selected = options.find((o) => o.value === value);
  const matches = options.filter((o) =>
    o.label.toLowerCase().includes(query.trim().toLowerCase()),
  );
  const items: (ComboOption | null)[] = [null, ...matches];

  useEffect(() => {
    if (!open) return;
    function onPointer(e: PointerEvent) {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("pointerdown", onPointer);
    return () => document.removeEventListener("pointerdown", onPointer);
  }, [open]);

  function pick(option: ComboOption | null) {
    onChange(option?.value);
    setOpen(false);
    setQuery("");
  }

  return (
    <div ref={root} className="relative">
      <input
        role="combobox"
        aria-label={label}
        aria-expanded={open}
        aria-controls={`${id}-list`}
        aria-activedescendant={open ? `${id}-${active}` : undefined}
        value={open ? query : (selected?.label ?? anyLabel)}
        placeholder={anyLabel}
        onFocus={() => {
          setOpen(true);
          setActive(0);
        }}
        onChange={(e) => {
          setQuery(e.target.value);
          setActive(0);
          setOpen(true);
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setActive((i) => Math.min(items.length - 1, i + 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setActive((i) => Math.max(0, i - 1));
          } else if (e.key === "Enter" && open) {
            e.preventDefault();
            pick(items[active] ?? null);
          } else if (e.key === "Escape") {
            setOpen(false);
            setQuery("");
          }
        }}
        className="focus-neon min-h-9 w-44 rounded-xl border border-glass-edge bg-surface-2/60 px-3 text-xs text-ink outline-none placeholder:text-ink-muted"
      />
      {open ? (
        <ul
          id={`${id}-list`}
          role="listbox"
          aria-label={label}
          className="absolute left-0 top-full z-30 mt-1 max-h-72 w-60 overflow-y-auto rounded-xl border border-glass-edge bg-surface p-1 shadow-xl"
        >
          {items.map((option, i) => (
            <li
              key={option?.value ?? "any"}
              id={`${id}-${i}`}
              role="option"
              aria-selected={(option?.value ?? undefined) === value}
              onPointerDown={(e) => e.preventDefault()}
              onClick={() => pick(option)}
              onMouseEnter={() => setActive(i)}
              className={cn(
                "flex cursor-pointer items-center justify-between gap-2 rounded-lg px-2.5 py-1.5 text-xs",
                i === active ? "bg-ink/10 text-ink" : "text-ink-muted",
              )}
            >
              <span className="truncate">{option?.label ?? anyLabel}</span>
              {option?.hint ? (
                <span className="font-mono tabular-nums text-ink-faint">{option.hint}</span>
              ) : null}
            </li>
          ))}
          {matches.length === 0 ? (
            <li className="px-2.5 py-1.5 text-xs text-ink-faint">No hero matches</li>
          ) : null}
        </ul>
      ) : null}
    </div>
  );
}
