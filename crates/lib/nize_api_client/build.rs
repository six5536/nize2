use std::{env, fs, path::Path};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = Path::new(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let src = workspace_root.join("codegen/nize-api/tsp-output/openapi.json");

    println!("cargo:rerun-if-changed={}", src.display());

    let file =
        fs::File::open(&src).unwrap_or_else(|e| panic!("failed to open {}: {e}", src.display()));
    let mut json: serde_json::Value = serde_json::from_reader(file)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", src.display()));

    // Progenitor requires at most one success (2xx) response per operation.
    // Our TypeSpec defines proper error responses (4xx/5xx) via @error models
    // which produce multiple response entries. Strip non-2xx responses so
    // progenitor only sees the success path; errors are handled at runtime.
    strip_error_responses(&mut json);

    let spec: openapiv3::OpenAPI = serde_json::from_value(json)
        .unwrap_or_else(|e| panic!("failed to deserialize OpenAPI spec: {e}"));

    let mut generator = progenitor::Generator::default();

    let tokens = generator.generate_tokens(&spec).unwrap();
    let ast = syn::parse2(tokens).unwrap();
    let content = prettyplease::unparse(&ast);

    let mut out_file = Path::new(&env::var("OUT_DIR").unwrap()).to_path_buf();
    out_file.push("codegen.rs");

    fs::write(out_file, content).unwrap();
}

/// Remove non-2xx responses from every operation in the OpenAPI spec.
/// Progenitor panics when it encounters multiple response types; this
/// pre-processing keeps the TypeSpec contract accurate while satisfying
/// the generator's single-success-response constraint.
fn strip_error_responses(spec: &mut serde_json::Value) {
    let paths = match spec.get_mut("paths").and_then(|p| p.as_object_mut()) {
        Some(p) => p,
        None => return,
    };

    let methods = ["get", "post", "put", "patch", "delete"];

    for (_path, path_item) in paths.iter_mut() {
        let path_obj = match path_item.as_object_mut() {
            Some(o) => o,
            None => continue,
        };
        for method in &methods {
            let op = match path_obj.get_mut(*method).and_then(|o| o.as_object_mut()) {
                Some(o) => o,
                None => continue,
            };
            let responses = match op.get_mut("responses").and_then(|r| r.as_object_mut()) {
                Some(r) => r,
                None => continue,
            };
            let error_codes: Vec<String> = responses
                .keys()
                .filter(|code| !code.starts_with('2'))
                .cloned()
                .collect();
            for code in error_codes {
                responses.remove(&code);
            }
        }
    }
}
