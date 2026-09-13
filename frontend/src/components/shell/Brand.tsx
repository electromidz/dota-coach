import Link from "next/link";

import { cn } from "@/lib/utils";

/**
 * Wordmark plus a small crosshair mark.
 *
 * Inline SVG rather than an image file: it is six path commands, it inherits
 * `currentColor`, and it costs no extra request at any breakpoint.
 */
export function Brand({
  className,
  href = "/",
}: {
  className?: string;
  href?: string;
}) {
  return (
    <Link
      href={href}
      className={cn(
        "focus-neon flex w-fit shrink-0 cursor-pointer items-center gap-2.5 rounded",
        className,
      )}
    >
      <Mark className="size-7" />
      {/* Not `text-base`: `--color-base` shadows that utility, so it sets the
          near-black page colour rather than a font size. */}
      <span className="font-display text-[1.0625rem] tracking-wide">
        <span className="bg-gradient-to-r from-keyword to-function bg-clip-text text-transparent">
          Dota
        </span>{" "}
        Coach
      </span>
    </Link>
  );
}

export function Mark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden
      className={cn("shrink-0", className)}
    >
      <circle
        cx="12"
        cy="12"
        r="9"
        stroke="var(--color-keyword)"
        strokeWidth="1.6"
        opacity="0.85"
      />
      <circle cx="12" cy="12" r="3.2" fill="var(--color-function)" />
      <path
        d="M12 1.5v4M12 18.5v4M1.5 12h4M18.5 12h4"
        stroke="var(--color-function)"
        strokeWidth="1.6"
        strokeLinecap="round"
      />
    </svg>
  );
}
