import type { Confidence } from "./types";

/**
 * How a benchmark `Confidence` reads to someone who has not read the spec.
 *
 * One copy, because the benchmark page and the training focus are describing
 * the *same* backend judgement about the *same* sample. Two copies would drift,
 * and the failure mode of drift here is a screen implying more certainty than
 * the screen next to it — which is the one thing the statistical model is built
 * to prevent.
 *
 * No thresholds are restated. The backend owns them; these sentences only say
 * what its verdict means.
 */
export const CONFIDENCE_NOTE: Record<Confidence, string> = {
  insufficient:
    "Too few matches on this hero to place you in the distribution. Percentiles appear once you have five.",
  low: "Based on a small number of matches, so treat the percentiles as indicative.",
  adequate: "",
};

/** The one-word badge: `low` -> `"Low confidence"`. */
export const CONFIDENCE_LABEL: Record<Confidence, string> = {
  insufficient: "Not enough data",
  low: "Low confidence",
  adequate: "Measured",
};
