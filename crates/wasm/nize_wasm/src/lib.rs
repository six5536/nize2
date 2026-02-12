use wasm_bindgen::prelude::*;

/// Returns the version of the nize-wasm package.
#[wasm_bindgen]
pub fn version() -> String {
    nize_core::version().to_string()
}
