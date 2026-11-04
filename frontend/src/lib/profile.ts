/**
 * Display helpers for the Profile page.
 *
 * As everywhere in `lib`, nothing here computes a statistic: win rates, day
 * counts and match totals arrive from the backend and are only formatted,
 * ordered or picked between.
 */

import {
  getBilling,
  getMatches,
  getMe,
  getRoleSelection,
  getStats,
} from "./api";
import type { BillingResponse, HeroStats, MatchView } from "./types";

/** OpenDota and Dotabuff both key players on the 32-bit Dota account id. */
export function openDotaUrl(accountId: number): string {
  return `https://www.opendota.com/players/${accountId}`;
}

export function dotabuffUrl(accountId: number): string {
  return `https://www.dotabuff.com/players/${accountId}`;
}

/** Most-played first; win rate breaks a tie so the order is stable. */
export function topHeroes(heroes: HeroStats[], count = 3): HeroStats[] {
  return [...heroes]
    .sort((a, b) => b.matches - a.matches || b.win_rate - a.win_rate)
    .slice(0, count);
}

/** The match list is newest first; a form strip reads oldest first. */
export function oldestFirst(matches: MatchView[]): MatchView[] {
  return [...matches].reverse();
}

/** Trial access that runs out inside three days, by the server's own count. */
export function trialEndingSoon(billing: BillingResponse): boolean {
  return (
    billing.entitlement === "trial" &&
    billing.days_remaining !== null &&
    billing.days_remaining < 3
  );
}

function days(n: number | null): string {
  if (n === null || n < 1) return "less than a day";
  return n === 1 ? "1 day" : `${n} days`;
}

/**
 * One line for the subscription card.
 *
 * "Access until", not "renews": payments are one-off crypto charges, so
 * nothing renews on its own and the card must not say it will.
 */
export function subscriptionLine(billing: BillingResponse): string {
  switch (billing.entitlement) {
    case "trial":
      return `${days(billing.days_remaining)} left in your free trial`;
    case "pro":
      return billing.access_ends_at
        ? `Access until ${shortDay(billing.access_ends_at)}`
        : "Subscribed";
    default:
      return "Your trial has ended";
  }
}

/** "4 Nov 2026" — a day, not a moment; used for ranges and access dates. */
export function shortDay(iso: string): string {
  return new Date(iso).toLocaleDateString("en-GB", {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

/** "12 Mar 2026 → 4 Nov 2026", or the single day when both ends agree. */
export function dateRange(oldest: string, newest: string): string {
  const from = shortDay(oldest);
  const to = shortDay(newest);
  return from === to ? from : `${from} → ${to}`;
}

// ---------------------------------------------------------------------------
// Device preferences
// ---------------------------------------------------------------------------

/**
 * Preferences the backend has no field for yet, kept in this browser only.
 *
 * Coach language needs the coaching prompt to honour it, and there is no
 * per-account settings table to store either value in. Until there is, these
 * are per-device conveniences and the UI says so.
 */
export interface DevicePreferences {
  language: string;
  timezone: string;
}

const PREFS_KEY = "dc_profile_prefs";

export const COACH_LANGUAGES = [
  { value: "en", label: "English" },
  { value: "ru", label: "Русский" },
  { value: "es", label: "Español" },
  { value: "pt", label: "Português" },
  { value: "fa", label: "فارسی" },
] as const;

export function browserTimezone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  } catch {
    return "UTC";
  }
}

/** Every IANA zone the browser knows, or a short fallback list. */
export function timezones(current: string): string[] {
  let zones: string[] = [];
  try {
    zones = Intl.supportedValuesOf("timeZone");
  } catch {
    zones = ["UTC", "Europe/London", "Europe/Berlin", "Europe/Moscow", "America/New_York", "Asia/Tehran"];
  }
  return zones.includes(current) ? zones : [current, ...zones];
}

export function readDevicePreferences(): DevicePreferences {
  const fallback = { language: "en", timezone: browserTimezone() };
  try {
    const raw = window.localStorage.getItem(PREFS_KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as Partial<DevicePreferences>;
    return {
      language: typeof parsed.language === "string" ? parsed.language : fallback.language,
      timezone: typeof parsed.timezone === "string" ? parsed.timezone : fallback.timezone,
    };
  } catch {
    return fallback;
  }
}

const PREFS_EVENT = "dc-profile-prefs";

/**
 * The last value written, kept here when storage refuses it so the select
 * still shows the choice for the rest of the visit.
 */
let unsaved: DevicePreferences | null = null;
let cached: { raw: string | null; value: DevicePreferences } | null = null;

/** False when storage is unavailable (private mode, blocked site data). */
export function writeDevicePreferences(prefs: DevicePreferences): boolean {
  let saved = true;
  try {
    window.localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
    unsaved = null;
  } catch {
    unsaved = prefs;
    saved = false;
  }
  window.dispatchEvent(new Event(PREFS_EVENT));
  return saved;
}

/** For `useSyncExternalStore`: this tab's writes and other tabs' alike. */
export function subscribeDevicePreferences(onChange: () => void): () => void {
  window.addEventListener(PREFS_EVENT, onChange);
  window.addEventListener("storage", onChange);
  return () => {
    window.removeEventListener(PREFS_EVENT, onChange);
    window.removeEventListener("storage", onChange);
  };
}

/** A stable object for as long as the stored value has not changed. */
export function getDevicePreferencesSnapshot(): DevicePreferences {
  if (unsaved) return unsaved;
  let raw: string | null = null;
  try {
    raw = window.localStorage.getItem(PREFS_KEY);
  } catch {
    raw = null;
  }
  if (!cached || cached.raw !== raw) {
    cached = { raw, value: readDevicePreferences() };
  }
  return cached.value;
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/** Page size for the export; the backend's maximum. */
const EXPORT_PAGE = 100;

/**
 * Everything the account can already read about itself, in one JSON file.
 *
 * Built from the same endpoints the app uses, so it can hold nothing a
 * player could not see on screen. Billing and role selection are optional:
 * a failure there must not cost the player their match history.
 */
export async function buildExport(): Promise<Record<string, unknown>> {
  const [me, stats] = await Promise.all([getMe(), getStats()]);

  const matches: MatchView[] = [];
  for (let page = 1; ; page += 1) {
    const response = await getMatches(page, EXPORT_PAGE, { mode: "all", sort: "oldest" });
    matches.push(...response.matches);
    if (page >= response.total_pages || response.matches.length === 0) break;
  }

  const [billing, roles] = await Promise.all([
    getBilling().catch(() => null),
    getRoleSelection().catch(() => null),
  ]);

  return {
    exported_at: new Date().toISOString(),
    user: me.user,
    dota_player: me.dota_player,
    stats,
    coaching_profile: roles?.profile ?? null,
    subscription: billing
      ? { entitlement: billing.entitlement, subscription: billing.subscription, payments: billing.payments }
      : null,
    matches,
  };
}

/** Hand the browser a file to save. */
export function downloadJson(data: unknown, filename: string) {
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}
