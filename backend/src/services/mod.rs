//! Business logic, organized as independent services composed in `AppState`.

pub mod admin;
pub mod audit;
pub mod auth;
pub mod benchmarks;
pub mod billing;
pub mod cache;
pub mod calibration;
pub mod coaching;
pub mod coaching_session;
pub mod dota;
pub mod events;
pub mod hero_meta;
pub mod heroes;
pub mod llm;
pub mod match_comparison;
pub mod metrics;
pub mod payments;
pub mod player_model;
pub mod progress;
pub mod roles;
pub mod sync;
pub mod training;
pub mod voucher;
