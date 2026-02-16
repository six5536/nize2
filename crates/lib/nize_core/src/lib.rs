//! # nize_core
//!
//! Core domain logic for Nize.

pub mod auth;
pub mod bun_sidecar;
pub mod config;
pub mod db;
pub mod embedding;
pub mod hello;
pub mod mcp;
pub mod migrate;
pub mod models;

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}
