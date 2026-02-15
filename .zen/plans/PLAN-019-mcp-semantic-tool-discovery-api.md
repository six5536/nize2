# PLAN-019: MCP Semantic Tool Discovery API (Stub)

**Status:** in-progress
**Workflow direction:** lateral
**Traceability:** Reference project `submodules/nize/` — REQ-MCP-semantic-tool-discovery.md, DESIGN-MCP-semantic-tool-discovery.md

## Goal

Expose the 5 semantic tool discovery meta-tools as MCP tools in the `nize_mcp` crate (Rust/rmcp), replacing the reference project's internal TypeScript/Vercel AI SDK implementation. All 5 tools return hardcoded dummy responses — real business logic (pgvector search, MCP proxy execution, caching) is out of scope.

## Context

In the reference project (`submodules/nize/`), semantic tool discovery is implemented as internal tools within the `packages/agent` TypeScript package, invoked by the Vercel AI SDK orchestrator. In nize-mcp, these same capabilities must be exposed as first-class MCP tools via rmcp's `#[tool]` macro, callable by any MCP client (Claude Desktop, GitHub Copilot, etc.).

The existing `nize_mcp` crate already has:
- `NizeMcpServer` struct with `PgPool` and `ToolRouter` (rmcp `#[tool_router]`)
- Bearer token auth middleware
- A single `hello` tool as pattern reference
- Tool parameter types via `schemars::JsonSchema` + `serde::Deserialize`

## Scope

**In-scope:**
- 5 MCP tool definitions in `nize_mcp` with rmcp `#[tool]` attributes
- Parameter types (`JsonSchema` + `Deserialize`) for each tool
- Response types (`Serialize`) for each tool
- Hardcoded dummy responses demonstrating correct response shapes
- Unit tests verifying tool count and response structure
- Wiring into existing `NizeMcpServer` tool router

**Out-of-scope:**
- Real semantic search (pgvector, embeddings)
- Real MCP server registry / tool indexing
- Real tool execution proxy
- Per-user preferences / access control beyond existing auth middleware
- Discovery caching
- Database migrations or schema changes
- REST API endpoints (these are MCP-only tools)

## Meta-Tools to Implement

### 1. `discover_tools`

Search for tools by natural language query. Returns ranked tool matches with server context.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | `String` | yes | Natural language description of desired capability |
| `domain` | `Option<String>` | no | Optional domain to filter results |

**Dummy response:** 2–3 hardcoded `DiscoveredTool` entries with a `servers` map. If `domain` is provided, filter the dummy data by domain. If no match, include a `suggestion` field.

### 2. `get_tool_schema`

Get detailed parameter schema for a specific tool.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `tool_id` | `String` | yes | Tool ID from discovery results |

**Dummy response:** A hardcoded `McpToolManifest` with inputs, outputs, preconditions, postconditions, side_effects.

### 3. `execute_tool`

Run a discovered tool with parameters. Returns execution result.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `tool_id` | `String` | yes | Tool ID to execute |
| `tool_name` | `String` | yes | Human-readable tool name for display |
| `params` | `serde_json::Value` | yes | Parameters matching tool schema |

**Dummy response:** A success result with a static JSON value and the tool_name echoed back.

### 4. `list_tool_domains`

List available tool categories. No parameters.

**Dummy response:** 2–3 hardcoded `ToolDomain` entries with id, name, description, tool_count.

### 5. `browse_tool_domain`

List all tools in a domain.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `domain_id` | `String` | yes | Domain ID from list_tool_domains |

**Dummy response:** Hardcoded tools filtered by domain_id, with a `servers` map.

## Response Types

Aligned with the reference project's TypeScript types, translated to Rust:

```rust
// Shared across discovery responses
struct DiscoveredTool {
    id: String,
    name: String,
    description: String,
    domain: String,
    server_id: String,
    server_name: String,
    score: f64,
}

struct ServerInfo {
    id: String,
    name: String,
    description: String,
}

struct ToolDomain {
    id: String,
    name: String,
    description: String,
    tool_count: u32,
}

// Tool manifest for get_tool_schema
struct McpToolManifest {
    id: String,
    server_id: String,
    name: String,
    description: String,
    domain: String,
    inputs: Vec<ToolField>,
    outputs: Vec<ToolField>,
    preconditions: Vec<String>,
    postconditions: Vec<String>,
    side_effects: Vec<String>,
}

struct ToolField {
    name: String,
    field_type: String,  // "type" is reserved in Rust
    required: bool,
    description: Option<String>,
}
```

