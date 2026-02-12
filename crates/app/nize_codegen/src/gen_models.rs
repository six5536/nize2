//! Generates `models.rs` — serde model structs from OpenAPI schemas.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::schema::{PropertyObject, SchemaObject};
use crate::writer::escape_rust_str;

/// Map an OpenAPI type + nullable to a Rust type string.
fn rust_type(prop: &PropertyObject, required: bool) -> String {
    let base = match prop.prop_type.as_deref() {
        Some("string") => "String",
        Some("boolean") => "bool",
        Some("integer") => "i64",
        Some("number") => "f64",
        Some("array") => "Vec<serde_json::Value>",
        _ => "serde_json::Value",
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
    // Doc comment
    if let Some(desc) = &schema.description {
        for line in desc.lines() {
            writeln!(out, "/// {}", escape_rust_str(line)).unwrap();
        }
    }

    writeln!(out, "#[derive(Debug, Clone, Serialize, Deserialize)]").unwrap();
    writeln!(out, "pub struct {name} {{").unwrap();

    for (field_name, prop) in &schema.properties {
        let snake = to_snake_case(field_name);
        let required = schema.required.contains(field_name);
        let ty = rust_type(prop, required);

        // Doc comment for field
        if let Some(desc) = &prop.description {
            writeln!(out, "    /// {}", escape_rust_str(desc)).unwrap();
        }

        // Rename attribute if snake_case differs from original
        if snake != *field_name {
            writeln!(out, "    #[serde(rename = \"{field_name}\")]").unwrap();
        }

        writeln!(out, "    pub {snake}: {ty},").unwrap();
    }

    writeln!(out, "}}").unwrap();
}
