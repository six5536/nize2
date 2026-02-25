// @awa-component: MCP-DummyData
//
//! Hardcoded dummy data for MCP meta-tool stub responses.
//!
//! Centralised here so that replacing stubs with real service calls
//! only requires changing this module.

use std::collections::HashMap;

use crate::tools::types::{
    DiscoveredTool, DiscoveryResult, McpToolManifest, ServerInfo, ToolDomain, ToolField,
};

/// Dummy servers used across discovery responses.
fn dummy_servers() -> HashMap<String, ServerInfo> {
    let mut servers = HashMap::new();
    servers.insert(
        "srv-filesystem".to_string(),
        ServerInfo {
            id: "srv-filesystem".to_string(),
            name: "Filesystem Server".to_string(),
            description: "Tools for reading, writing, and managing files".to_string(),
        },
    );
    servers.insert(
        "srv-database".to_string(),
        ServerInfo {
            id: "srv-database".to_string(),
            name: "Database Server".to_string(),
            description: "Tools for querying and managing databases".to_string(),
        },
    );
    servers
}

/// Dummy tool catalogue.
fn dummy_tools() -> Vec<DiscoveredTool> {
    vec![
        DiscoveredTool {
            id: "tool-read-file".to_string(),
            name: "read_file".to_string(),
            description: "Read contents of a file at a given path".to_string(),
            domain: "filesystem".to_string(),
            server_id: "srv-filesystem".to_string(),
            score: 0.95,
        },
        DiscoveredTool {
            id: "tool-write-file".to_string(),
            name: "write_file".to_string(),
            description: "Write content to a file at a given path".to_string(),
            domain: "filesystem".to_string(),
            server_id: "srv-filesystem".to_string(),
            score: 0.90,
        },
        DiscoveredTool {
            id: "tool-query-db".to_string(),
            name: "query_database".to_string(),
            description: "Execute a SQL query against a connected database".to_string(),
            domain: "database".to_string(),
            server_id: "srv-database".to_string(),
            score: 0.88,
        },
    ]
}

/// Return a dummy `DiscoveryResult`, optionally filtered by domain.
pub fn discover_tools(query: &str, domain: Option<&str>) -> DiscoveryResult {
    let all_tools = dummy_tools();
    let tools: Vec<DiscoveredTool> = match domain {
        Some(d) => all_tools.into_iter().filter(|t| t.domain == d).collect(),
        None => all_tools,
    };

    let has_results = !tools.is_empty();
    let servers = if has_results {
        let ids: Vec<String> = tools.iter().map(|t| t.server_id.clone()).collect();
        dummy_servers()
            .into_iter()
            .filter(|(k, _)| ids.contains(k))
            .collect()
    } else {
        HashMap::new()
    };

    let suggestion = if !has_results {
        Some(format!(
            "No tools matched query \"{query}\". Try broader terms or list domains first."
        ))
    } else {
        None
    };

    DiscoveryResult {
        tools,
        servers,
        suggestion,
    }
}

/// Return a dummy tool manifest for `get_tool_schema`.
pub fn get_tool_manifest(tool_id: &str) -> McpToolManifest {
    McpToolManifest {
        id: tool_id.to_string(),
        server_id: "srv-filesystem".to_string(),
        name: "read_file".to_string(),
        description: "Read contents of a file at a given path".to_string(),
        domain: "filesystem".to_string(),
        inputs: vec![ToolField {
            name: "path".to_string(),
            field_type: "string".to_string(),
            required: true,
            description: Some("Absolute or relative file path to read".to_string()),
        }],
        outputs: vec![ToolField {
            name: "content".to_string(),
            field_type: "string".to_string(),
            required: true,
            description: Some("File content as UTF-8 text".to_string()),
        }],
        preconditions: vec!["File must exist and be readable".to_string()],
        postconditions: vec!["File content is returned unchanged".to_string()],
        side_effects: vec![],
    }
}

/// Return the list of available tool domains.
pub fn list_tool_domains() -> Vec<ToolDomain> {
    vec![
        ToolDomain {
            id: "filesystem".to_string(),
            name: "Filesystem".to_string(),
            description: "Tools for reading, writing, and managing files".to_string(),
            tool_count: 2,
        },
        ToolDomain {
            id: "database".to_string(),
            name: "Database".to_string(),
            description: "Tools for querying and managing databases".to_string(),
            tool_count: 1,
        },
    ]
}

/// Return tools in a specific domain.
pub fn browse_tool_domain(domain_id: &str) -> DiscoveryResult {
    discover_tools("", Some(domain_id))
}
