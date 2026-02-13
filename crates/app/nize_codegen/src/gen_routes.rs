//! Generates `routes.rs` — route path constants from OpenAPI paths.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::schema::PathItem;
use crate::writer::escape_rust_str;

/// Generate the contents of `routes.rs`.
pub fn generate(paths: &BTreeMap<String, PathItem>) -> String {
    let mut out = String::new();

    out.push_str("//! Route path constants extracted from the OpenAPI specification.\n\n");

    for (path, item) in paths {
        let methods = collect_methods(item);
        for method in &methods {
            let const_name = route_const_name(method, path);
            let desc = method_description(item, method);
            if let Some(desc) = desc {
                // Use first line only for the doc comment to avoid broken Rust syntax.
                let first_line = desc.lines().next().unwrap_or(&desc);
                writeln!(out, "/// {method} {path} — {}", escape_rust_str(first_line)).unwrap();
            } else {
                writeln!(out, "/// {method} {path}").unwrap();
            }
            writeln!(out, "pub const {const_name}: &str = \"{path}\";").unwrap();
            out.push('\n');
        }
    }

    out
}

/// Collect HTTP methods defined on a path item.
fn collect_methods(item: &PathItem) -> Vec<&'static str> {
    let mut methods = Vec::new();
    if item.get.is_some() {
        methods.push("GET");
    }
    if item.post.is_some() {
        methods.push("POST");
    }
    if item.put.is_some() {
        methods.push("PUT");
    }
    if item.delete.is_some() {
        methods.push("DELETE");
    }
    if item.patch.is_some() {
        methods.push("PATCH");
    }
    methods
}

/// Build a SCREAMING_SNAKE constant name from method + path.
/// e.g. GET /api/hello → GET_API_HELLO
fn route_const_name(method: &str, path: &str) -> String {
    let path_part: String = path
        .trim_start_matches('/')
        .replace('/', "_")
        .replace('-', "_")
        .replace('{', "")
        .replace('}', "")
        .to_uppercase();
    format!("{method}_{path_part}")
}

/// Get the description from the operation for a given method.
fn method_description(item: &PathItem, method: &str) -> Option<String> {
    let op = match method {
        "GET" => item.get.as_ref(),
        "POST" => item.post.as_ref(),
        "PUT" => item.put.as_ref(),
        "DELETE" => item.delete.as_ref(),
        "PATCH" => item.patch.as_ref(),
        _ => None,
    };
    op.and_then(|o| o.description.clone())
}
