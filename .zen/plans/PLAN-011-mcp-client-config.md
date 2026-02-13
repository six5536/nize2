# PLAN-011: MCP Client Configuration (One-Click Setup)

| Field              | Value                                                    |
|--------------------|----------------------------------------------------------|
| **Status**         | in-progress (Phases 1–3 complete)                        |
| **Workflow**       | top-down                                                 |
| **Reference**      | PLAN-009 (mcp-server), PLAN-010 (cloud-server-split)     |
| **Traceability**   | —                                                        |

## Goal

Enable one-click configuration of the Nize MCP server in popular AI clients, directly from the Nize Desktop UI. For each supported client:

1. Detect if the client is installed
2. Check if Nize is already configured
3. Write/update the client's config file with the correct connection details
4. Show feedback (configured / not installed / error)

## Supported Clients

| Client | Transport | Config Method | Config Path (macOS) |
|--------|-----------|---------------|---------------------|
| Claude Desktop | stdio (mcp-remote bridge) | JSON config file | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Claude Code | HTTP streamable (direct) | CLI command or `~/.claude.json` | `~/.claude.json` |
| GitHub Copilot (VS Code) | HTTP streamable (direct) | User-level `mcp.json` | `~/Library/Application Support/Code/User/mcp.json` |
| ChatGPT Desktop | HTTP streamable (remote) | UI-only (not automatable) | N/A |

### Claude Cowork

Claude Cowork is a feature within Claude Desktop (visible in `claude_desktop_config.json` as `coworkScheduledTasksEnabled`). MCP server configuration is shared with Claude Desktop — no separate setup needed.

### ChatGPT Desktop

ChatGPT configures MCP servers through its built-in Connections UI in settings. This cannot be automated via config file. The Nize Desktop UI should show instructions (URL + token to paste) rather than a "Configure" button.

## Decisions

### Transport Strategy

Clients supporting HTTP Streamable transport connect directly to `http://127.0.0.1:{mcp_port}/mcp` with a bearer token header. No bridge needed.

Clients that only support stdio (Claude Desktop) use `mcp-remote` as a bridge:
- `mcp-remote` converts stdio ↔ HTTP Streamable
- Invoked via the **sidecar node binary** (not system node or mise node)

### Sidecar Node for mcp-remote

The Nize Desktop app already bundles a sidecar `node` binary at `{exe_dir}/node` (resolved via `exe_dir.join("node")`). For stdio clients that need `mcp-remote`:

**Option A — Bundle mcp-remote as a resource** (like pglite-server.mjs):
- Pre-bundle `mcp-remote` into `resources/mcp-remote/mcp-remote.mjs` at build time
- Config: `"command": "{sidecar_node_path}", "args": ["{bundled_mcp_remote_path}", ...]`
- Pro: No network fetch at runtime, works offline, deterministic
- Con: Must update bundle when mcp-remote updates

**Option B — Use npx via sidecar node**:
- Ship `npx` alongside sidecar node (npx is a JS script from npm)
- Config: `"command": "{sidecar_node_path}", "args": ["{npx_path}", "-y", "mcp-remote", ...]`
- Pro: Always gets latest mcp-remote
- Con: Requires npm/npx alongside node, network fetch on first use

**Choice: Option A** — Bundle `mcp-remote` as a resource.

Rationale:
- Consistent with existing pattern (pglite-server.mjs is bundled the same way)
- No network dependency at config time
- Deterministic behavior
- The sidecar node is a standalone binary without npm/npx

### Config File Writing

Each client's config file has a different JSON structure. The Rust backend (Tauri commands) handles:
1. Reading the existing config file (preserve other servers)
2. Merging the Nize MCP entry
3. Writing back atomically (write to temp → rename)

### Token Management

Each client config stores the MCP bearer token. The flow:
1. User clicks "Configure" for a client
2. Backend checks if an MCP token exists for this client (by name)
3. If not, auto-generates a new token via `nize_core::auth::mcp_tokens::create_mcp_token()`
4. Writes the token into the client config
5. Shows success feedback

Each client gets its own named token (e.g. `claude-desktop`, `claude-code`, `copilot-vscode`) for independent revocation.

## Config Formats

### Claude Desktop (`claude_desktop_config.json`)

```json
{
  "mcpServers": {
    "nize": {
      "command": "/path/to/sidecar/node",
      "args": [
        "/path/to/bundled/mcp-remote.mjs",
        "http://127.0.0.1:{mcp_port}/mcp",
        "--allow-http",
        "--header",
        "Authorization:${AUTH_TOKEN}"
      ],
      "env": {
        "AUTH_TOKEN": "Bearer {token}"
      }
    }
  }
}
```

### Claude Code (`~/.claude.json`)

Added via `claude mcp add-json` or direct file edit:

