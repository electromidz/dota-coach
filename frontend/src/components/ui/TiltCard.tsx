"use client";

import { useRef, useState } from "react";

import { cn } from "@/lib/utils";

/** Maximum rotation, in degrees. Past ~8 the text starts to smear. */
const MAX_TILT = 7;

/**
 * A card that tilts in 3D toward the pointer, and tips back on touch.
 *
 * CSS 3D transforms rather than WebGL: no runtime, no canvas, no extra
 * bundle, and `prefers-reduced-motion` flattens it completely (the stylesheet
 * neutralises `.layer-3d`). Children marked `pop-3d` sit forward on the Z
 * axis, so the parallax is real geometry rather than a painted highlight.
 *
 * Transform only — never layout — so a tilting card cannot nudge its
 * neighbours.
 */
export function TiltCard({
  className,
  children,
}: {
  className?: string;
  children: React.ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [transform, setTransform] = useState<string>();

  function tiltTo(clientX: number, clientY: number) {
    const el = ref.current;
    if (!el) return;

    const box = el.getBoundingClientRect();
    // -0.5..0.5 from the card's centre.
    const px = (clientX - box.left) / box.width - 0.5;
    const py = (clientY - box.top) / box.height - 0.5;

    setTransform(
      `rotateX(${(-py * MAX_TILT * 2).toFixed(2)}deg) ` +
        `rotateY(${(px * MAX_TILT * 2).toFixed(2)}deg)`,
    );
  }

  function reset() {
    setTransform(undefined);
  }

  // `min-w-0`: as a grid item this wrapper would otherwise refuse to shrink
  // below its content's intrinsic width and push the whole grid past the
  // viewport. A perspective wrapper should never drive layout.
  return (
    <div className="scene-3d min-w-0">
      <div
        ref={ref}
        // Pointer events cover mouse, pen and touch in one path.
        onPointerMove={(e) => {
          if (e.pointerType === "mouse") tiltTo(e.clientX, e.clientY);
        }}
        onPointerDown={(e) => tiltTo(e.clientX, e.clientY)}
        onPointerUp={reset}
        onPointerLeave={reset}
        onPointerCancel={reset}
        style={{ transform }}
        className={cn("layer-3d", className)}
      >
        {children}
      </div>
    </div>
  );
}
