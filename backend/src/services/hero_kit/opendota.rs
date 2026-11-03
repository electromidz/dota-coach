//! OpenDota implementation of [`HeroKitProvider`].
//!
//! Verified against the live endpoints before this was written (no key
//! required; the same rate limit as the rest of OpenDota):
//!
//!   - `GET /constants/heroes` — `{ "<id>": { id, name: "npc_dota_hero_*",
//!     localized_name, roles: [..] } }`
//!   - `GET /constants/hero_abilities` — `{ "npc_dota_hero_*": { abilities:
//!     ["<ability key>", ..] } }`, including `generic_hidden` placeholders
//!   - `GET /constants/abilities` — `{ "<ability key>": { dname, .. } }`
//!   - `GET /constants/items` — `{ "<item key>": { id, dname, cost, .. } }`
//!   - `GET /heroes/{id}/itemPopularity` — `{ start_game_items, early_game_items,
//!     mid_game_items, late_game_items }`, each `{ "<item id>": purchases }`
//!
//! The constants are one document each for the whole game, so they are
//! fetched once and kept for a day. Item popularity is per hero and kept for a
//! day too.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::sync::RwLock;

use super::{HeroKitError, HeroKitProvider};
use crate::domain::hero::HeroKit;

const TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// A built item at or above this cost counts as core.
const CORE_BUILT_MIN_COST: i64 = 1000;
/// A basic item (no components) counts only at this cost, e.g. Blink Dagger.
const CORE_BASIC_MIN_COST: i64 = 2000;
const MAX_ABILITIES: usize = 6;
const MAX_CORE_ITEMS: usize = 4;

#[derive(Deserialize)]
struct HeroConst {
    name: String,
    localized_name: String,
    #[serde(default)]
    roles: Vec<String>,
}

#[derive(Deserialize)]
struct HeroAbilities {
    /// Mostly ability keys, but a hero with alternate forms has a nested list
    /// (Monkey King's transform pair), so entries are read loosely.
    #[serde(default)]
    abilities: Vec<serde_json::Value>,
}

/// Ability keys, with nested lists flattened and anything else ignored.
fn ability_keys(values: Vec<serde_json::Value>) -> Vec<String> {
    let mut keys = Vec::new();
    for value in values {
        match value {
            serde_json::Value::String(key) => keys.push(key),
            serde_json::Value::Array(inner) => keys.extend(ability_keys(inner)),
            _ => {}
        }
    }
    keys
}

#[derive(Deserialize)]
struct AbilityConst {
    #[serde(default)]
    dname: Option<String>,
}

#[derive(Deserialize)]
struct ItemConst {
    id: i64,
    #[serde(default)]
    dname: Option<String>,
    #[serde(default)]
    cost: Option<i64>,
    /// A list for built items, null for basic ones.
    #[serde(default)]
    components: Option<serde_json::Value>,
}

/// A finished item rather than a component or consumable.
fn is_core(cost: i64, built: bool) -> bool {
    if built {
        cost >= CORE_BUILT_MIN_COST
    } else {
        cost >= CORE_BASIC_MIN_COST
    }
}

#[derive(Deserialize)]
struct ItemPopularity {
    #[serde(default)]
    early_game_items: HashMap<String, i64>,
    #[serde(default)]
    mid_game_items: HashMap<String, i64>,
}

/// The four constant documents, reduced to what a kit needs.
struct Constants {
    heroes: HashMap<i32, HeroConst>,
    abilities_by_hero: HashMap<String, Vec<String>>,
    ability_names: HashMap<String, String>,
    /// Item id -> (display name, counts as core).
    items: HashMap<i64, (String, bool)>,
}

pub struct OpenDotaHeroKitProvider {
    http: Client,
    base_url: String,
    api_key: Option<String>,
    constants: RwLock<Option<(Instant, Arc<Constants>)>>,
    popularity: RwLock<HashMap<i32, (Instant, Vec<i64>)>>,
}

