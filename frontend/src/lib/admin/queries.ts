/**
 * Every admin API call, typed, in one module.
 *
 * Screens import from here only; nothing in `components/admin` builds a URL
 * or parses a response on its own. Access is enforced by the backend
 * (`AdminUser` on every `/api/admin/*` route) — this module adds no gate of
 * its own and trusts no client-side flag.
 */

import { apiFetch } from "../api";
import type {
  AdminStats,
  AdminUser,
  AdminUserDetail,
  AdminUserList,
  CreateVoucherParams,
  UserListParams,
  Voucher,
  VoucherDetail,
  VoucherList,
  VoucherListItem,
  VoucherStatus,
} from "./types";

/** The backend's page-size ceiling. */
export const MAX_PAGE_SIZE = 100;

function query(params: Record<string, string | number | undefined | null>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== "") search.set(key, String(value));
  }
  const qs = search.toString();
  return qs ? `?${qs}` : "";
}

const post = <T>(path: string, body?: unknown) =>
  apiFetch<T>(path, {
    method: "POST",
    body: body === undefined ? undefined : JSON.stringify(body),
  });

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

/** `from`/`to` are RFC3339; the backend defaults to the last 30 days. */
export function getStats(range: { from?: string; to?: string } = {}): Promise<AdminStats> {
  return apiFetch<AdminStats>(`/api/admin/stats${query(range)}`);
}

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

export function listUsers(params: UserListParams = {}): Promise<AdminUserList> {
  return apiFetch<AdminUserList>(`/api/admin/users${query({ ...params })}`);
}

export function getUser(id: string): Promise<AdminUserDetail> {
  return apiFetch<AdminUserDetail>(`/api/admin/users/${id}`);
}

/** Adds `days` to whichever window governs access, reviving a lapsed one. */
export function extendAccess(id: string, days: number, reason?: string): Promise<AdminUser> {
  return post<AdminUser>(`/api/admin/users/${id}/extend`, { days, reason: reason || undefined });
}

/** Refused on its very next request; no session revocation needed. */
export function disableUser(id: string, reason?: string): Promise<AdminUser> {
  return post<AdminUser>(`/api/admin/users/${id}/disable`, { reason: reason || undefined });
}

export function enableUser(id: string, reason?: string): Promise<AdminUser> {
  return post<AdminUser>(`/api/admin/users/${id}/enable`, { reason: reason || undefined });
}

// ---------------------------------------------------------------------------
// Vouchers
// ---------------------------------------------------------------------------

export function listVouchers(
  params: { page?: number; limit?: number; status?: VoucherStatus | "" } = {},
): Promise<VoucherList> {
  return apiFetch<VoucherList>(`/api/admin/vouchers${query(params)}`);
}

/**
 * Every voucher matching `status`, across all pages — for the CSV export,
 * which must hold the whole result, not the page on screen.
 */
export async function listAllVouchers(status: VoucherStatus | "" = ""): Promise<VoucherListItem[]> {
  const all: VoucherListItem[] = [];
  for (let page = 1; ; page += 1) {
    const response = await listVouchers({ page, limit: MAX_PAGE_SIZE, status });
    all.push(...response.vouchers);
    if (page >= response.total_pages || response.vouchers.length === 0) return all;
  }
}

export function getVoucher(id: string): Promise<VoucherDetail> {
  return apiFetch<VoucherDetail>(`/api/admin/vouchers/${id}`);
}

/** Always a list, even for one voucher. */
export async function createVouchers(params: CreateVoucherParams): Promise<Voucher[]> {
  const response = await post<{ vouchers: Voucher[] }>("/api/admin/vouchers", params);
  return response.vouchers;
}

/** Existing redemptions are untouched; only future ones are refused. */
export function deactivateVoucher(id: string): Promise<Voucher> {
  return post<Voucher>(`/api/admin/vouchers/${id}/deactivate`);
}
