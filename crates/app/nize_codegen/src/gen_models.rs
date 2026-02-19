//! Generates `models.rs` — serde model structs from OpenAPI schemas.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::schema::{PropertyObject, SchemaObject};
use crate::writer::escape_rust_str;

/// Map an OpenAPI type + nullable to a Rust type string.
fn rust_type(prop: &PropertyObject, required: bool) -> String {
    // Handle $ref to another schema
    if let Some(ref_path) = &prop.ref_path {
        let ref_name = ref_path.rsplit('/').next().unwrap_or(ref_path);
        // Strip namespace prefix (e.g. "Auth.AuthUser" → "AuthUser")
        let struct_name = match ref_name.rsplit_once('.') {
            Some((_, suffix)) => suffix,
            None => ref_name,
        };
        if !required {
            return format!("Option<{struct_name}>");
        }
        return struct_name.to_string();
    }

    // Handle allOf (resolve first $ref found)
    if !prop.all_of.is_empty() {
        for item in &prop.all_of {
            if item.ref_path.is_some() {
                return rust_type(item, required);
            }
        }
    }

    let base = match prop.prop_type.as_deref() {
        Some("string") => "String".to_string(),
        Some("boolean") => "bool".to_string(),
        Some("integer") => "i64".to_string(),
        Some("number") => "f64".to_string(),
        Some("array") => {
            let item_type = prop
                .items
                .as_ref()
                .map(|items| rust_type(items, true))
                .unwrap_or_else(|| "serde_json::Value".to_string());
            format!("Vec<{item_type}>")
        }
        _ => "serde_json::Value".to_string(),
    };

    if prop.nullable {
        format!("Option<{base}>")
    } else if !required {
        format!("Option<{base}>")
    } else {
        base.to_string()
    }
}

/// Convert a camelCase field name to snake_case.
fn to_snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// Escape Rust keywords by prefixing with `r#`.
fn escape_rust_keyword(name: &str) -> String {
    const RUST_KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while", "yield",
    ];
    if RUST_KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

/// Generate the contents of `models.rs`.
pub fn generate(schemas: &BTreeMap<String, SchemaObject>) -> String {
    let mut out = String::new();

    out.push_str("use serde::{Deserialize, Serialize};\n\n");

    for (name, schema) in schemas {
        generate_struct(&mut out, name, schema);
        out.push('\n');
    }

    out
}

fn generate_struct(out: &mut String, name: &str, schema: &SchemaObject) {
    // Strip namespace prefix (e.g. "Auth.AuthStatusResponse" → "AuthStatusResponse")
    let struct_name = match name.rsplit_once('.') {
        Some((_, suffix)) => suffix,
        None => name,
    };

    // Doc comment
    if let Some(desc) = &schema.description {
        for line in desc.lines() {
            writeln!(out, "/// {}", escape_rust_str(line)).unwrap();
        }
    }

    writeln!(out, "#[derive(Debug, Clone, Serialize, Deserialize)]").unwrap();
    writeln!(out, "pub struct {struct_name} {{").unwrap();

    for (field_name, prop) in &schema.properties {
        let snake = to_snake_case(field_name);
        let required = schema.required.contains(field_name);
        let ty = rust_type(prop, required);

        // Doc comment for field
        if let Some(desc) = &prop.description {
            writeln!(out, "    /// {}", escape_rust_str(desc)).unwrap();
        }

        // Rename attribute if snake_case differs from original, or if it's a keyword
        let rust_field = escape_rust_keyword(&snake);
        if snake != *field_name || rust_field != snake {
            writeln!(out, "    #[serde(rename = \"{field_name}\")]").unwrap();
        }

        writeln!(out, "    pub {rust_field}: {ty},").unwrap();
    }

    writeln!(out, "}}").unwrap();
}
