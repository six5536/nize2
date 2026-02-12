//! Code generator CLI for Nize API models.
//!
//! Thin wrapper around [`nize_codegen::generate`].

use std::path::PathBuf;

/// Resolve the workspace root (directory containing top-level `Cargo.toml`).
fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let mut dir = PathBuf::from(manifest_dir);
    // Walk up from crates/app/nize_codegen to workspace root
    while !dir.join("Cargo.toml").exists()
        || !dir.join("crates").exists()
    {
        if !dir.pop() {
            panic!("Cannot find workspace root");
        }
    }
    dir
}

fn main() {
    let root = workspace_root();
    let spec_path = root
        .join("codegen")
        .join("nize-api")
        .join("tsp-output")
        .join("@typespec")
        .join("openapi3")
        .join("openapi.yaml");
    let output_dir = root
        .join("crates")
        .join("lib")
        .join("nize_api")
        .join("src")
        .join("generated");

    println!("Reading: {}", spec_path.display());
    println!("Output:  {}", output_dir.display());

    match nize_codegen::generate(&spec_path, &output_dir) {
        Ok(true) => println!("Done — generated files written."),
        Ok(false) => println!("Done — already up-to-date."),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
