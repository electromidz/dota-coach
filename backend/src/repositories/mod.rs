//! Database access. One module per aggregate, returning domain models.
//!
//! Queries are runtime-checked (`sqlx::query_as`) rather than macro-checked so
//! the crate builds without a live database (Docker image builds offline).

pub mod dota_player;
pub mod r#match;
pub mod session;
pub mod user;
