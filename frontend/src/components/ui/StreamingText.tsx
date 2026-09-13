"use client";

import { useEffect, useState } from "react";

import { cn } from "@/lib/utils";

/** Characters per second. Fast enough to read along with, not to wait on. */
const RATE = 45;

/**
 * Reveals text a character at a time, the way a model streams it.
 *
 * Accessibility rules this obeys:
 *  - The full string is always in the DOM for assistive tech; only a visual
 *    clone animates. A screen reader never hears a half-finished word.
 *  - `prefers-reduced-motion` skips straight to the finished text.
 *  - Layout is reserved by the full string, so nothing reflows as it types.
 */
export function StreamingText({
  text,
  className,
  startDelay = 0,
}: {
  text: string;
  className?: string;
  startDelay?: number;
}) {
  const [shown, setShown] = useState(0);
  const [instant, setInstant] = useState(false);

  useEffect(() => {
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
    if (reduced.matches) {
      setInstant(true);
      return;
    }

    setShown(0);
    let frame = 0;
    let start: number | null = null;

    const tick = (now: number) => {
      if (start === null) start = now;
      const elapsed = now - start - startDelay;

      if (elapsed < 0) {
        frame = requestAnimationFrame(tick);
        return;
      }

      const next = Math.min(text.length, Math.floor((elapsed / 1000) * RATE));
      setShown(next);

      if (next < text.length) frame = requestAnimationFrame(tick);
    };

    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [text, startDelay]);

  const done = instant || shown >= text.length;

  return (
    <span className={cn("relative inline-block", className)}>
      {/* The real text: present for screen readers and for layout, hidden from
          sight until the reveal finishes so there is no double image. */}
      <span className={cn(!done && "invisible")}>{text}</span>

      {!done ? (
        <span aria-hidden className="absolute inset-0">
          {text.slice(0, shown)}
          <span className="caret ml-0.5 inline-block h-[1em] w-[2px] translate-y-[0.15em] bg-function align-baseline" />
        </span>
      ) : null}
    </span>
  );
}
