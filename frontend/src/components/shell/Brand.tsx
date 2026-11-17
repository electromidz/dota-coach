import Link from "next/link";

import { cn } from "@/lib/utils";

/**
 * Wordmark plus the small "Ascent" mark.
 *
 * Inline SVG rather than an image file: it is a handful of shapes, it reads
 * the theme tokens, and it costs no extra request at any breakpoint.
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
    // "Ascent": same geometry as public/icons/icon.svg, cropped to the mark so
    // it reads at header size. Start dot, rising match trajectory, focus ring.
    <svg
      viewBox="72 72 368 368"
      fill="none"
      aria-hidden
      className={cn("shrink-0", className)}
    >
      <defs>
        <linearGradient
          id="brand-mark-line"
          x1="136"
          y1="376"
          x2="368"
          y2="152"
          gradientUnits="userSpaceOnUse"
        >
          <stop offset="0" stopColor="var(--color-good)" />
          <stop offset="0.45" stopColor="var(--color-keyword)" />
          <stop offset="1" stopColor="var(--color-function)" />
        </linearGradient>
      </defs>
      <rect
        x="88"
        y="88"
        width="336"
        height="336"
        rx="56"
        stroke="var(--color-border)"
        strokeWidth="16"
      />
      <path
        d="M136 376L216 288L272 320L335 209"
        stroke="url(#brand-mark-line)"
        strokeWidth="38"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="136" cy="376" r="28" fill="var(--color-good)" />
      <circle
        cx="368"
        cy="152"
        r="56"
        stroke="var(--color-keyword)"
        strokeWidth="22"
      />
      <circle cx="368" cy="152" r="22" fill="var(--color-function)" />
    </svg>
  );
}
