/**
 * Turning admin data into words.
 *
 * As everywhere in `lib`, nothing here decides access or invents a count: it
 * names slugs, formats numbers the backend computed, and groups rows for
 * display.
 */

import type { IconName } from "@/components/ui/Icon";

import type { SubscriptionStatus } from "../types";
import type {
  AccountStatus,
  ActivityEvent,
  AdminUser,
  AdminUserDetail,
  Voucher,
  VoucherListItem,
  VoucherStatus,
} from "./types";

// ---------------------------------------------------------------------------
// Accounts and subscriptions
// ---------------------------------------------------------------------------

const SUBSCRIPTION_LABELS: Record<SubscriptionStatus, string> = {
  trialing: "Trial",
  active: "Paid",
  expired: "Expired",
  cancelled: "Cancelled",
  past_due: "Past due",
};

/**
 * The billing lifecycle in words. "Paid" rather than "Active" so it can never
 * be confused with the *account* being active. `null` means no subscription
 * row was ever created — in practice, never logged in.
 */
export function subscriptionLabel(status: SubscriptionStatus | null): string {
  return status === null ? "None" : SUBSCRIPTION_LABELS[status];
}

export function accountStatusLabel(status: AccountStatus): string {
  return status === "disabled" ? "Disabled" : "Active";
}

/** A plan slug ("pro") as a name ("Pro"). */
export function planName(plan: string | null): string {
  if (!plan) return "—";
  return plan.charAt(0).toUpperCase() + plan.slice(1);
}

/** What an admin recognises an account by. */
export function displayName(user: Pick<AdminUser, "persona_name" | "steam_id">): string {
  return user.persona_name ?? user.steam_id;
}

// ---------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------

/** `42.857` → `"43%"`. Input is already 0–100, never a 0–1 fraction. A missing
 *  rate is "n/a": there was nothing to divide by, which is not 0%. */
export function formatPercentPoints(value: number | null): string {
  if (value === null) return "n/a";
  if (value > 0 && value < 1) return "<1%";
  return `${Math.round(value)}%`;
}

export function formatCount(value: number): string {
  return Math.round(value).toLocaleString("en-US");
}

export interface Delta {
  /** Signed difference, current − previous. */
  change: number;
  /** Signed percentage change, or `null` when the previous value was 0. */
  pct: number | null;
  direction: "up" | "down" | "flat";
}

/** Compares two backend counts. No rounding: the caller formats. */
export function delta(current: number, previous: number): Delta {
  const change = current - previous;
  return {
    change,
    pct: previous === 0 ? null : (change / previous) * 100,
    direction: change > 0 ? "up" : change < 0 ? "down" : "flat",
  };
}

/** "+12% vs previous", "+3 vs previous" (no baseline), "No change". */
export function deltaLabel(d: Delta, format: (n: number) => string = formatCount): string {
  if (d.direction === "flat") return "No change";
  const sign = d.change > 0 ? "+" : "−";
  if (d.pct === null) return `${sign}${format(Math.abs(d.change))} vs previous`;
  return `${sign}${Math.round(Math.abs(d.pct))}% vs previous`;
}

// ---------------------------------------------------------------------------
// Durations and dates
// ---------------------------------------------------------------------------

/** `1` → "1 day", `30` → "30 days", `365` → "1 year". */
export function durationLabel(days: number): string {
  if (days % 365 === 0) {
    const years = days / 365;
    return years === 1 ? "1 year" : `${years} years`;
  }
  return days === 1 ? "1 day" : `${days} days`;
}

