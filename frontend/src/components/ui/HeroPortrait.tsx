"use client";

import { useState } from "react";

import { heroPortraitUrl } from "@/lib/heroes";
import { cn } from "@/lib/utils";

const SIZES = {
  sm: "h-9 w-16",
  md: "h-12 w-[5.25rem]",
  lg: "h-16 w-28",
} as const;

/**
 * Hero portrait from Valve's CDN, with a monogram fallback.
 *
 * Portraits are 16:9 crops, so the box matches rather than distorting them.
 * A missing hero id, an offline CDN or a blocked third-party image all land on
 * the same monogram — the row never collapses or shows a broken image.
 */
export function HeroPortrait({
  heroId,
  heroName,
  size = "md",
  className,
}: {
  heroId: number;
  heroName: string;
  size?: keyof typeof SIZES;
  className?: string;
}) {
  const [failed, setFailed] = useState(false);
  const src = heroPortraitUrl(heroId);

  return (
    <div
      className={cn(
        "relative shrink-0 overflow-hidden rounded-lg border border-glass-edge bg-surface-2",
        SIZES[size],
        className,
      )}
    >
      {src && !failed ? (
        // Plain <img>: Valve's CDN is not in the Next image allowlist, and
        // these are already small, correctly sized crops.
        // eslint-disable-next-line @next/next/no-img-element
        <img
          src={src}
          alt=""
          loading="lazy"
          decoding="async"
          onError={() => setFailed(true)}
          className="size-full object-cover"
        />
      ) : (
        <span className="flex size-full items-center justify-center font-display text-sm text-ink-faint">
          {heroName.slice(0, 2).toUpperCase()}
        </span>
      )}

      {/* Darkens the lower edge so overlaid text stays readable on bright
          portraits. */}
      <span
        aria-hidden
        className="absolute inset-0 bg-gradient-to-t from-base/60 to-transparent"
      />
    </div>
  );
}
