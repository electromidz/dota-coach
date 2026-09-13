/** Joins conditional class names, dropping falsy entries. */
export function cn(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(" ");
}

/** `2400` -> `"40:00"`. */
export function formatDuration(seconds: number): string {
  const safe = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(safe / 60);
  const rest = safe % 60;
  return `${minutes}:${rest.toString().padStart(2, "0")}`;
}

/** `(kills + assists) / max(deaths, 1)`, to one decimal. */
export function kda(kills: number, deaths: number, assists: number): string {
  return ((kills + assists) / Math.max(deaths, 1)).toFixed(1);
}

/** Short relative time. Falls back to the date once it stops being useful. */
export function timeAgo(iso: string, now: Date = new Date()): string {
  const then = new Date(iso);
  if (Number.isNaN(then.getTime())) return "unknown";

  const seconds = Math.floor((now.getTime() - then.getTime()) / 1000);
  if (seconds < 60) return "just now";

  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;

  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;

  return then.toLocaleDateString();
}

/**
 * OpenDota packs the medal into `rank_tier`: tens digit is the medal, ones
 * digit the star. `80` is Immortal, which has no stars.
 */
const MEDALS = [
  "Uncalibrated",
  "Herald",
  "Guardian",
  "Crusader",
  "Archon",
  "Legend",
  "Ancient",
  "Divine",
  "Immortal",
];

export function formatRank(rankTier: number | null): string | null {
  if (rankTier === null || rankTier <= 0) return null;

  const medal = MEDALS[Math.floor(rankTier / 10)];
  if (!medal) return null;

  const star = rankTier % 10;
  if (medal === "Immortal" || star === 0) return medal;

  return `${medal} ${star}`;
}
