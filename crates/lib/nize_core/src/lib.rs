//! # nize_core
//!
//! Core domain logic for Nize.

pub mod auth;
pub mod config;
pub mod db;
pub mod hello;
pub mod migrate;
pub mod models;
pub mod node_sidecar;

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
