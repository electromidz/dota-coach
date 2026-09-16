//! Business logic, organized as independent services composed in `AppState`.

pub mod auth;
pub mod benchmarks;
pub mod billing;
pub mod coaching;
pub mod dota;
pub mod hero_meta;
pub mod heroes;
pub mod llm;
pub mod metrics;
pub mod payments;
pub mod player_model;
pub mod roles;
pub mod sync;
pub mod training;
