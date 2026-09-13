//! AI Dota Coach backend.
//!
//! Exposed as a library so integration tests can build the real router with
//! stubbed providers, rather than testing a second, parallel wiring.

pub mod api;
pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod repositories;
pub mod services;
pub mod state;
