//! One-off: give every existing account a subscription row.
//!
//! Phase 3 materializes the trial at login. An account that signed up before
//! that shipped, and has not logged in since, has no `subscriptions` row at
//! all — invisible to admin stats as "in trial" even though its clock
//! (anchored to `users.created_at`) has been running the whole time. This
//! backfills it for everyone at once instead of waiting for each account's
//! next visit.
//!
//! Idempotent: goes through `services::billing::subscription_for`, the exact
//! path a real login takes, so a rerun just finds each row already there and
//! does nothing to it. Also emits `trial_started` — and `trial_expired` for
//! any backfilled trial whose window had already closed — the same events a
//! real login or the background sweep would have produced.
//!
//! Run with `cargo run --bin backfill_subscriptions`.

use dota_coach_backend::config::Config;
use dota_coach_backend::services::billing;
use dota_coach_backend::{db, repositories};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    let config = Config::from_env()?;
    let pool = db::connect(&config).await?;

    let user_ids = repositories::user::ids_without_subscription(&pool).await?;
    println!("{} accounts have no subscription row", user_ids.len());

    let mut done = 0;
    let mut failed = 0;
    for user_id in user_ids {
        match billing::subscription_for(&pool, &config.billing, user_id).await {
            Ok(_) => done += 1,
            Err(e) => {
                failed += 1;
                eprintln!("could not backfill {user_id}: {e}");
            }
        }
    }

    println!("backfilled {done} accounts ({failed} failed)");
    Ok(())
}
