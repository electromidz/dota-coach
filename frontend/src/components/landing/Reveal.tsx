"use client";

import { useInView } from "@/lib/useInView";
import { cn } from "@/lib/utils";

/**
 * Fade-and-rise scroll entrance, built entirely on the app's existing
 * `rise-in` keyframe (see `globals.css`) rather than a new animation — so
 * `prefers-reduced-motion` already flattens it for free.
 *
 * Renders `opacity-0` until the element enters the viewport, then swaps to
 * `rise-in`, whose `animation-fill-mode: both` holds the finished state.
 * `as` picks the wrapper tag so this never nests a `<div>` inside a `<p>`
 * or breaks a `<li>`/`<section>` boundary.
 */
export function Reveal({
  children,
  className,
  delayMs = 0,
  as: Tag = "div",
}: {
  children: React.ReactNode;
  className?: string;
  delayMs?: number;
  as?: "div" | "li" | "section" | "article";
}) {
  const { ref, inView } = useInView<HTMLDivElement>();

  return (
    <Tag
      ref={ref as never}
      style={inView && delayMs ? { animationDelay: `${delayMs}ms` } : undefined}
      className={cn(inView ? "rise-in" : "opacity-0", className)}
    >
      {children}
    </Tag>
  );
}
