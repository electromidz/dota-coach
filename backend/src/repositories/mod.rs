//! Database access. One module per aggregate, returning domain models.
//!
//! Queries are runtime-checked (`sqlx::query_as`) rather than macro-checked so
//! the crate builds without a live database (Docker image builds offline).

pub mod benchmark;
pub mod billing;
pub mod coaching;
pub mod coaching_profile;
pub mod dota_player;
pub mod hero_meta;
pub mod hero_pool;
pub mod r#match;
pub mod metrics;
pub mod player_model;
pub mod session;
pub mod training;
pub mod user;
