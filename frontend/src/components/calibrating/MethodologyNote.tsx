import type { Methodology } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * What the dashed line is, in plain words.
 *
 * Every number in this sentence comes from the response, not from a constant
 * here. That is the point: the model is server configuration, and a
 * methodology note that has drifted from the model it describes is worse than
 * no note at all — it is a specific false claim rather than a vague one.
 *
 * The comparison this product is deliberately not making: other rank trackers
 * print an exact MMR figure under every match. Valve has not published one for
 * years, so those numbers are invented. This says which of its own points are
 * invented, and by what rule.
 */
export function MethodologyNote({
  methodology,
  className,
}: {
  methodology: Methodology;
  className?: string;
}) {
  return (
    <p className={cn("text-xs leading-relaxed text-ink-faint", className)}>
      <span className="font-semibold text-ink-muted">How this is measured.</span>{" "}
      Solid points are real rank readings taken when your matches were last
      synced. Dashed points between them are{" "}
      <span className="text-ink-muted">modeled</span>, not Valve&rsquo;s numbers
      — Dota has not published a per-match MMR change in years. The model moves{" "}
      <span className="font-mono tabular-nums">
        +{formatMmr(methodology.win_base_mmr)}
      </span>{" "}
      per win and{" "}
      <span className="font-mono tabular-nums">
        &minus;{formatMmr(methodology.loss_base_mmr)}
      </span>{" "}
      per loss, adjusted for how you performed, and is anchored so each stretch
      lands on the next real reading. Rank confidence counts ranked matches at{" "}
      <span className="font-mono tabular-nums">
        {methodology.confidence_per_match_pct}%
      </span>{" "}
      each, settling at{" "}
      <span className="font-mono tabular-nums">
        {methodology.confidence_threshold_pct}%
      </span>
      .
    </p>
  );
}

/** Whole numbers stay whole: "+30", not "+30.0". */
function formatMmr(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}