impl OpenDotaHeroKitProvider {
    pub fn new(http: Client, base_url: &str, api_key: Option<String>) -> Arc<Self> {
        Arc::new(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            constants: RwLock::new(None),
            popularity: RwLock::new(HashMap::new()),
        })
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, HeroKitError> {
        let mut request = self.http.get(format!("{}/{path}", self.base_url));
        if let Some(key) = &self.api_key {
            request = request.query(&[("api_key", key)]);
        }
        let response = request
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(|e| HeroKitError::Unavailable(e.to_string()))?;
        match response.status() {
            StatusCode::TOO_MANY_REQUESTS => return Err(HeroKitError::RateLimited),
            StatusCode::NOT_FOUND => return Err(HeroKitError::NotFound),
            s if !s.is_success() => return Err(HeroKitError::Unavailable(format!("status {s}"))),
            _ => {}
        }
        // Read then parse, so a schema mismatch names the field rather than
        // surfacing as reqwest's generic "error decoding response body".
        let body = response
            .bytes()
            .await
            .map_err(|e| HeroKitError::Unavailable(e.to_string()))?;
        serde_json::from_slice::<T>(&body)
            .map_err(|e| HeroKitError::InvalidResponse(format!("{path}: {e}")))
    }

    async fn constants(&self) -> Result<Arc<Constants>, HeroKitError> {
        if let Some((at, c)) = self.constants.read().await.as_ref() {
            if at.elapsed() < TTL {
                return Ok(c.clone());
            }
        }

        let heroes: HashMap<String, HeroConst> = self.get("constants/heroes").await?;
        let hero_abilities: HashMap<String, HeroAbilities> =
            self.get("constants/hero_abilities").await?;
        let abilities: HashMap<String, AbilityConst> = self.get("constants/abilities").await?;
        let items: HashMap<String, ItemConst> = self.get("constants/items").await?;

        let built = Arc::new(Constants {
            heroes: heroes
                .into_iter()
                .filter_map(|(id, h)| id.parse().ok().map(|id| (id, h)))
                .collect(),
            abilities_by_hero: hero_abilities
                .into_iter()
                .map(|(hero, a)| (hero, ability_keys(a.abilities)))
                .collect(),
            ability_names: abilities
                .into_iter()
                .filter_map(|(key, a)| a.dname.map(|name| (key, name)))
                .collect(),
            items: items
                .into_values()
                .filter_map(|i| {
                    let built = i
                        .components
                        .as_ref()
                        .and_then(|c| c.as_array())
                        .is_some_and(|c| !c.is_empty());
                    let core = is_core(i.cost.unwrap_or(0), built);
                    i.dname.map(|name| (i.id, (name, core)))
                })
                .collect(),
        });

        *self.constants.write().await = Some((Instant::now(), built.clone()));
        Ok(built)
    }

    /// Item ids by purchase count across early and mid game, most bought first.
    async fn popular_items(&self, hero_id: i32) -> Result<Vec<i64>, HeroKitError> {
        if let Some((at, ids)) = self.popularity.read().await.get(&hero_id) {
            if at.elapsed() < TTL {
                return Ok(ids.clone());
            }
        }
        let popularity: ItemPopularity = self.get(&format!("heroes/{hero_id}/itemPopularity")).await?;
        let ids = rank_items(&popularity.early_game_items, &popularity.mid_game_items);
        self.popularity
            .write()
            .await
            .insert(hero_id, (Instant::now(), ids.clone()));
        Ok(ids)
    }
}

/// Merge two `{ item id: purchases }` maps and sort by purchases, descending.
fn rank_items(early: &HashMap<String, i64>, mid: &HashMap<String, i64>) -> Vec<i64> {
    let mut totals: HashMap<i64, i64> = HashMap::new();
    for (id, count) in early.iter().chain(mid.iter()) {
        if let Ok(id) = id.parse::<i64>() {
            *totals.entry(id).or_default() += count;
        }
    }
    let mut ranked: Vec<(i64, i64)> = totals.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.into_iter().map(|(id, _)| id).collect()
}

