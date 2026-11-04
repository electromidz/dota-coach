/** Joins conditional class names, dropping falsy entries. */
export function cn(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(" ");
}

/**
 * A percentile or rank as an ordinal: `72` -> `"72nd"`, `11` -> `"11th"`.
 *
 * Rounds first, so `71.6` reads `"72nd"`. The teens take `th` whatever their
 * last digit.
 */
export function ordinal(value: number): string {
  const n = Math.max(0, Math.round(value));
  const lastTwo = n % 100;
  if (lastTwo >= 11 && lastTwo <= 13) return `${n}th`;
  switch (n % 10) {
    case 1:
      return `${n}st`;
    case 2:
      return `${n}nd`;
    case 3:
      return `${n}rd`;
    default:
      return `${n}th`;
  }
}

/**
 * Corrects ordinals inside stored text, e.g. "the 72th percentile" ->
 * "the 72nd percentile". Analyses written before the backend fix kept the
 * wrong suffixes in their saved evidence; this keeps them readable.
 */
export function fixOrdinals(text: string): string {
  return text.replace(/\b(\d+)(?:st|nd|rd|th)\b/g, (_, n: string) =>
    ordinal(Number(n)),
  );
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

  const diff = Math.floor((now.getTime() - then.getTime()) / 1000);
  const future = diff < 0;
  const seconds = Math.abs(diff);
  if (seconds < 60) return future ? "in a moment" : "just now";

  const say = (n: number, unit: string) => (future ? `in ${n}${unit}` : `${n}${unit} ago`);
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return say(minutes, "m");
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return say(hours, "h");
  const days = Math.floor(hours / 24);
  if (days < 14) return say(days, "d");
  const weeks = Math.floor(days / 7);
  if (days < 60) return say(weeks, "w");
  const months = Math.floor(days / 30);
  if (days < 365) return say(months, "mo");
  return say(Math.floor(days / 365), "y");
}

/** "Sep 28, 2026 · 17:53" in the viewer's timezone, no seconds. */
export function formatMatchDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const day = date.toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
  const time = date.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
  return `${day} · ${time}`;
}

/** The full date and time, for the tooltip behind a relative time. */
export function fullDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
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

/** The population behind a section's numbers. */
export interface Scope {
  /** A hero name, or omitted for every hero. */
  hero?: string | null;
  /** A position label, or omitted / null for every role. */
  role?: string | null;
  /** Defaults to the population every page reads. */
  mode?: string;
  matches: number;
}

/** `Mirana · Soft Support · Ranked + All Pick · 5 matches`. */
export function scopeText({ hero, role, mode = "Ranked + All Pick", matches }: Scope): string {
  return [
    hero || "All heroes",
    role || "All roles",
    mode,
    `${matches.toLocaleString("en-US")} ${matches === 1 ? "match" : "matches"}`,
  ].join(" · ");
}
