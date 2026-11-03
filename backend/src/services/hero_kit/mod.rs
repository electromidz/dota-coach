//! What a hero *is*: its abilities and the items players build on it.
//!
//! Fed to the coach as evidence so advice can name "Static Storm" or "Force
//! Staff" instead of saying "use your spells well". Facts about the hero, not
//! about the player: nothing here is a statistic about anyone's games.
//!
//! Behind a trait like every other provider. The coaching layer never knows
//! the data came from OpenDota, and an outage costs only these facts.

pub mod opendota;

use async_trait::async_trait;

use crate::domain::hero::HeroKit;

#[derive(Debug, thiserror::Error)]
pub enum HeroKitError {
    #[error("hero kit provider unavailable: {0}")]
    Unavailable(String),
    #[error("hero kit provider rate limited")]
    RateLimited,
    #[error("unexpected hero kit response: {0}")]
    InvalidResponse(String),
    #[error("no hero kit for this hero")]
    NotFound,
}

#[async_trait]
pub trait HeroKitProvider: Send + Sync {
    async fn get_kit(&self, hero_id: i32) -> Result<HeroKit, HeroKitError>;
}

/// No provider configured: every lookup is a clean miss.
pub struct NullHeroKitProvider;

#[async_trait]
impl HeroKitProvider for NullHeroKitProvider {
    async fn get_kit(&self, _hero_id: i32) -> Result<HeroKit, HeroKitError> {
        Err(HeroKitError::NotFound)
    }
}
