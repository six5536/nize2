//! Hello world function for Nize.

/// Returns a greeting string with the crate version.
pub fn hello_world() -> String {
    format!("Hello from nize_core v{}", super::version())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_world_contains_version() {
        let greeting = hello_world();
        assert!(greeting.starts_with("Hello from nize_core v"));
        assert!(greeting.contains(env!("CARGO_PKG_VERSION")));
    }
}
