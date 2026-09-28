//! Event-level match data, behind a trait.
//!
//! A second Dota provider boundary, beside [`crate::services::dota`], and
//! deliberately not folded into it. The two answer different questions and fail
//! independently:
//!
//! | Boundary | Question | Used by |
//! |---|---|---|
//! | [`crate::services::dota::DotaDataProvider`] | What are this player's matches, and what were the totals? | sync, metrics, benchmarks, every page |
//! | [`MatchFactsProvider`] | What happened, second by second, in *this one* match? | single-match analysis |
//!
//! Keeping them apart is what lets STRATZ be the source of truth for the
//! timeline while OpenDota stays the source of truth for the history, and what
//! lets a STRATZ outage cost exactly one section of one page instead of the
//! match list, the stats and the benchmarks with it.
//!
//! [`ProviderError`] is reused rather than redefined: a provider failure means
//! the same things here as it does there, and the API already translates it
//! once, in `crate::error`.

pub mod stratz;

use async_trait::async_trait;

use crate::domain::match_facts::MatchFacts;
use crate::services::dota::ProviderError;

#[async_trait]
pub trait MatchFactsProvider: Send + Sync {
    /// Whether this deployment can reach the provider at all.
    ///
    /// Configuration, not health: false means no credentials were supplied, so
    /// waiting will not help and the caller should degrade rather than retry.
    fn is_configured(&self) -> bool;

    /// Human-readable provider name, for logs and for the note a user reads
    /// when the timeline is missing.
    fn name(&self) -> &'static str;

    /// The timestamped record of one match, for one player in it.
    ///
    /// `ProviderError::NotFound` covers both "no such match" and "that player
    /// is not in this match" — from the caller's side they are the same answer,
    /// and distinguishing them would tell an attacker which match ids exist.
    async fn get_match_facts(
        &self,
        match_id: i64,
        account_id: i64,
    ) -> Result<MatchFacts, ProviderError>;
}

/// The provider a deployment gets when no credentials are configured.
///
/// Mirrors `UnconfiguredPaymentProvider`: choosing a null implementation once,
/// at startup, keeps every caller free of the question. The single-match
/// analysis still works without it — it falls back to the stored aggregate and
/// says so — so this is a reduction in depth, not an outage.
pub struct UnconfiguredMatchFactsProvider;

#[async_trait]
impl MatchFactsProvider for UnconfiguredMatchFactsProvider {
    fn is_configured(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "STRATZ"
    }

    async fn get_match_facts(
        &self,
        _match_id: i64,
        _account_id: i64,
    ) -> Result<MatchFacts, ProviderError> {
        Err(ProviderError::Unavailable(
            "no match-facts provider is configured".into(),
        ))
    }
}
