//! Hero meta providers: what the wider player base is doing right now.
//!
//! The provider supplies counts. The *scoring* lives in [`strength`], above the
//! boundary, so replacing OpenDota with STRATZ changes where picks and wins
//! come from and nothing about how strong a hero is judged to be.
//!
//! Provider priority per the spec is STRATZ, then OpenDota, then an optional
//! Dotabuff. Only OpenDota is implemented: STRATZ's GraphQL API rejects
//! unauthenticated requests (verified: `POST https://api.stratz.com/graphql`
//! answers `403` without a token), so its response shape cannot be verified,
//! and inventing it would breach the "never invent provider fields" rule. The
//! trait is the seam it drops into once a token exists.

pub mod opendota;
pub mod strength;

use async_trait::async_trait;

use crate::domain::benchmark::Segment;
use crate::domain::hero::{HeroMeta, HeroMetaContext, RankBracket};

#[derive(Debug, thiserror::Error)]
pub enum HeroMetaError {
    #[error("hero meta provider unavailable: {0}")]
    Unavailable(String),
    #[error("hero meta provider rate limited")]
    RateLimited,
    #[error("unexpected hero meta response: {0}")]
    InvalidResponse(String),
    #[error("no hero meta for this context")]
    NotFound,
}

impl HeroMetaError {
    /// A short, user-safe explanation. Hero meta is a degradable feature: the
    /// page still renders the player's own pool without it, so the reason is
    /// shown rather than turned into an error response.
    pub fn user_note(&self) -> &'static str {
        match self {
            HeroMetaError::RateLimited => {
                "The hero meta provider is rate limiting us. Try again shortly."
            }
            HeroMetaError::NotFound => "No hero meta is published for your bracket yet.",
            _ => "Hero meta is unavailable right now.",
        }
    }
}

/// The cohort a provider answered with.
pub struct HeroMetaSet {
    pub heroes: Vec<HeroMeta>,
    /// Dimensions genuinely segmented on — not the ones requested.
    pub segmented_by: Vec<Segment>,
    /// The bracket actually used, which may be `None` even when one was asked
    /// for: a provider with no data for Immortal falls back to all brackets
    /// and says so here rather than pretending.
    pub bracket: Option<RankBracket>,
    /// Name of the provider, for display. Never a secret.
    pub source: &'static str,
    /// Set when the cohort carries a limitation worth surfacing.
    pub note: Option<String>,
}

#[async_trait]
pub trait HeroMetaProvider: Send + Sync {
    async fn get_hero_meta(&self, context: &HeroMetaContext) -> Result<HeroMetaSet, HeroMetaError>;
}
