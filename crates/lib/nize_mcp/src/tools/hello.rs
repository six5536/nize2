//! Hello world tool — parameter types.

use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for the `hello` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HelloRequest {
    /// Name to greet. Defaults to "world" if omitted.
    pub name: Option<String>,
}