```json
{
  "mcpServers": {
    "nize": {
      "type": "http",
      "url": "http://127.0.0.1:{mcp_port}/mcp",
      "headers": {
        "Authorization": "Bearer {token}"
      }
    }
  }
}
```

### GitHub Copilot / VS Code (`mcp.json`)

```json
{
  "servers": {
    "nize": {
      "type": "http",
      "url": "http://127.0.0.1:{mcp_port}/mcp",
      "headers": {
        "Authorization": "Bearer {token}"
      }
    }
  }
}
```

Note: VS Code uses `"servers"` (not `"mcpServers"`) as the top-level key.

### ChatGPT Desktop (manual)

User must open ChatGPT → Settings → Connections → Add MCP Server:
- URL: `http://127.0.0.1:{mcp_port}/mcp`
- Auth header: `Authorization: Bearer {token}`

## Current State

| Component | Status |
|-----------|--------|
| MCP server running on separate port | ✅ Done (PLAN-009) |
| MCP bearer token generation | ✅ Done (PLAN-009 Phase 4) |
| Token generation UI in desktop | ✅ Done (MainApp.tsx) |
| MCP endpoint URL display | ✅ Done (MainApp.tsx) |
| Sidecar node binary | ✅ Done (PLAN-007) |
| Bundled mcp-remote | ✅ Done (Phase 1) |
| Client config writing | ✅ Done (Phase 2) |
| One-click config UI | ✅ Done (Phase 3) |

## Plan

### Phase 1 — Bundle mcp-remote

Bundle `mcp-remote` as a single-file script in the app resources (same pattern as pglite-server.mjs).

- [x] **1.1** Add `mcp-remote` build script: `packages/nize-desktop/scripts/build-mcp-remote.mjs`
  - Uses esbuild to bundle `mcp-remote` into `crates/app/nize_desktop/resources/mcp-remote/mcp-remote.mjs`
  - Single-file output, no external dependencies at runtime
- [x] **1.2** Add npm script: `"build:mcp-remote": "node scripts/build-mcp-remote.mjs"` in `packages/nize-desktop/package.json`
- [x] **1.3** Add `mcp-remote` as a devDependency in `packages/nize-desktop/package.json`
- [x] **1.4** Update Tauri `beforeDevCommand` / `beforeBuildCommand` to run the mcp-remote build
- [x] **1.5** Verify: bundled `mcp-remote.mjs` exists after build, sidecar node can execute it

### Phase 2 — Rust Backend: Config Detection & Writing

Add Tauri commands for detecting, reading, and writing client configs.

- [x] **2.1** Create `crates/app/nize_desktop/src/mcp_clients.rs` module:
  - `enum McpClient { ClaudeDesktop, ClaudeCode, CopilotVscode, ChatGptDesktop }`
  - `struct McpClientStatus { client: McpClient, installed: bool, configured: bool, token_name: Option<String> }`
  - Platform-aware config path resolution for each client

- [x] **2.2** Implement detection functions:
  - `is_client_installed(client) → bool` — checks if config directory exists
  - `is_nize_configured(client) → bool` — reads config file, checks for `"nize"` entry

- [x] **2.3** Implement config writing functions:
  - `configure_claude_desktop(mcp_port, token, node_path, mcp_remote_path) → Result<()>`
    - Reads existing `claude_desktop_config.json` (or creates new)
    - Merges `mcpServers.nize` entry
    - Writes back atomically
  - `configure_claude_code(mcp_port, token) → Result<()>`
    - Reads existing `~/.claude.json` (or creates new)
    - Merges `mcpServers.nize` entry with HTTP transport
    - Writes back atomically
  - `configure_copilot_vscode(mcp_port, token) → Result<()>`
    - Reads existing user-level `mcp.json` (or creates new)
    - Merges `servers.nize` entry with HTTP transport
    - Writes back atomically

- [x] **2.4** Implement path resolution helpers:
  - `sidecar_node_path() → PathBuf` — resolves bundled node binary path
  - `bundled_mcp_remote_path() → PathBuf` — resolves bundled mcp-remote.mjs path

- [x] **2.5** Add Tauri commands:
  - `get_mcp_client_statuses() → Vec<McpClientStatus>` — returns status of all clients
  - `configure_mcp_client(client: McpClient) → Result<String>` — configures the client, auto-generates token, returns success message
  - `remove_mcp_client(client: McpClient) → Result<()>` — removes Nize entry from client config

- [x] **2.6** Register new commands in `run_tauri()` invoke_handler

### Phase 3 — React UI: MCP Client Settings

Replace the current manual token generation UI with a one-click client configuration panel.

- [x] **3.1** Create `packages/nize-desktop/src/settings/McpClientSettings.tsx`:
  - List of supported clients with icons/names
  - Each row shows: client name, status badge (Not Installed / Not Configured / Configured ✓), action button
  - "Configure" button for automatable clients (Claude Desktop, Claude Code, VS Code Copilot)
  - "Show Instructions" expandable for ChatGPT Desktop (displays URL + token)
  - Loading state during configuration
  - Error display on failure

