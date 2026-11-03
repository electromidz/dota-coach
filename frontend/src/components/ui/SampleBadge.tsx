import { InfoTip } from "@/components/ui/InfoTip";
import type { Confidence } from "@/lib/types";

/**
 * A small warning icon when a figure rests on few matches; nothing when the
 * sample is solid. The count and its meaning live in the tooltip.
 */
export function SampleBadge({
  sample,
  confidence,
  align = "end",
}: {
  sample: number;
  confidence: Confidence;
  align?: "start" | "end";
}) {
  if (confidence === "adequate") return null;
  const thin = confidence === "insufficient";
  return (
    <InfoTip
      label={`${thin ? "Too few matches" : "Small sample"}: ${sample} ${sample === 1 ? "match" : "matches"}`}
      icon="alert"
      iconClassName={thin ? "text-ink-faint" : "text-warn"}
      align={align}
    >
      <span>
        {thin ? "Too few matches to generalize" : "Small sample"} · {sample}{" "}
        {sample === 1 ? "match" : "matches"}
      </span>
    </InfoTip>
  );
}
