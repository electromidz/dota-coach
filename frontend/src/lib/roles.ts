import type { ScoreComponent } from "./types";

/**
 * Presentation helpers for the role performance score.
 *
 * As with `stats.ts`, there is deliberately **no arithmetic over match data**
 * here. The score, its components and their weights all arrive from the
 * backend, already normalised and already weighted; what remains is writing
 * each measure in the units a player reads it in.
 */

/** A component's measured value, in its own units. */
export function formatComponentValue(component: ScoreComponent): string {
  switch (component.key) {
    case "win_rate":
    case "kill_participation":
      return `${Math.round(component.value * 100)}%`;
    case "kda":
      return component.value.toFixed(2);
    case "deaths_per_10":
      return component.value.toFixed(1);
  }
}

/** `0.45` -> `"45%"`, for the share of the score a measure carries. */
export function formatWeight(weight: number): string {
  return `${Math.round(weight * 100)}%`;
}

/**
 * How many more eligible games a role needs before it can be recommended.
 *
 * Zero once the floor is cleared, so a caller can treat it as "nothing to say".
 */
export function gamesUntilRecommendable(
  matches: number,
  minimum: number,
): number {
  return Math.max(0, minimum - matches);
}