## Implementation Steps

### Step 1: Create Type Definitions

Create `crates/lib/nize_mcp/src/tools/types.rs`:
- Define all shared response structs with `#[derive(Serialize, Clone)]`
- These types are serialized to JSON in `CallToolResult::success` content

### Step 2: Create Dummy Data Module

Create `crates/lib/nize_mcp/src/tools/dummy.rs`:
- Provide functions returning hardcoded `DiscoveredTool`, `ToolDomain`, `ServerInfo`, `McpToolManifest` data
- Centralizes dummy data for easy replacement with real service calls later

### Step 3: Create Discovery Tool Module

Create `crates/lib/nize_mcp/src/tools/discovery.rs`:
- Define `DiscoverToolsRequest`, `GetToolSchemaRequest`, `BrowseToolDomainRequest` parameter types (with `JsonSchema` + `Deserialize`)
- No `ListToolDomainsRequest` needed (no parameters)

### Step 4: Add Tools to NizeMcpServer

Update `crates/lib/nize_mcp/src/server.rs`:
- Add 5 new `#[tool]` methods to the `#[tool_router] impl NizeMcpServer`:
  - `discover_tools` — calls dummy data, serializes to JSON, returns as `Content::text`
  - `get_tool_schema` — returns hardcoded manifest
  - `execute_tool` — returns hardcoded success result
  - `list_tool_domains` — returns hardcoded domains
  - `browse_tool_domain` — returns hardcoded tools filtered by domain_id

### Step 5: Update Module Declarations

Update `crates/lib/nize_mcp/src/tools/mod.rs`:
- Add `pub mod types;`, `pub mod dummy;`, `pub mod discovery;`

### Step 6: Tests

Add tests in `crates/lib/nize_mcp/src/tools/discovery.rs` or a separate test file:
- Verify the server exposes exactly 6 tools (hello + 5 meta-tools)
- Verify `discover_tools` returns valid JSON with expected fields
- Verify `list_tool_domains` returns non-empty domain list
- Verify `browse_tool_domain` with known domain returns tools
- Verify `get_tool_schema` returns manifest structure
- Verify `execute_tool` returns success result

### Step 7: Build & Verify

1. `cargo build` — compilation succeeds
2. `cargo clippy` — no warnings
3. `cargo test -p nize_mcp` — all tests pass

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| Return JSON as `Content::text` | rmcp `CallToolResult` supports text content; JSON is the natural format for structured tool responses. Tools format their own response text. |
| Rust naming (`snake_case` fields) vs TypeScript (`camelCase`) | Use Rust convention (`snake_case`) with `#[serde(rename_all = "camelCase")]` on structs for JSON output parity with reference project |
| Dummy data in separate module | Clean separation; replace `dummy.rs` calls with real service calls later without touching tool handler code |
| No `ExecuteToolRequest` param struct | `execute_tool` needs `serde_json::Value` for `params`; define inline or in discovery module |
| No REST API exposure | These are MCP-only tools; the REST API already has `/mcp/servers/*` routes for management |

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| rmcp `#[tool]` macro limitations for complex return types | Return serialized JSON string via `Content::text`; avoid custom `IntoContents` impls |
| `serde_json::Value` in tool params may not generate useful JSON schema | Use `schemars::JsonSchema` derive; if schema is too loose, add description annotations |
| Tool count growth may affect MCP client UX | 6 tools total is well within reasonable limits; meta-tool pattern keeps it bounded |

## Resolved Questions

1. **`execute_tool` `params` type:** Use `serde_json::Value` (fully dynamic). Real params depend on the target tool schema; `Value` is appropriate for the stub and likely for the real implementation too.

2. **Response format:** Serialize responses as JSON strings via `Content::text(serde_json::to_string_pretty(&response))`. Structured JSON is the expected format for programmatic consumption by MCP clients.

## Completion Criteria

- 5 new MCP tools registered in `NizeMcpServer` (6 total with `hello`)
- Each tool returns well-formed dummy JSON matching reference project response shapes
- `cargo build`, `cargo clippy`, `cargo test -p nize_mcp` all pass
- Tools are accessible via MCP client (e.g., Claude Desktop) when `nize_desktop_server` runs

## Dependencies

- Existing: `nize_mcp` crate with rmcp tool infrastructure
- Existing: `serde`, `serde_json`, `schemars` dependencies (already in Cargo.toml)
