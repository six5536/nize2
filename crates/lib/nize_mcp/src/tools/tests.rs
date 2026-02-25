// @awa-component: MCP-MetaToolTests
//
//! Tests for MCP meta-tool discovery stubs.

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use crate::server::NizeMcpServer;
    use crate::tools::dummy;
    use crate::tools::types::ExecutionResult;

    // @awa-test: MCP-1_AC-1
    #[test]
    fn server_exposes_six_tools() {
        let tools = NizeMcpServer::list_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(tools.len(), 6, "Expected 6 tools, got: {names:?}");
        assert!(names.contains(&"hello"));
        assert!(names.contains(&"discover_tools"));
        assert!(names.contains(&"get_tool_schema"));
        assert!(names.contains(&"execute_tool"));
        assert!(names.contains(&"list_tool_domains"));
        assert!(names.contains(&"browse_tool_domain"));
    }

    // @awa-test: MCP-1.1_AC-1
    #[test]
    fn discover_tools_returns_ranked_matches() {
        let result = dummy::discover_tools("read file", None);
        assert!(!result.tools.is_empty(), "Expected non-empty tool list");
        assert!(!result.servers.is_empty(), "Expected non-empty servers map");
        assert!(result.suggestion.is_none());

        // Verify JSON serialization round-trips
        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: Value = serde_json::from_str(&json).expect("parse");
        assert!(parsed["tools"].is_array());
        assert!(parsed["servers"].is_object());

        // Verify tool fields
        let first = &parsed["tools"][0];
        assert!(first["id"].is_string());
        assert!(first["name"].is_string());
        assert!(first["description"].is_string());
        assert!(first["domain"].is_string());
        assert!(first["serverId"].is_string());
        assert!(first["score"].is_f64());
    }

    // @awa-test: MCP-1.1_AC-2
    #[test]
    fn discover_tools_filters_by_domain() {
        let result = dummy::discover_tools("query", Some("filesystem"));
        assert!(
            result.tools.iter().all(|t| t.domain == "filesystem"),
            "All tools should be in filesystem domain"
        );
        assert_eq!(result.tools.len(), 2);
    }

    // @awa-test: MCP-1.1_AC-3
    #[test]
    fn discover_tools_unknown_domain_returns_suggestion() {
        let result = dummy::discover_tools("query", Some("nonexistent"));
        assert!(result.tools.is_empty());
        assert!(result.suggestion.is_some());
    }

    // @awa-test: MCP-1.4_AC-1
    #[test]
    fn list_tool_domains_returns_domains() {
        let domains = dummy::list_tool_domains();
        assert!(!domains.is_empty(), "Expected non-empty domain list");

        let json = serde_json::to_string(&domains).expect("serialize");
        let parsed: Value = serde_json::from_str(&json).expect("parse");
        let arr = parsed.as_array().expect("should be array");

        for domain in arr {
            assert!(domain["id"].is_string());
            assert!(domain["name"].is_string());
            assert!(domain["description"].is_string());
            assert!(domain["toolCount"].is_u64());
        }
    }

    // @awa-test: MCP-1.5_AC-1
    #[test]
    fn browse_tool_domain_returns_tools_for_known_domain() {
        let result = dummy::browse_tool_domain("filesystem");
        assert!(!result.tools.is_empty());
        assert!(result.tools.iter().all(|t| t.domain == "filesystem"));
    }

    // @awa-test: MCP-1.5_AC-2
    #[test]
    fn browse_tool_domain_empty_for_unknown_domain() {
        let result = dummy::browse_tool_domain("nonexistent");
        assert!(result.tools.is_empty());
    }

    // @awa-test: MCP-1.2_AC-1
    #[test]
    fn get_tool_manifest_returns_manifest_structure() {
        let manifest = dummy::get_tool_manifest("tool-read-file");
        let json = serde_json::to_string(&manifest).expect("serialize");
        let parsed: Value = serde_json::from_str(&json).expect("parse");

        assert!(parsed["id"].is_string());
        assert!(parsed["serverId"].is_string());
        assert!(parsed["name"].is_string());
        assert!(parsed["description"].is_string());
        assert!(parsed["domain"].is_string());
        assert!(parsed["inputs"].is_array());
        assert!(parsed["outputs"].is_array());
        assert!(parsed["preconditions"].is_array());
        assert!(parsed["postconditions"].is_array());
        assert!(parsed["sideEffects"].is_array());

        // Check input field shape
        let input = &parsed["inputs"][0];
        assert!(input["name"].is_string());
        assert!(input["type"].is_string());
        assert!(input["required"].is_boolean());
    }

    // @awa-test: MCP-1.3_AC-1
    #[test]
    fn execution_result_serialises_correctly() {
        let result = ExecutionResult {
            success: true,
            tool_name: "test_tool".to_string(),
            result: serde_json::json!({"key": "value"}),
        };
        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: Value = serde_json::from_str(&json).expect("parse");

        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["toolName"], "test_tool");
        assert_eq!(parsed["result"]["key"], "value");
    }
}
