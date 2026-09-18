"use client";

import { useEffect, useState } from "react";

import { useInView } from "@/lib/useInView";
import { cn } from "@/lib/utils";

const DURATION_MS = 1400;
/** Cubic ease-out: fast start, settles into the final value. */
const ease = (t: number) => 1 - Math.pow(1 - t, 3);

/**
 * Counts up from 0 to `value` once, the moment it scrolls into view.
 *
 * `prefers-reduced-motion` skips the tween and shows the final number
 * immediately — a counting animation is exactly the kind of motion that
 * setting exists to suppress.
 */
export function Counter({
  value,
  format = (n) => Math.round(n).toLocaleString("en-US"),
  className,
}: {
  value: number;
  format?: (n: number) => string;
  className?: string;
}) {
  const { ref, inView } = useInView<HTMLSpanElement>();
  const [shown, setShown] = useState(0);

  useEffect(() => {
    if (!inView) return;

    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setShown(value);
      return;
    }

    let frame = 0;
    let start: number | null = null;

    const tick = (now: number) => {
      if (start === null) start = now;
      const t = Math.min(1, (now - start) / DURATION_MS);
      setShown(value * ease(t));
      if (t < 1) frame = requestAnimationFrame(tick);
    };

    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [inView, value]);

  return (
    <span ref={ref} className={cn("tabular-nums", className)}>
      {format(shown)}
    </span>
  );
}
