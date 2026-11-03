"use client";

import { useState } from "react";

import { heroPortraitUrl } from "@/lib/heroes";

/** A hero portrait cropped to fill a circle; initials if it cannot load. */
export function HeroDot({ heroId, heroName }: { heroId: number; heroName: string }) {
  const [failed, setFailed] = useState(false);
  const src = heroPortraitUrl(heroId);
  if (!src || failed) {
    return (
      <span className="flex size-full items-center justify-center bg-surface-2 text-[0.625rem] text-ink-muted">
        {heroName.slice(0, 2)}
      </span>
    );
  }
  return (
    // eslint-disable-next-line @next/next/no-img-element
    <img
      src={src}
      alt=""
      className="size-full object-cover"
      loading="lazy"
      onError={() => setFailed(true)}
    />
  );
}
