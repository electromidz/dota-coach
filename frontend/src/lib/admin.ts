import type { AdminUserSummary, SubscriptionStatus, Voucher } from "./types";

/**
 * Presentation helpers for the admin panel.
 *
 * As everywhere else in `lib`, nothing here decides anything or computes a
 * count — every number on the admin screens comes from the backend. What
 * remains is turning a stored slug into a sentence a human reads.
 */

const PLAN_LABELS: Record<SubscriptionStatus, string> = {
  trialing: "Trial",
  active: "Active",
  expired: "Expired",
  cancelled: "Cancelled",
  past_due: "Past due",
};

/** The account's billing lifecycle, in words. `null` means the account has
 *  never had a subscription row materialised — practically, never logged in
 *  since trials started at signup. */
export function planLabel(status: SubscriptionStatus | null): string {
  return status === null ? "No subscription" : PLAN_LABELS[status];
}

export function accountStatusLabel(status: AdminUserSummary["status"]): string {
  return status === "disabled" ? "Disabled" : "Active";
}

/** `42.857` -> `"43%"`. Unlike `formatPercent` in `lib/stats`, the input here
 *  is already a 0–100 figure, not a 0–1 fraction — the two must never be fed
 *  each other's output. */
export function formatPercentPoints(value: number | null): string {
  return value === null ? "—" : `${Math.round(value)}%`;
}

/** A short, human name for an account — what an admin recognises them by. */
export function displayName(user: Pick<AdminUserSummary, "persona_name" | "steam_id">): string {
  return user.persona_name ?? user.steam_id;
}

export type VoucherState = "active" | "deactivated" | "expired" | "used_up";

/** One of four states, in priority order: an admin turning a code off wins
 *  over it merely running out, and running out of *uses* is checked before
 *  running out of *time* — both are just as final, but a use-limit is the
 *  more common reason a code stops working, so it reads first. */
export function voucherState(voucher: Voucher): VoucherState {
  if (!voucher.active) return "deactivated";
  if (voucher.used_count >= voucher.max_uses) return "used_up";
  if (voucher.expires_at && new Date(voucher.expires_at).getTime() <= Date.now()) {
    return "expired";
  }
  return "active";
}

const VOUCHER_STATE_LABELS: Record<VoucherState, string> = {
  active: "Active",
  deactivated: "Deactivated",
  expired: "Expired",
  used_up: "Used up",
};

export function voucherStateLabel(voucher: Voucher): string {
  return VOUCHER_STATE_LABELS[voucherState(voucher)];
}

/** Plain CSV, not a library: eight columns and no value here can contain a
 *  newline, so the only escaping that matters is a literal `"` or `,`. */
export function vouchersToCsv(vouchers: Voucher[]): string {
  const header = [
    "code",
    "duration_days",
    "max_uses",
    "used_count",
    "expires_at",
    "active",
    "note",
    "created_at",
  ];
  const escape = (value: string) => `"${value.replace(/"/g, '""')}"`;

  const rows = vouchers.map((v) =>
    [
      v.code,
      String(v.duration_days),
      String(v.max_uses),
      String(v.used_count),
      v.expires_at ?? "",
      String(v.active),
      v.note ?? "",
      v.created_at,
    ]
      .map(escape)
      .join(","),
  );

  return [header.map(escape).join(","), ...rows].join("\n");
}