- [x] **3.2** Create `packages/nize-desktop/src/settings/McpClientCard.tsx`:
  - Individual client card component
  - Status indicator (grey = not installed, yellow = installed but not configured, green = configured)
  - Configure / Reconfigure / Remove actions
  - Confirmation dialog for reconfigure (warns about token rotation)

- [x] **3.3** Update `MainApp.tsx`:
  - Replace manual MCP token section with `<McpClientSettings />` component
  - Keep MCP endpoint URL display (useful for manual config of other clients)

- [x] **3.4** Wire up Tauri command invocations:
  - `invoke<McpClientStatus[]>("get_mcp_client_statuses")` on mount
  - `invoke("configure_mcp_client", { client })` on button click
  - `invoke("remove_mcp_client", { client })` on remove
  - Refresh statuses after configure/remove

### Phase 4 — Cross-Platform Config Paths

Ensure config path resolution works on macOS, Linux, and Windows.

- [ ] **4.1** Add platform-specific config paths:

  | Client | macOS | Linux | Windows |
  |--------|-------|-------|---------|
  | Claude Desktop | `~/Library/Application Support/Claude/claude_desktop_config.json` | `~/.config/Claude/claude_desktop_config.json` | `%APPDATA%\Claude\claude_desktop_config.json` |
  | Claude Code | `~/.claude.json` | `~/.claude.json` | `%USERPROFILE%\.claude.json` |
  | VS Code Copilot | `~/Library/Application Support/Code/User/mcp.json` | `~/.config/Code/User/mcp.json` | `%APPDATA%\Code\User\mcp.json` |

- [ ] **4.2** Handle VS Code variants:
  - VS Code Insiders: `Code - Insiders` directory
  - VS Codium: `VSCodium` directory
  - Cursor: `Cursor` directory (uses same mcp.json format)

- [ ] **4.3** Add `#[cfg(target_os = ...)]` blocks or runtime detection for path resolution

### Phase 5 — Testing & Polish

- [ ] **5.1** Manual test: Configure Claude Desktop from UI → verify config written → verify MCP connection works
- [ ] **5.2** Manual test: Configure Claude Code from UI → verify `~/.claude.json` updated → verify `claude mcp list` shows nize
- [ ] **5.3** Manual test: Configure VS Code Copilot from UI → verify `mcp.json` written → verify MCP server appears in VS Code
- [ ] **5.4** Manual test: ChatGPT instructions display correctly with working URL and token
- [ ] **5.5** Edge cases:
  - Config file doesn't exist yet → create with correct structure
  - Config file has other servers → preserve them
  - Nize already configured → update token/port
  - Config file is malformed → show error, don't corrupt
  - Client not installed → grey out, show "Not Installed"

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| mcp-remote bundling may fail (complex deps) | High — Claude Desktop won't work | Test esbuild bundle early; fallback to npx if needed |
| Client config format changes | Medium — config may not be recognized | Pin to known-good format; version-check if possible |
| Config file permissions on different OS | Medium — write may fail | Use atomic write; handle permission errors gracefully |
| Claude Code `~/.claude.json` has complex structure | Medium — may break other settings | Read-merge-write carefully; only touch `mcpServers` key |
| VS Code has multiple variants (Insiders, Codium, Cursor) | Low — some variants missed | Start with standard VS Code; add variants in Phase 4 |
| MCP port changes on each restart (ephemeral) | High — configs become stale | Use fixed port (19560 default from NIZE_MCP_PORT) |
| ChatGPT may add config file support later | Low — plan already covers it | UI shows instructions; update when file config available |

## Resolved Questions

1. **Fixed MCP port**: ✅ Yes — keep fixed default (19560). Client configs hardcode this port. User can override via `NIZE_MCP_PORT` env var.

2. **Token per client**: ✅ Yes — each client gets its own named token (`nize-claude-desktop`, `nize-claude-code`, `nize-copilot-vscode`, `nize-chatgpt`) for independent revocation.

3. **Auto-reconfigure deferred**: ✅ Yes — deferred. Show the current port in the UI; manual reconfigure for now.

## Completion Criteria

- [x] `mcp-remote` bundled as a resource and invocable via sidecar node
- [x] Tauri commands for detect/configure/remove all 3 automatable clients
- [x] React UI with status indicators and one-click configuration
- [ ] Claude Desktop: one-click configure → MCP connection works
- [ ] Claude Code: one-click configure → `claude mcp list` shows nize
- [ ] VS Code Copilot: one-click configure → MCP server appears in VS Code
- [ ] ChatGPT Desktop: instructions displayed with correct URL + token
- [ ] Existing client configs are preserved (no data loss)
- [ ] Cross-platform paths resolved for macOS (Linux/Windows deferred if needed)
