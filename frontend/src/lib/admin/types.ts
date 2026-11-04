/**
 * Every shape the admin API answers with, in one place.
 *
 * Mirrors `backend/src/domain/admin.rs` and `domain/audit.rs`. Nothing here is
 * computed on the client: counts, deltas' inputs and funnel rates all arrive
 * from the backend.
 */

import type { SubscriptionStatus } from "../types";

export type AccountStatus = "active" | "disabled";

/** Where an account stands with billing, as the admin list shows it. */
export interface AdminSubscription {
  /** `null` for an account that has never had a subscription row. */
  status: SubscriptionStatus | null;
  plan: string | null;
  trialEndsAt: string | null;
  currentPeriodEnd: string | null;
}

/** One row of the admin user list, and the base of the detail view. */
export interface AdminUser {
  id: string;
  /** A string: SteamID64 exceeds JavaScript's safe integer range. */
  steam_id: string;
  persona_name: string | null;
  avatar_url: string | null;
  status: AccountStatus;
  is_admin: boolean;
  created_at: string;
  last_login_at: string | null;
  subscription_status: SubscriptionStatus | null;
  subscription_plan: string | null;
  trial_ends_at: string | null;
  current_period_end: string | null;
}

/** Something the account did, from the `events` table. `type` is open: a
 *  type this build doesn't know still renders. */
export interface AdminEventRecord {
  id: string;
  type: string;
  metadata: Record<string, unknown>;
  created_at: string;
}

/** Something an admin did, from the audit log. */
export interface AuditLogEntry {
  id: string;
  /** `null` once the admin's own account is gone. */
  admin_id: string | null;
  admin_persona_name: string | null;
  action: string;
  target_type: string;
  target_id: string;
  metadata: Record<string, unknown>;
  created_at: string;
}

/** One line of an account's activity feed: either kind, normalised. */
export interface ActivityEvent {
  id: string;
  kind: "event" | "admin";
  /** The raw slug: `trial_expired`, `disable_user`, … */
  type: string;
  at: string;
  /** For admin actions: who took it. */
  actor: string | null;
  reason: string | null;
  metadata: Record<string, unknown>;
}

export interface AdminUserDetail extends AdminUser {
  /** Newest first. */
  events: AdminEventRecord[];
  /** Newest first. */
  admin_actions: AuditLogEntry[];
}

export interface Paged {
  page: number;
  limit: number;
  total: number;
  total_pages: number;
}

export interface AdminUserList extends Paged {
  users: AdminUser[];
}

export type UserSortKey =
  | "name"
  | "steam_id"
  | "status"
  | "subscription"
  | "plan"
  | "joined"
  | "last_seen";

export type SortDirection = "asc" | "desc";

export interface UserListParams {
  page?: number;
  limit?: number;
  status?: AccountStatus | "";
  plan?: SubscriptionStatus | "";
  search?: string;
  sort?: UserSortKey;
  dir?: SortDirection;
}

export interface DailyStat {
  date: string;
  signups: number;
  /** Distinct accounts. */
  logins: number;
  /** Raw purchase events. */
  purchases: number;
}

/** Window-scoped counts; the same shape for the window and the one before. */
export interface PeriodCounts {
  signups: number;
  active_users: number;
  trials_started: number;
  trials_expired: number;
  purchases: number;
  voucher_redemptions: number;
  revenue_cents: number;
}

/** All-time signup → trial → paid. A rate is `null` when its denominator is 0. */
export interface Funnel {
  signed_up: number;
  trial_started: number;
  purchased: number;
  signup_to_trial_pct: number | null;
  trial_to_paid_pct: number | null;
  signup_to_paid_pct: number | null;
}

export interface AdminStats {
  from: string;
  to: string;
  total_users: number;
  dau: number;
  wau: number;
  mau: number;
  active_trials: number;
  trials_expired: number;
  trials_started: number;
  paid_users: number;
  currently_paid: number;
  /** Of trials started in the window, the share that has bought since. */
  trial_to_paid_conversion_pct: number | null;
  voucher_redemptions: number;
  revenue_cents: number;
  currency: string;
  /** Oldest first, no gaps. */
  daily: DailyStat[];
  period: PeriodCounts;
  previous: PeriodCounts;
  previous_from: string;
  previous_to: string;
  funnel: Funnel;
}

export interface Voucher {
  id: string;
  code: string;
  duration_days: number;
  max_uses: number;
  used_count: number;
  expires_at: string | null;
  active: boolean;
  /** The admin's own label. Never shown to the redeeming user. */
  note: string | null;
  created_by: string | null;
  created_at: string;
}

export interface VoucherRedemption {
  id: string;
  user_id: string;
  steam_id: string;
  persona_name: string | null;
  redeemed_at: string;
}

/** A row of the voucher list: the voucher and who redeemed it. */
export interface VoucherListItem extends Voucher {
  redeemed_by: VoucherRedemption | null;
}

export interface VoucherDetail extends Voucher {
  /** Newest first. */
  redemptions: VoucherRedemption[];
}

export type VoucherStatus = "active" | "used" | "expired" | "deactivated";

export interface VoucherList extends Paged {
  vouchers: VoucherListItem[];
}

export interface CreateVoucherParams {
  duration_days: number;
  expires_at?: string;
  note?: string;
  /** How many independent codes; defaults to 1 on the backend. */
  count?: number;
}
