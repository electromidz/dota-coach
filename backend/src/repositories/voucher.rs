//! Voucher storage: the codes an admin makes, and who has redeemed them.
//!
//! Every write here that matters under concurrency is deliberately narrow —
//! `find_by_code_for_update`, `increment_used_count` and `record_redemption`
//! are only ever called together, inside one transaction, from
//! `services::voucher::redeem`. Reading this file alone will not explain why
//! a voucher can never be over-redeemed; that guarantee is the combination
//! of the row lock and the `UNIQUE(voucher_id, user_id)` constraint from
//! `migrations/0016_vouchers.sql`, exercised there.

use chrono::{DateTime, Utc};
use rand::Rng;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::domain::voucher::{Voucher, VoucherRedemption};

const CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
/// The keyspace (32^8, over a trillion) makes a real collision practically
/// impossible; this bounds the retry loop so a pathological run fails loudly
/// instead of looping forever.
const MAX_CODE_ATTEMPTS: u32 = 10;

macro_rules! voucher_columns {
    () => {
        "id, code, duration_days, max_uses, used_count, expires_at, active, note,
         created_by, created_at"
    };
}

fn code_group(rng: &mut impl Rng) -> String {
    (0..4)
        .map(|_| CODE_ALPHABET[rng.random_range(0..CODE_ALPHABET.len())] as char)
        .collect()
}

/// `DOTA-XXXX-XXXX` over an alphabet with no `0/O` or `1/I` — a code that has
/// to be read aloud or retyped from a screenshot should never have a
/// character two people could reasonably disagree about.
fn generate_code() -> String {
    let mut rng = rand::rng();
    format!("DOTA-{}-{}", code_group(&mut rng), code_group(&mut rng))
}

pub struct NewVoucher {
    pub duration_days: i32,
    pub max_uses: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
    pub created_by: Option<Uuid>,
}

/// Creates one voucher with a freshly generated code, retrying on the
/// vanishingly unlikely event of a collision with an existing one.
pub async fn create(pool: &PgPool, new: &NewVoucher) -> Result<Voucher, sqlx::Error> {
    for _ in 0..MAX_CODE_ATTEMPTS {
        let code = generate_code();

        let result = sqlx::query_as::<_, Voucher>(concat!(
            "INSERT INTO vouchers (code, duration_days, max_uses, expires_at, note, created_by)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING ",
            voucher_columns!()
        ))
        .bind(&code)
        .bind(new.duration_days)
        .bind(new.max_uses)
        .bind(new.expires_at)
        .bind(&new.note)
        .bind(new.created_by)
        .fetch_one(pool)
        .await;

        match result {
            Ok(voucher) => return Ok(voucher),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => continue,
            Err(e) => return Err(e),
        }
    }

    Err(sqlx::Error::Protocol(
        "could not generate a unique voucher code".into(),
    ))
}

/// `count` independent vouchers sharing the same duration/max-uses/expiry —
/// a bulk giveaway batch. Not wrapped in one transaction: each voucher is
/// useful on its own, so a failure partway through still leaves whatever was
/// already created as valid, redeemable codes rather than rolling them back.
pub async fn create_bulk(
    pool: &PgPool,
    new: &NewVoucher,
    count: i32,
) -> Result<Vec<Voucher>, sqlx::Error> {
    let mut vouchers = Vec::with_capacity(count.max(0) as usize);
    for _ in 0..count {
        vouchers.push(create(pool, new).await?);
    }
    Ok(vouchers)
}

pub async fn find_by_code(pool: &PgPool, code: &str) -> Result<Option<Voucher>, sqlx::Error> {
    sqlx::query_as::<_, Voucher>(concat!(
        "SELECT ",
        voucher_columns!(),
        " FROM vouchers WHERE code = $1"
    ))
    .bind(code)
    .fetch_optional(pool)
    .await
}

/// The redemption path's row lock. Held for the length of the caller's
/// transaction, so a second simultaneous redemption of the same code blocks
/// here until the first has committed — at which point it sees the
/// incremented `used_count`, not the value that made the first one look safe.
pub async fn find_by_code_for_update(
    tx: &mut Transaction<'_, Postgres>,
    code: &str,
) -> Result<Option<Voucher>, sqlx::Error> {
    sqlx::query_as::<_, Voucher>(concat!(
        "SELECT ",
        voucher_columns!(),
        " FROM vouchers WHERE code = $1 FOR UPDATE"
    ))
    .bind(code)
    .fetch_optional(&mut **tx)
    .await
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Voucher>, sqlx::Error> {
    sqlx::query_as::<_, Voucher>(concat!(
        "SELECT ",
        voucher_columns!(),
        " FROM vouchers WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<Voucher>, sqlx::Error> {
    sqlx::query_as::<_, Voucher>(concat!(
        "SELECT ",
        voucher_columns!(),
        " FROM vouchers ORDER BY created_at DESC LIMIT $1 OFFSET $2"
    ))
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn count(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM vouchers")
        .fetch_one(pool)
        .await
}

/// A pre-check for a clear error message, not the actual guarantee — see
/// `record_redemption`.
pub async fn has_redeemed(
    tx: &mut Transaction<'_, Postgres>,
    voucher_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let redeemed: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM voucher_redemptions WHERE voucher_id = $1 AND user_id = $2",
    )
    .bind(voucher_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(redeemed.is_some())
}

/// The actual concurrency guarantee. `UNIQUE(voucher_id, user_id)` means two
/// simultaneous redemption attempts by the same user cannot both insert —
/// the loser gets a unique-violation `sqlx::Error`, which
/// `services::voucher::redeem` maps to `VoucherError::AlreadyRedeemed`
/// regardless of what `has_redeemed` saw a moment earlier.
pub async fn record_redemption(
    tx: &mut Transaction<'_, Postgres>,
    voucher_id: Uuid,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO voucher_redemptions (voucher_id, user_id) VALUES ($1, $2)")
        .bind(voucher_id)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

pub async fn increment_used_count(
    tx: &mut Transaction<'_, Postgres>,
    voucher_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE vouchers SET used_count = used_count + 1 WHERE id = $1")
        .bind(voucher_id)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

/// Every redemption of one voucher, newest first — "who redeemed this" on
/// the admin voucher-detail page.
pub async fn list_redemptions(
    pool: &PgPool,
    voucher_id: Uuid,
) -> Result<Vec<VoucherRedemption>, sqlx::Error> {
    sqlx::query_as::<_, VoucherRedemption>(
        "SELECT id, voucher_id, user_id, redeemed_at
           FROM voucher_redemptions
          WHERE voucher_id = $1
          ORDER BY redeemed_at DESC",
    )
    .bind(voucher_id)
    .fetch_all(pool)
    .await
}

/// Returns `false` when the id does not exist.
pub async fn deactivate(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE vouchers SET active = false WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}
