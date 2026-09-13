//! Normalized internal domain models.
//!
//! Nothing provider-specific lives here: OpenDota/STRATZ response shapes are
//! translated into these types at the provider boundary.
//!
//! Phase 4 adds `metrics`; Phase 5 `coaching`.

pub mod r#match;
pub mod metrics;
pub mod player;
pub mod session;
pub mod user;
