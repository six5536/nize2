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
    let spec = serde_json::from_reader(file)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", src.display()));

    let mut generator = progenitor::Generator::default();

    let tokens = generator.generate_tokens(&spec).unwrap();
    let ast = syn::parse2(tokens).unwrap();
    let content = prettyplease::unparse(&ast);

    let mut out_file = Path::new(&env::var("OUT_DIR").unwrap()).to_path_buf();
    out_file.push("codegen.rs");

    fs::write(out_file, content).unwrap();
}
