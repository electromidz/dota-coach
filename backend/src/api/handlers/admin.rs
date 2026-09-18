//! The admin panel's backend: how many people use the product, what happens
//! during their trial, and how many buy.
//!
//! Every route here requires `AdminUser` — `users.is_admin`, set by hand in
//! the database. There is no self-serve promotion path, deliberately: this
//! surface reaches every account's billing state and event history.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::extract::{AdminUser, AppJson, AppPath, AppQuery};
use crate::domain::admin::{AdminStats, AdminUserDetail, AdminUserSummary, AdminVoucherDetail};
use crate::domain::audit::{AuditAction, AuditLogEntry, AuditTargetType};
use crate::domain::billing::SubscriptionStatus;
use crate::domain::voucher::Voucher;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::repositories::admin::UserFilters;
use crate::repositories::voucher::NewVoucher;
use crate::services;
use crate::services::audit;
use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use utoipa::ToSchema;

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 100;
/// How many rows an account's event timeline shows. A history, not an export
/// — an admin looking for "what happened around the time they churned" needs
/// the recent shape, not every row that was ever written.
const USER_EVENT_LIMIT: i64 = 100;

#[derive(Deserialize)]
pub struct StatsQuery {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

/// `GET /api/admin/stats?from&to`
#[utoipa::path(
    get, path = "/api/admin/stats", tag = "admin",
    summary = "Usage, trial and revenue counts",
    description = "Every count is computed in SQL over users, subscriptions, payments and events. `dau`/`wau`/`mau` are fixed-width windows ending at `to`; `trials_expired`, `revenue_cents` and `daily` are scoped to `[from, to]`. Both default to the last 30 days.",
    security(("session" = [])),
    params(
        ("from" = Option<String>, Query, description = "RFC3339 timestamp. Defaults to 30 days before `to`.", example = "2026-08-01T00:00:00Z"),
        ("to" = Option<String>, Query, description = "RFC3339 timestamp. Defaults to now.", example = "2026-09-01T00:00:00Z"),
    ),
    responses(
        (status = 200, description = "Usage, trial and revenue counts", body = AdminStats),
        (status = 400, description = "`from` is after `to`", body = crate::error::ErrorBody),
        (status = 403, description = "Signed in, but not an admin", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn stats(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
    AppQuery(query): AppQuery<StatsQuery>,
) -> AppResult<Json<AdminStats>> {
    if let (Some(from), Some(to)) = (query.from, query.to) {
        if from > to {
            return Err(AppError::BadRequest("`from` must not be after `to`.".into()));
        }
    }

    let stats = services::admin::stats(&state.db, &state.config.billing.currency, query.from, query.to)
        .await?;

    Ok(Json(stats))
}

#[derive(Deserialize)]
pub struct UserListQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    /// `active` or `disabled` — the account itself, not its billing state.
    pub status: Option<String>,
    /// `trialing`, `active`, `expired`, `cancelled` or `past_due`.
    pub plan: Option<String>,
    pub last_login_before: Option<DateTime<Utc>>,
    /// Matches Steam display name (partial, case-insensitive) or SteamID64
    /// (exact) — this product has no email or username.
    pub search: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct UserListResponse {
    pub users: Vec<AdminUserSummary>,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub total_pages: i64,
}

/// `GET /api/admin/users`
#[utoipa::path(
    get, path = "/api/admin/users", tag = "admin",
    summary = "Paginated user list",
    security(("session" = [])),
    params(
        ("page" = Option<i64>, Query, description = "1-based page number. Defaults to 1.", example = 1, minimum = 1),
        ("limit" = Option<i64>, Query, description = "Rows per page. Defaults to 20.", example = 20, minimum = 1, maximum = 100),
        ("status" = Option<String>, Query, description = "Filter by account status: `active` or `disabled`.", example = "active"),
        ("plan" = Option<String>, Query, description = "Filter by billing lifecycle: `trialing`, `active`, `expired`, `cancelled` or `past_due`.", example = "trialing"),
        ("last_login_before" = Option<String>, Query, description = "RFC3339 timestamp. Only accounts not seen since before this — including ones that never logged in.", example = "2026-08-01T00:00:00Z"),
        ("search" = Option<String>, Query, description = "Matches Steam display name (partial) or SteamID64 (exact).", example = "Midz"),
    ),
    responses(
        (status = 200, description = "One page of accounts, newest first", body = UserListResponse),
        (status = 400, description = "Invalid `status`, `plan`, or pagination", body = crate::error::ErrorBody),
        (status = 403, description = "Signed in, but not an admin", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn list_users(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
    AppQuery(query): AppQuery<UserListQuery>,
) -> AppResult<Json<UserListResponse>> {
    let (page, limit) = validate_pagination(query.page, query.limit)?;
    let filters = UserFilters {
        status: validate_account_status(query.status)?,
        plan: validate_plan(query.plan)?,
        last_login_before: query.last_login_before,
        search: query.search,
    };
    let offset = (page - 1) * limit;

    let users = repositories::admin::list_users(&state.db, &filters, limit, offset).await?;
    let total = repositories::admin::count_users(&state.db, &filters).await?;

    Ok(Json(UserListResponse {
        users,
        page,
        limit,
        total,
        total_pages: total_pages(total, limit),
    }))
}

/// `GET /api/admin/users/:id`
#[utoipa::path(
    get, path = "/api/admin/users/{id}", tag = "admin",
    summary = "One account's profile, subscription and recent activity",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path, description = "The account id — `users.id`, not a Dota player id.",
            example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    responses(
        (status = 200, description = "Profile, subscription state and event timeline", body = AdminUserDetail),
        (status = 404, description = "No such account", body = crate::error::ErrorBody),
        (status = 403, description = "Signed in, but not an admin", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn get_user(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<AdminUserDetail>> {
    let summary = repositories::admin::find_user_summary(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("No such account.".into()))?;
    let events = repositories::event::list_for_user(&state.db, id, USER_EVENT_LIMIT).await?;

    Ok(Json(AdminUserDetail { summary, events }))
}

#[derive(Deserialize, ToSchema)]
pub struct ExtendAccessRequest {
    /// Must be positive. Capped generously — this grants access, and a typo
    /// here should cost a support ticket, not a year of free service.
    pub days: i64,
}

const MAX_EXTENSION_DAYS: i64 = 365;

/// `POST /api/admin/users/:id/extend`
#[utoipa::path(
    post, path = "/api/admin/users/{id}/extend", tag = "admin",
    summary = "Grant more access",
    description = "Adds `days` to whichever window currently governs the account's access — the trial end for a trialing or expired account, the paid-through date for an active or past-due one. An already-expired row is pulled back to `trialing` in the same write.",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path, description = "The account id.", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    request_body = ExtendAccessRequest,
    responses(
        (status = 200, description = "The updated account", body = AdminUserSummary),
        (status = 400, description = "`days` is not positive", body = crate::error::ErrorBody),
        (status = 404, description = "No such account, or it has never had a subscription row", body = crate::error::ErrorBody),
        (status = 403, description = "Signed in, but not an admin", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn extend(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    AppPath(id): AppPath<Uuid>,
    AppJson(body): AppJson<ExtendAccessRequest>,
) -> AppResult<Json<AdminUserSummary>> {
    if !(1..=MAX_EXTENSION_DAYS).contains(&body.days) {
        return Err(AppError::BadRequest(format!(
            "days must be between 1 and {MAX_EXTENSION_DAYS}."
        )));
    }

    let extended = repositories::admin::extend_access(&state.db, id, body.days).await?;
    if !extended {
        return Err(AppError::NotFound(
            "No such account, or it has never had a subscription.".into(),
        ));
    }

    audit::record(
        &state.db,
        admin.id,
        AuditAction::ExtendAccess,
        AuditTargetType::User,
        id,
        serde_json::json!({ "days": body.days }),
    )
    .await;

    let summary = repositories::admin::find_user_summary(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("No such account.".into()))?;

    Ok(Json(summary))
}

/// `POST /api/admin/users/:id/disable`
#[utoipa::path(
    post, path = "/api/admin/users/{id}/disable", tag = "admin",
    summary = "Disable an account",
    description = "Every future request from this account is refused with 403 ACCOUNT_DISABLED, checked fresh on each request — no need to also revoke sessions.",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path, description = "The account id.", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    responses(
        (status = 200, description = "The updated account", body = AdminUserSummary),
        (status = 404, description = "No such account", body = crate::error::ErrorBody),
        (status = 403, description = "Signed in, but not an admin", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn disable_user(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<AdminUserSummary>> {
    let disabled = repositories::admin::disable_user(&state.db, id).await?;
    if !disabled {
        return Err(AppError::NotFound("No such account.".into()));
    }

    audit::record(
        &state.db,
        admin.id,
        AuditAction::DisableUser,
        AuditTargetType::User,
        id,
        serde_json::json!({}),
    )
    .await;

    let summary = repositories::admin::find_user_summary(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("No such account.".into()))?;

    Ok(Json(summary))
}

/// `POST /api/admin/users/:id/enable`
#[utoipa::path(
    post, path = "/api/admin/users/{id}/enable", tag = "admin",
    summary = "Re-enable a disabled account",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path, description = "The account id.", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    responses(
        (status = 200, description = "The updated account", body = AdminUserSummary),
        (status = 404, description = "No such account", body = crate::error::ErrorBody),
        (status = 403, description = "Signed in, but not an admin", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn enable_user(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<AdminUserSummary>> {
    let enabled = repositories::admin::enable_user(&state.db, id).await?;
    if !enabled {
        return Err(AppError::NotFound("No such account.".into()));
    }

    audit::record(
        &state.db,
        admin.id,
        AuditAction::EnableUser,
        AuditTargetType::User,
        id,
        serde_json::json!({}),
    )
    .await;

    let summary = repositories::admin::find_user_summary(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("No such account.".into()))?;

    Ok(Json(summary))
}

// ---------------------------------------------------------------------------
// Vouchers
// ---------------------------------------------------------------------------

const MAX_VOUCHER_DURATION_DAYS: i32 = 3650;
const MAX_VOUCHER_USES: i32 = 1_000_000;
const MAX_VOUCHER_BATCH: i32 = 1000;

#[derive(Deserialize, ToSchema)]
pub struct CreateVoucherRequest {
    pub duration_days: i32,
    pub max_uses: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
    /// How many independent codes to generate with these same settings.
    /// Defaults to 1 — a "bulk" creation is this same request with a bigger
    /// number, not a different shape.
    pub count: Option<i32>,
}

#[derive(Serialize, ToSchema)]
pub struct VoucherListResponse {
    pub vouchers: Vec<Voucher>,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub total_pages: i64,
}

/// `POST /api/admin/vouchers`
#[utoipa::path(
    post, path = "/api/admin/vouchers", tag = "admin",
    summary = "Create one or more voucher codes",
    description = "`count` (default 1) generates that many independent codes sharing the same duration, use limit, expiry and note. Always returns a list, even for a single voucher, so the client never branches on shape.",
    security(("session" = [])),
    request_body = CreateVoucherRequest,
    responses(
        (status = 200, description = "The newly created voucher(s)", body = VoucherListResponse),
        (status = 400, description = "Invalid duration, use limit, or count", body = crate::error::ErrorBody),
        (status = 403, description = "Signed in, but not an admin", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn create_vouchers(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    AppJson(body): AppJson<CreateVoucherRequest>,
) -> AppResult<Json<VoucherListResponse>> {
    let count = body.count.unwrap_or(1);

    if !(1..=MAX_VOUCHER_DURATION_DAYS).contains(&body.duration_days) {
        return Err(AppError::BadRequest(format!(
            "duration_days must be between 1 and {MAX_VOUCHER_DURATION_DAYS}."
        )));
    }
    if !(1..=MAX_VOUCHER_USES).contains(&body.max_uses) {
        return Err(AppError::BadRequest(format!(
            "max_uses must be between 1 and {MAX_VOUCHER_USES}."
        )));
    }
    if !(1..=MAX_VOUCHER_BATCH).contains(&count) {
        return Err(AppError::BadRequest(format!(
            "count must be between 1 and {MAX_VOUCHER_BATCH}."
        )));
    }

    let new = NewVoucher {
        duration_days: body.duration_days,
        max_uses: body.max_uses,
        expires_at: body.expires_at,
        note: body.note,
        created_by: Some(admin.id),
    };

    let vouchers = repositories::voucher::create_bulk(&state.db, &new, count).await?;

    for voucher in &vouchers {
        audit::record(
            &state.db,
            admin.id,
            AuditAction::CreateVoucher,
            AuditTargetType::Voucher,
            voucher.id,
            serde_json::json!({
                "code": voucher.code,
                "duration_days": voucher.duration_days,
                "max_uses": voucher.max_uses,
            }),
        )
        .await;
    }

    Ok(Json(VoucherListResponse {
        vouchers,
        page: 1,
        limit: count as i64,
        total: count as i64,
        total_pages: 1,
    }))
}

/// Plain `page`/`limit`, shared by every admin list that has no other
/// filters — vouchers and the audit log today.
#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

/// `GET /api/admin/vouchers`
#[utoipa::path(
    get, path = "/api/admin/vouchers", tag = "admin",
    summary = "Paginated voucher list",
    security(("session" = [])),
    params(
        ("page" = Option<i64>, Query, description = "1-based page number. Defaults to 1.", example = 1, minimum = 1),
        ("limit" = Option<i64>, Query, description = "Rows per page. Defaults to 20.", example = 20, minimum = 1, maximum = 100),
    ),
    responses(
        (status = 200, description = "One page of vouchers, newest first", body = VoucherListResponse),
        (status = 400, description = "Invalid pagination", body = crate::error::ErrorBody),
        (status = 403, description = "Signed in, but not an admin", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn list_vouchers(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
    AppQuery(query): AppQuery<PageQuery>,
) -> AppResult<Json<VoucherListResponse>> {
    let (page, limit) = validate_pagination(query.page, query.limit)?;
    let offset = (page - 1) * limit;

    let vouchers = repositories::voucher::list(&state.db, limit, offset).await?;
    let total = repositories::voucher::count(&state.db).await?;

    Ok(Json(VoucherListResponse {
        vouchers,
        page,
        limit,
        total,
        total_pages: total_pages(total, limit),
    }))
}

/// `GET /api/admin/vouchers/:id`
#[utoipa::path(
    get, path = "/api/admin/vouchers/{id}", tag = "admin",
    summary = "One voucher and everyone who has redeemed it",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path, description = "The voucher id.", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    responses(
        (status = 200, description = "The voucher and its redemptions", body = AdminVoucherDetail),
        (status = 404, description = "No such voucher", body = crate::error::ErrorBody),
        (status = 403, description = "Signed in, but not an admin", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn get_voucher(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<AdminVoucherDetail>> {
    let voucher = repositories::voucher::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("No such voucher.".into()))?;
    let redemptions = repositories::admin::list_voucher_redemptions(&state.db, id).await?;

    Ok(Json(AdminVoucherDetail {
        voucher,
        redemptions,
    }))
}

/// `POST /api/admin/vouchers/:id/deactivate`
#[utoipa::path(
    post, path = "/api/admin/vouchers/{id}/deactivate", tag = "admin",
    summary = "Deactivate a voucher",
    description = "Existing redemptions are untouched — only future ones are refused.",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path, description = "The voucher id.", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    responses(
        (status = 200, description = "The deactivated voucher", body = Voucher),
        (status = 404, description = "No such voucher", body = crate::error::ErrorBody),
        (status = 403, description = "Signed in, but not an admin", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn deactivate_voucher(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<Voucher>> {
    let deactivated = repositories::voucher::deactivate(&state.db, id).await?;
    if !deactivated {
        return Err(AppError::NotFound("No such voucher.".into()));
    }

    audit::record(
        &state.db,
        admin.id,
        AuditAction::DeactivateVoucher,
        AuditTargetType::Voucher,
        id,
        serde_json::json!({}),
    )
    .await;

    let voucher = repositories::voucher::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("No such voucher.".into()))?;

    Ok(Json(voucher))
}

// ---------------------------------------------------------------------------
// Audit log
// ---------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct AuditLogResponse {
    pub entries: Vec<AuditLogEntry>,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub total_pages: i64,
}

/// `GET /api/admin/audit-log`
#[utoipa::path(
    get, path = "/api/admin/audit-log", tag = "admin",
    summary = "Every mutating admin action, newest first",
    description = "Who extended, disabled, enabled, created a voucher for, or deactivated a voucher for which account — written before each of those requests answers, best-effort (a logging failure never blocks the action itself).",
    security(("session" = [])),
    params(
        ("page" = Option<i64>, Query, description = "1-based page number. Defaults to 1.", example = 1, minimum = 1),
        ("limit" = Option<i64>, Query, description = "Rows per page. Defaults to 20.", example = 20, minimum = 1, maximum = 100),
    ),
    responses(
        (status = 200, description = "One page of audit entries, newest first", body = AuditLogResponse),
        (status = 400, description = "Invalid pagination", body = crate::error::ErrorBody),
        (status = 403, description = "Signed in, but not an admin", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn list_audit_log(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
    AppQuery(query): AppQuery<PageQuery>,
) -> AppResult<Json<AuditLogResponse>> {
    let (page, limit) = validate_pagination(query.page, query.limit)?;
    let offset = (page - 1) * limit;

    let entries = repositories::audit::list(&state.db, limit, offset).await?;
    let total = repositories::audit::count(&state.db).await?;

    Ok(Json(AuditLogResponse {
        entries,
        page,
        limit,
        total,
        total_pages: total_pages(total, limit),
    }))
}

fn validate_account_status(raw: Option<String>) -> AppResult<Option<String>> {
    match raw.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some("active") => Ok(Some("active".into())),
        Some("disabled") => Ok(Some("disabled".into())),
        Some(other) => Err(AppError::BadRequest(format!(
            "Unknown status '{other}'. Use 'active' or 'disabled'."
        ))),
    }
}

fn validate_plan(raw: Option<String>) -> AppResult<Option<String>> {
    match raw.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(other) => SubscriptionStatus::parse(other)
            .map(|status| Some(status.slug().to_string()))
            .ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Unknown plan '{other}'. Use 'trialing', 'active', 'expired', 'cancelled' or 'past_due'."
                ))
            }),
    }
}

fn validate_pagination(page: Option<i64>, limit: Option<i64>) -> AppResult<(i64, i64)> {
    let page = page.unwrap_or(1);
    let limit = limit.unwrap_or(DEFAULT_LIMIT);

    if page < 1 {
        return Err(AppError::BadRequest("page must be 1 or greater.".into()));
    }
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(AppError::BadRequest(format!(
            "limit must be between 1 and {MAX_LIMIT}."
        )));
    }
    if page
        .checked_sub(1)
        .and_then(|p| p.checked_mul(limit))
        .is_none()
    {
        return Err(AppError::BadRequest("page is too large.".into()));
    }

    Ok((page, limit))
}

fn total_pages(total: i64, limit: i64) -> i64 {
    if total <= 0 {
        0
    } else {
        (total + limit - 1) / limit
    }
}
