"use client";

import { useEffect, useRef, useState } from "react";

/**
 * The rendered width of an element, kept current with ResizeObserver, so a
 * chart can lay itself out in real pixels instead of stretching a viewBox.
 * Falls back to `initial` where ResizeObserver does not exist (tests, SSR).
 */
export function useWidth<T extends HTMLElement>(initial = 640) {
  const ref = useRef<T | null>(null);
  const [width, setWidth] = useState(initial);

  useEffect(() => {
    const element = ref.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      const next = Math.round(entry.contentRect.width);
      if (next > 0) setWidth(next);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  return [ref, width] as const;
}
