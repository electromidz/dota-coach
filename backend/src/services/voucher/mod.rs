//! Redeeming a voucher: turning a code into subscription time.
//!
//! Every check before the redemption insert is for a clear error message,
//! not the guarantee itself. The guarantee is the database: the voucher row
//! is locked (`SELECT ... FOR UPDATE`) for the length of the transaction,
//! and `UNIQUE(voucher_id, user_id)` on `voucher_redemptions` is what makes
//! two simultaneous redemption attempts by the same user resolve to exactly
//! one success, whatever the pre-checks believed a moment earlier.

mod rate_limit;

pub use rate_limit::RateLimiter;

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::BillingConfig;
use crate::domain::billing::{next_period, Subscription};
use crate::domain::event::EventType;
use crate::repositories;
use crate::services::events;

#[derive(Debug, thiserror::Error)]
pub enum VoucherError {
    #[error("that code is not valid")]
    NotFound,
    #[error("that code is no longer active")]
    Inactive,
    #[error("that code has expired")]
    Expired,
    #[error("that code has already been used the maximum number of times")]
    UsedUp,
    #[error("you have already redeemed this code")]
    AlreadyRedeemed,
    #[error("too many attempts — try again in a minute")]
    RateLimited,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub async fn redeem(
    pool: &PgPool,
    billing_config: &BillingConfig,
    limiter: &RateLimiter,
    user_id: Uuid,
    code: &str,
) -> Result<Subscription, VoucherError> {
    if !limiter.check(user_id) {
        return Err(VoucherError::RateLimited);
    }

    // Real accounts get a subscription row at login (Phase 3), but that call
    // is best-effort — a transient failure there logs a warning rather than
    // blocking sign-in — so this is not provably unreachable. Idempotent and
    // cheap: a no-op for the overwhelming majority of callers who already
    // have a row.
    repositories::billing::ensure_subscription(
        pool,
        user_id,
        &billing_config.plan,
        billing_config.trial_days,
    )
    .await?;

    let now = Utc::now();
    let mut tx = pool.begin().await?;

    let voucher = repositories::voucher::find_by_code_for_update(&mut tx, code)
        .await?
        .ok_or(VoucherError::NotFound)?;

    if !voucher.active {
        return Err(VoucherError::Inactive);
    }
    if voucher
        .expires_at
        .is_some_and(|expires_at| expires_at <= now)
    {
        return Err(VoucherError::Expired);
    }
    if voucher.used_count >= voucher.max_uses {
        return Err(VoucherError::UsedUp);
    }
    if repositories::voucher::has_redeemed(&mut tx, voucher.id, user_id).await? {
        return Err(VoucherError::AlreadyRedeemed);
    }

    // The pre-check above is a better error message; this insert is the
    // actual guarantee under concurrency.
    match repositories::voucher::record_redemption(&mut tx, voucher.id, user_id).await {
        Ok(()) => {}
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(VoucherError::AlreadyRedeemed);
        }
        Err(e) => return Err(e.into()),
    }

    repositories::voucher::increment_used_count(&mut tx, voucher.id).await?;

    // Guaranteed to exist by the `ensure_subscription` call above; a miss
    // here would mean the account vanished mid-request.
    let subscription = repositories::billing::lock_subscription_for_user(&mut tx, user_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;

    let (start, end) = next_period(
        subscription.current_period_end,
        now,
        i64::from(voucher.duration_days),
    );
    repositories::billing::extend_with_voucher(&mut tx, subscription.id, start, end).await?;

    tx.commit().await?;

    events::track(
        pool,
        user_id,
        EventType::VoucherRedeemed,
        serde_json::json!({ "code": voucher.code, "duration_days": voucher.duration_days }),
    )
    .await;

    // Re-read outside the now-closed transaction for the response.
    repositories::billing::find_by_user(pool, user_id)
        .await?
        .ok_or_else(|| VoucherError::Database(sqlx::Error::RowNotFound))
}
