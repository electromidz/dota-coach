//! One-off: correct stored net worth.
//!
//! When OpenDota omitted a player's `net_worth`, the match was stored with
//! `total_gold` instead. Total gold counts gold already spent, so those rows
//! overstate net worth, sometimes several times over. This fetches every
//! stored match again and writes the real figure, or clears it when the
//! provider has none.
//!
//! Dry run by default: prints what would change. `--apply` writes.
//! Paced at about one provider request per second.
//!
//! Run with `cargo run --bin backfill_net_worth [-- --apply]`.

use std::time::Duration;

use dota_coach_backend::config::Config;
use dota_coach_backend::services::dota::opendota::OpenDotaProvider;
use dota_coach_backend::services::dota::DotaDataProvider;
use dota_coach_backend::{db, repositories};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let apply = std::env::args().any(|a| a == "--apply");

    let config = Config::from_env()?;
    let pool = db::connect(&config).await?;
    let dota = OpenDotaProvider::new(&config.dota)?;

    let matches = repositories::r#match::all_with_accounts(&pool).await?;
    println!("{} stored matches", matches.len());

    let (mut changed, mut same, mut failed) = (0, 0, 0);
    for (id, match_id, account_id, stored) in matches {
        tokio::time::sleep(Duration::from_millis(1100)).await;
        let detail = match dota.get_match_details(match_id, account_id).await {
            Ok(detail) => detail,
            Err(e) => {
                failed += 1;
                eprintln!("{match_id}: {e}");
                continue;
            }
        };
        if detail.net_worth == stored {
            same += 1;
            continue;
        }
        changed += 1;
        println!(
            "{match_id}: {} -> {}",
            stored.map_or("none".into(), |v| v.to_string()),
            detail.net_worth.map_or("none".into(), |v| v.to_string()),
        );
        if apply {
            repositories::r#match::set_net_worth(&pool, id, detail.net_worth).await?;
        }
    }

    println!(
        "net worth: {changed} {}, {same} unchanged, {failed} could not be fetched",
        if apply { "updated" } else { "would change" }
    );
    Ok(())
}