/** "4 Nov 2026". */
export function shortDate(iso: string): string {
  return new Date(iso).toLocaleDateString("en-GB", {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

/** "4 Nov 2026, 14:05:09" — the exact moment, for a hover title. */
export function exactTime(iso: string): string {
  return new Date(iso).toLocaleString("en-GB", {
    day: "numeric",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

/** "14:05". */
export function clockTime(iso: string): string {
  return new Date(iso).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
}

/** "Today", "Yesterday", or "Mon 2 Nov 2026" — a day heading. */
export function dayHeading(iso: string, now: Date = new Date()): string {
  const day = new Date(iso);
  const key = (d: Date) => d.toDateString();
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);

  if (key(day) === key(now)) return "Today";
  if (key(day) === key(yesterday)) return "Yesterday";
  return day.toLocaleDateString("en-GB", {
    weekday: "short",
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

// ---------------------------------------------------------------------------
// Activity
// ---------------------------------------------------------------------------

const ACTIVITY: Record<string, { label: string; icon: IconName; tone: Tone }> = {
  login: { label: "Signed in", icon: "user", tone: "neutral" },
  logout: { label: "Signed out", icon: "logout", tone: "neutral" },
  trial_started: { label: "Trial started", icon: "clock", tone: "info" },
  trial_expired: { label: "Trial expired", icon: "alert", tone: "warn" },
  subscription_expired: { label: "Subscription expired", icon: "alert", tone: "warn" },
  purchase: { label: "Purchased a subscription", icon: "coins", tone: "good" },
  voucher_redeemed: { label: "Redeemed a voucher", icon: "ticket", tone: "good" },
  feature_used: { label: "Used a feature", icon: "spark", tone: "neutral" },
  page_view: { label: "Viewed a page", icon: "home", tone: "neutral" },
  // Admin actions, from the audit log.
  extend_access: { label: "Access extended by an admin", icon: "plus", tone: "info" },
  disable_user: { label: "Account disabled by an admin", icon: "alert", tone: "bad" },
  enable_user: { label: "Account re-enabled by an admin", icon: "check", tone: "good" },
};

/** `trial_expired` → "Trial expired"; an unknown slug becomes readable text. */
export function activityLabel(type: string): string {
  return ACTIVITY[type]?.label ?? humanize(type);
}

export function activityIcon(type: string): IconName {
  return ACTIVITY[type]?.icon ?? "info";
}

export function activityTone(type: string): Tone {
  return ACTIVITY[type]?.tone ?? "neutral";
}

function humanize(slug: string): string {
  const text = slug.replace(/_/g, " ").trim();
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** The one detail worth a second line, when the metadata carries one. */
export function activityDetail(event: ActivityEvent): string | null {
  const m = event.metadata;
  if (event.type === "extend_access" && typeof m.days === "number") {
    return `+${durationLabel(m.days)}`;
  }
  if (event.type === "voucher_redeemed" && typeof m.code === "string") {
    return typeof m.duration_days === "number"
      ? `${m.code} · ${durationLabel(m.duration_days)}`
      : m.code;
  }
  if (event.type === "feature_used" && typeof m.scope === "string") return humanize(m.scope);
  if (event.type === "page_view" && typeof m.path === "string") return m.path;
  return null;
}

/** The account's own events and the admin actions on it, as one feed, newest first. */
export function activityFeed(detail: Pick<AdminUserDetail, "events" | "admin_actions">): ActivityEvent[] {
  const events: ActivityEvent[] = detail.events.map((e) => ({
    id: `e-${e.id}`,
    kind: "event",
    type: e.type,
    at: e.created_at,
    actor: null,
    reason: null,
    metadata: e.metadata,
  }));
  const actions: ActivityEvent[] = detail.admin_actions.map((a) => ({
    id: `a-${a.id}`,
    kind: "admin",
    type: a.action,
    at: a.created_at,
    actor: a.admin_persona_name ?? (a.admin_id ? "An admin" : "A former admin"),
    reason: typeof a.metadata.reason === "string" ? a.metadata.reason : null,
    metadata: a.metadata,
  }));

  return [...events, ...actions].sort((x, y) => y.at.localeCompare(x.at));
}

/** Consecutive rows that share a calendar day, in feed order. */
export function groupByDay<T extends { at: string }>(rows: T[]): { day: string; rows: T[] }[] {
  const groups: { day: string; key: string; rows: T[] }[] = [];
  for (const row of rows) {
    const key = new Date(row.at).toDateString();
    const last = groups[groups.length - 1];
    if (last && last.key === key) last.rows.push(row);
    else groups.push({ day: dayHeading(row.at), key, rows: [row] });
  }
  return groups.map(({ day, rows }) => ({ day, rows }));
}

// ---------------------------------------------------------------------------
// Vouchers
// ---------------------------------------------------------------------------

/**
 * One of four states, in the same priority the backend filter uses:
 * deactivated, then used up, then expired, then active.
 */
export function voucherStatus(voucher: Voucher, now: number = Date.now()): VoucherStatus {
  if (!voucher.active) return "deactivated";
  if (voucher.used_count >= voucher.max_uses) return "used";
  if (voucher.expires_at && new Date(voucher.expires_at).getTime() <= now) return "expired";
  return "active";
}

export const VOUCHER_STATUS_LABELS: Record<VoucherStatus, string> = {
  active: "Active",
  used: "Used",
  expired: "Expired",
  deactivated: "Deactivated",
};

export function voucherStatusLabel(voucher: Voucher): string {
  return VOUCHER_STATUS_LABELS[voucherStatus(voucher)];
}

/** Only a code that could still be redeemed can be deactivated. */
export function canDeactivate(voucher: Voucher): boolean {
  return voucherStatus(voucher) === "active";
}

/** An expiry countdown only means something while the code is still usable. */
export function showsExpiry(voucher: Voucher): boolean {
  return voucher.expires_at !== null && voucherStatus(voucher) === "active";
}

/** RFC 4180-style: every field quoted, quotes doubled. */
export function vouchersToCsv(vouchers: VoucherListItem[]): string {
  const header = [
    "code",
    "status",
    "duration_days",
    "used_count",
    "max_uses",
    "expires_at",
    "redeemed_by",
    "redeemed_by_steam_id",
    "redeemed_at",
    "note",
    "created_at",
  ];
  const escape = (value: string) => `"${value.replace(/"/g, '""')}"`;

  const rows = vouchers.map((v) =>
    [
      v.code,
      voucherStatus(v),
      String(v.duration_days),
      String(v.used_count),
      String(v.max_uses),
      v.expires_at ?? "",
      v.redeemed_by?.persona_name ?? "",
      v.redeemed_by?.steam_id ?? "",
      v.redeemed_by?.redeemed_at ?? "",
      v.note ?? "",
      v.created_at,
    ]
      .map(escape)
      .join(","),
  );

  return [header.map(escape).join(","), ...rows].join("\n");
}

// ---------------------------------------------------------------------------
// Shared tone vocabulary for badges
// ---------------------------------------------------------------------------

export type Tone = "neutral" | "info" | "good" | "warn" | "bad" | "accent";

export function subscriptionTone(status: SubscriptionStatus | null): Tone {
  switch (status) {
    case "active":
      return "good";
    case "trialing":
      return "info";
    case "past_due":
      return "warn";
    case "expired":
    case "cancelled":
      return "bad";
    default:
      return "neutral";
  }
}

export function voucherTone(status: VoucherStatus): Tone {
  return status === "active" ? "good" : status === "used" ? "info" : "neutral";
}
