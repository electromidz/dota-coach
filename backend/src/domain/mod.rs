//! Normalized internal domain models.
//!
//! Nothing provider-specific lives here: OpenDota/STRATZ response shapes are
//! translated into these types at the provider boundary.
//!
//! Phase 4 adds `metrics`; Phase 5 `coaching`.

pub mod benchmark;
pub mod billing;
pub mod coaching;
pub mod coaching_profile;
pub mod eligibility;
pub mod hero;
pub mod r#match;
pub mod metrics;
pub mod player;
pub mod player_model;
pub mod role;
pub mod scope;
pub mod session;
pub mod training;
pub mod user;
