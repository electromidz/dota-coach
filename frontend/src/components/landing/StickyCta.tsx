"use client";

import { useEffect, useState } from "react";

import { ButtonLink } from "@/components/ui/Button";
import { Icon } from "@/components/ui/Icon";
import { steamLoginUrl } from "@/lib/api";
import { cn } from "@/lib/utils";

/**
 * A "Start free trial" bar pinned to the bottom of the screen on phones,
 * shown once the hero's own button has scrolled away. Hidden from `sm` up,
 * where the nav carries a sign-in button.
 */
export function StickyCta() {
  const [shown, setShown] = useState(false);

  useEffect(() => {
    const onScroll = () => setShown(window.scrollY > 560);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <div
      aria-hidden={!shown}
      className={cn(
        "fixed inset-x-0 bottom-0 z-40 border-t border-glass-edge bg-base/90 px-4 pt-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] backdrop-blur sm:hidden",
        "transition-transform duration-200 motion-reduce:transition-none",
        shown ? "translate-y-0" : "pointer-events-none translate-y-full",
      )}
    >
      <ButtonLink href={steamLoginUrl()} tabIndex={shown ? 0 : -1} className="w-full whitespace-nowrap">
        <Icon name="steam" className="size-5" />
        Start free trial
      </ButtonLink>
    </div>
  );
}
