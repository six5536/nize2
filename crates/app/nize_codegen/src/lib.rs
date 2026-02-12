//! Code generator library for Nize API models and route constants.
//!
//! Reads an OpenAPI 3.0 YAML file (produced by TypeSpec) and emits
//! Rust source files into `crates/lib/nize_api/src/generated/`.

mod gen_models;
mod gen_routes;
mod schema;
mod writer;

use std::path::Path;

use schema::OpenApiDoc;

/// Generate all Rust source files from an OpenAPI YAML spec.
///
/// * `spec_path`  — path to the OpenAPI YAML file
/// * `output_dir` — path to `crates/lib/nize_api/src/generated/`
///
/// Returns `Ok(true)` if files were generated (stale), `Ok(false)` if
/// already up-to-date.
pub fn generate(spec_path: &Path, output_dir: &Path) -> Result<bool, String> {
    let hash_path = output_dir.join(".hash");

    // Read the spec source
    let yaml_str = std::fs::read_to_string(spec_path)
        .map_err(|e| format!("Failed to read {}: {e}", spec_path.display()))?;

    // Check staleness
    if is_up_to_date(&yaml_str, &hash_path) {
        return Ok(false);
    }

    // Parse
    let doc: OpenApiDoc = serde_yaml::from_str(&yaml_str)
        .map_err(|e| format!("Failed to parse OpenAPI YAML: {e}"))?;

    // Create output directory
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create {}: {e}", output_dir.display()))?;

    // Generate model structs
    generate_file(output_dir, "models.rs", &gen_models::generate(&doc.components.schemas))?;

    // Generate route constants
    generate_file(output_dir, "routes.rs", &gen_routes::generate(&doc.paths))?;

    // Generate mod.rs re-exports
    let mod_rs = "\
pub mod models;
pub mod routes;
";
    generate_file(output_dir, "mod.rs", mod_rs)?;

    // Write hash file for staleness check
    let hash = compute_hash(&yaml_str);
    std::fs::write(&hash_path, &hash)
        .map_err(|e| format!("Failed to write {}: {e}", hash_path.display()))?;

    Ok(true)
}

fn generate_file(output_dir: &Path, filename: &str, content: &str) -> Result<(), String> {
    let path = output_dir.join(filename);
    writer::write_generated_file(&path, content)
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

/// Check if the generated code is up-to-date by comparing hashes.
fn is_up_to_date(yaml_str: &str, hash_path: &Path) -> bool {
    let stored_hash = match std::fs::read_to_string(hash_path) {
        Ok(h) => h,
        Err(_) => return false,
    };
    let current_hash = compute_hash(yaml_str);
    stored_hash.trim() == current_hash
}

/// Compute a hash of the input string.
///
/// Uses FNV-1a 128-bit — sufficient for staleness detection.
fn compute_hash(input: &str) -> String {
    let mut h: u128 = 0x6c62272e07bb0142_62b821756295c58d;
    for b in input.bytes() {
        h ^= b as u128;
        h = h.wrapping_mul(0x0000000001000000_000000000000013B);
    }
    format!("{h:032x}")
}
