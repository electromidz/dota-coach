"use client";

import { useEffect, useRef, useState } from "react";

/**
 * True once the element has scrolled into view, and stays true afterward —
 * a scroll entrance should never replay when a user scrolls back past it.
 *
 * `rootMargin` fires the reveal a little before the element's edge actually
 * crosses the viewport, so content is settled by the time a reader's eye
 * reaches it rather than animating under their gaze.
 */
export function useInView<T extends HTMLElement>(rootMargin = "0px 0px -10% 0px") {
  const ref = useRef<T | null>(null);
  const [inView, setInView] = useState(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setInView(true);
          observer.disconnect();
        }
      },
      { rootMargin, threshold: 0.1 },
    );

    observer.observe(el);
    return () => observer.disconnect();
  }, [rootMargin]);

  return { ref, inView };
}
