"use client";

import { Icon } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

/**
 * How many numbered pages the bar offers at once.
 *
 * Enough to step a few pages without the control wrapping on a phone. A history
 * of two hundred games is ten pages, so a longer strip would mostly be paging
 * past games the player does not remember.
 */
const WINDOW = 6;

/**
 * Numbered pagination, as pure props.
 *
 * Nothing here knows about matches, fetching or the URL — it reports which page
 * was asked for and the owner decides what that means. That is what keeps the
 * window arithmetic below testable on its own.
 */
export function Pagination({
  page,
  totalPages,
  disabled = false,
  onChange,
}: {
  page: number;
  totalPages: number;
  disabled?: boolean;
  onChange: (page: number) => void;
}) {
  if (totalPages <= 1) return null;

  const pages = pageWindow(page, totalPages);

  return (
    <nav
      aria-label="Match history pages"
      className="flex flex-wrap items-center justify-center gap-1.5"
    >
      <Step
        label="Prev"
        icon="chevron-left"
        disabled={disabled || page <= 1}
        onClick={() => onChange(page - 1)}
      />

      {pages.map((target) => (
        <button
          key={target}
          type="button"
          disabled={disabled}
          onClick={() => onChange(target)}
          aria-label={`Page ${target}`}
          aria-current={target === page ? "page" : undefined}
          className={cn(
            "focus-neon min-h-9 min-w-9 cursor-pointer rounded-lg border px-2",
            "font-mono text-xs tabular-nums transition-colors duration-200 ease-out",
            "disabled:cursor-not-allowed disabled:opacity-60",
            target === page
              ? "border-function/60 bg-function/10 text-ink"
              : "border-glass-edge text-ink-muted hover:text-ink",
          )}
        >
          {target}
        </button>
      ))}

      <Step
        label="Next"
        icon="chevron-right"
        trailing
        disabled={disabled || page >= totalPages}
        onClick={() => onChange(page + 1)}
      />

      <span className="ml-1 w-full text-center font-mono text-xs tabular-nums text-ink-faint sm:ml-2 sm:w-auto">
        Page {page} of {totalPages}
      </span>
    </nav>
  );
}

/**
 * The numbers to show around the current page.
 *
 * Kept full-width at the ends rather than shrinking: on page one a window that
 * simply centred on the current page would waste half its slots on pages that do
 * not exist, and the strip would visibly change size as the reader moves.
 */
export function pageWindow(page: number, totalPages: number): number[] {
  const size = Math.min(WINDOW, totalPages);
  // Centre the window, then push it back inside both ends.
  const start = Math.min(
    Math.max(1, page - Math.floor((size - 1) / 2)),
    totalPages - size + 1,
  );

  return Array.from({ length: size }, (_, i) => start + i);
}

function Step({
  label,
  icon,
  trailing = false,
  disabled,
  onClick,
}: {
  label: string;
  icon: "chevron-left" | "chevron-right";
  trailing?: boolean;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "focus-neon flex min-h-9 cursor-pointer items-center gap-1 rounded-lg border border-glass-edge",
        "px-2.5 text-xs uppercase tracking-wider text-ink-muted",
        "transition-colors duration-200 ease-out hover:text-ink",
        "disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:text-ink-muted",
      )}
    >
      {trailing ? null : <Icon name={icon} className="size-3.5" />}
      {label}
      {trailing ? <Icon name={icon} className="size-3.5" /> : null}
    </button>
  );
}
