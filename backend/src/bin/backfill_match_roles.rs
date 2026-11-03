//! One-off: re-estimate stored match roles and normalise hero names.
//!
//! Roles for matches without lane data come from farm priority within the
//! team. That used to be ranked on end-of-game net worth, which put carries
//! who spent on buybacks below their supports ("Terrorblade, Hard Support").
//! It is now ranked on gold per minute; this re-derives the stored roles by
//! fetching each such match again.
//!
//! Hero names are copied onto matches at sync time, so a renamed hero keeps
//! its old name on older matches; this rewrites them to the catalogue's.
//!
//! Dry run by default: prints what would change. `--apply` writes.
//! Paced at about one provider request per second.
//!
//! Run with `cargo run --bin backfill_match_roles [-- --apply]`.

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

    // Names first: one statement, no provider calls beyond the catalogue.
    let names = dota.heroes().await?;
    if apply {
        let renamed = repositories::r#match::normalize_hero_names(&pool, None, &names).await?;
        println!("hero names: {renamed} matches renamed");
    } else {
        println!("hero names: dry run, {} heroes in the catalogue", names.len());
    }

    let candidates = repositories::r#match::farm_ranked_matches(&pool).await?;
    println!("{} matches have a farm-priority role", candidates.len());

    let (mut changed, mut same, mut failed) = (0, 0, 0);
    for (id, match_id, account_id, stored) in candidates {
        tokio::time::sleep(Duration::from_millis(1100)).await;
        let detail = match dota.get_match_details(match_id, account_id).await {
            Ok(detail) => detail,
            Err(e) => {
                failed += 1;
                eprintln!("{match_id}: {e}");
                continue;
            }
        };
        let role = detail.derive_role().as_str();
        if role == stored {
            same += 1;
            continue;
        }
        changed += 1;
        println!("{match_id}: {stored} -> {role}");
        if apply {
            repositories::r#match::set_role(&pool, id, role).await?;
        }
    }

    println!(
        "roles: {changed} {}, {same} unchanged, {failed} could not be fetched",
        if apply { "updated" } else { "would change" }
    );
    Ok(())
}