#[async_trait]
impl HeroKitProvider for OpenDotaHeroKitProvider {
    async fn get_kit(&self, hero_id: i32) -> Result<HeroKit, HeroKitError> {
        let constants = self.constants().await?;
        let hero = constants.heroes.get(&hero_id).ok_or(HeroKitError::NotFound)?;

        let abilities: Vec<String> = constants
            .abilities_by_hero
            .get(&hero.name)
            .map(|keys| {
                keys.iter()
                    .filter(|k| !k.starts_with("generic_"))
                    .filter_map(|k| constants.ability_names.get(k).cloned())
                    .take(MAX_ABILITIES)
                    .collect()
            })
            .unwrap_or_default();

        // A popularity outage still leaves the abilities worth sending.
        let core_items: Vec<String> = match self.popular_items(hero_id).await {
            Ok(ids) => ids
                .into_iter()
                .filter_map(|id| constants.items.get(&id))
                .filter(|(_, core)| *core)
                .map(|(name, _)| name.clone())
                .take(MAX_CORE_ITEMS)
                .collect(),
            Err(e) => {
                tracing::debug!(error = %e, hero_id, "item popularity unavailable");
                Vec::new()
            }
        };

        Ok(HeroKit {
            hero_id,
            hero_name: hero.localized_name.clone(),
            roles: hero.roles.clone(),
            abilities,
            core_items,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hits the real API. `cargo test -- --ignored live_kit` to run.
    #[tokio::test]
    #[ignore]
    async fn live_kit() {
        let provider = OpenDotaHeroKitProvider::new(Client::new(), "https://api.opendota.com/api", None);
        let kit = provider.get_kit(9).await.expect("kit");
        println!("{kit:?}");
        assert!(!kit.abilities.is_empty());
    }

    #[test]
    fn components_and_cheap_items_are_not_core() {
        assert!(!is_core(1000, false)); // Staff of Wizardry
        assert!(!is_core(460, true)); // Magic Wand
        assert!(is_core(1500, true)); // Arcane Boots
        assert!(is_core(2250, false)); // Blink Dagger
    }

    #[test]
    fn items_are_ranked_by_purchases_across_early_and_mid_game() {
        let early = HashMap::from([("1".to_string(), 10), ("2".to_string(), 50)]);
        let mid = HashMap::from([("1".to_string(), 45), ("3".to_string(), 5), ("x".to_string(), 99)]);
        // 1: 55, 2: 50, 3: 5; a non-numeric key is ignored.
        assert_eq!(rank_items(&early, &mid), vec![1, 2, 3]);
    }

    #[test]
    fn the_live_document_shapes_parse() {
        let hero: HashMap<String, HeroConst> = serde_json::from_str(
            r#"{"35":{"id":35,"name":"npc_dota_hero_sniper","localized_name":"Sniper","roles":["Carry","Nuker"],"base_health":120}}"#,
        )
        .unwrap();
        assert_eq!(hero["35"].localized_name, "Sniper");

        let pop: ItemPopularity = serde_json::from_str(
            r#"{"start_game_items":{"16":143},"early_game_items":{"29":100},"mid_game_items":{"1":40},"late_game_items":{}}"#,
        )
        .unwrap();
        assert_eq!(pop.early_game_items["29"], 100);

        // Monkey King's list nests his transform pair; it must not sink the
        // whole document.
        let abilities: HashMap<String, HeroAbilities> = serde_json::from_str(
            r#"{"npc_dota_hero_monkey_king":{"abilities":["monkey_king_boundless_strike",["monkey_king_untransform","monkey_king_transfiguration"]]}}"#,
        )
        .unwrap();
        let keys = ability_keys(
            abilities.into_values().next().unwrap().abilities,
        );
        assert_eq!(keys.len(), 3);

        let item: HashMap<String, ItemConst> =
            serde_json::from_str(r#"{"blink":{"id":1,"dname":"Blink Dagger","cost":2250}}"#).unwrap();
        assert_eq!(item["blink"].cost, Some(2250));
    }
}
