//! Dota data providers.
//!
//! Callers depend on [`DotaDataProvider`] only. OpenDota response shapes stop
//! at this boundary, so a STRATZ implementation can be dropped in later without
//! touching handlers, services or repositories.

pub mod opendota;

use std::collections::HashMap;

use async_trait::async_trait;

use crate::domain::r#match::NormalizedMatch;

/// A player as the provider knows them.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderPlayer {
    pub account_id: i64,
    pub persona_name: Option<String>,
    pub avatar_url: Option<String>,
    pub profile_url: Option<String>,
    pub rank_tier: Option<i32>,
    /// False when the provider has no public profile for this account. The
    /// account may still have matches.
    pub has_public_profile: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("not found at provider")]
    NotFound,
    #[error("provider rate limit reached")]
    RateLimited,
    #[error("provider unavailable: {0}")]
    Unavailable(String),
    #[error("unexpected provider response: {0}")]
    Decode(String),
}

#[async_trait]
pub trait DotaDataProvider: Send + Sync {
    async fn get_player(&self, account_id: i64) -> Result<ProviderPlayer, ProviderError>;

    async fn get_player_matches(
        &self,
        account_id: i64,
        limit: u32,
    ) -> Result<Vec<NormalizedMatch>, ProviderError>;

    async fn get_match_details(
        &self,
        match_id: i64,
        account_id: i64,
    ) -> Result<NormalizedMatch, ProviderError>;

    /// Hero id -> display name. Implementations are expected to cache this.
    async fn heroes(&self) -> Result<HashMap<i32, String>, ProviderError>;
}

/// Fallback label when the hero catalogue is unavailable, so a sync never fails
/// just because a cosmetic lookup did.
pub fn fallback_hero_name(hero_id: i32) -> String {
    format!("Hero {hero_id}")
}
