# PLAN-029: Connect MCP Tools to nize-chat

**Status:** done
**Workflow direction:** lateral
**Traceability:** PLAN-024 (tool discovery & hooks); PLAN-027 (chat backend); PLAN-028 (API key management); ARCHITECTURE.md → nize_mcp, nize_api, nize-chat, nize_core

## Goal

Enable the chat interface (nize-chat) to use MCP meta-tools during conversations, allowing the LLM to discover and execute external MCP tools on behalf of the user. Currently `streamText()` is called without a `tools` parameter — the LLM can only generate text, not call tools.

**In scope:** `@ai-sdk/mcp` client connecting to existing MCP server, bearer token acquisition, `maxSteps` multi-turn tool loop, tool result rendering in frontend.
**Out of scope:** Individual tool exposure (meta-tools pattern preserved), tool policy/permissions beyond existing hook pipeline, new MCP tool types, new REST endpoints.

## Current State

| Component | Status |
|-----------|--------|
| MCP meta-tools (Rust, MCP protocol) | Done — `discover_tools`, `get_tool_schema`, `execute_tool`, `list_tool_domains`, `browse_tool_domain` in `nize_mcp::server` (PLAN-024) |
| MCP server (Streamable HTTP, bearer token auth) | Done — `http://127.0.0.1:{mcpPort}/mcp` (default port 19560) |
| MCP token creation API | Done — `POST /auth/mcp-tokens` (JWT cookie auth) |
| nize-chat `processChat()` | Done — calls `streamText()` with **no tools**, no `maxSteps` (PLAN-027) |
| Hook pipeline (before/after tool calls) | Done (PLAN-024) |
| AI proxy (key injection) | Done (PLAN-028) |
| Tool discovery via pgvector embeddings | Done (PLAN-022/024) |
| Execution proxy (HTTP + stdio MCP servers) | Done (PLAN-024/025) |
| `@ai-sdk/mcp` in nize-chat | **Not done** |
| Frontend tool result rendering | **Not done** |

## Architecture Decisiong

| Considered | Decision | Rationale |
|---|---|---|
| (A) `@ai-sdk/mcp` client → existing MCP server | **Selected** | Zero new Rust code; MCP server already has all 5 meta-tools with hooks, auth, audit; `@ai-sdk/mcp` converts MCP tools to AI SDK `tools` automatically; bearer token obtained via existing `POST /auth/mcp-tokens` REST endpoint |
| (B) New REST API endpoints duplicating meta-tool logic | Rejected | Duplicates `nize_mcp::server` into `nize_api`; two code paths to maintain; hook pipeline already runs in MCP server |
| (C) Embed tool logic directly in nize-chat (TypeScript) | Rejected | Duplicates Rust implementation; bypasses hook pipeline; inconsistent with meta-tools architecture |

### Auth Bridge: JWT Cookie → MCP Bearer Token

nize-chat has the user's JWT cookie (forwarded from the browser). The MCP server requires `Authorization: Bearer <token>`. Bridge:

1. On each chat request, nize-chat calls `POST /auth/mcp-tokens` with `{ name: "nize-desktop-chat", overwrite: true }` (REST API, JWT cookie auth)
2. The API atomically revokes any existing token with that name for the user and creates a new one — exactly 1 `nize-desktop-chat` token per user at any time
3. The fresh token is passed to `@ai-sdk/mcp`'s `StreamableHTTPClientTransport` as the bearer token
4. The MCP server validates the token via its existing `mcp_auth_middleware`

No cleanup needed in `onFinish` — the next chat request overwrites the token. On logout, existing `POST /auth/logout-all` revokes all tokens.

### API Change: `overwrite` Flag on Token Creation

Add an `overwrite` boolean field to `CreateMcpTokenRequest`:

- `overwrite: true` — if a token with the same `name` exists for the user, revoke it (set `revoked_at = now()`) then create the new one. Atomic (single transaction).
- `overwrite: false` (default) — if a token with the same `name` exists (and is not revoked), reject with 409 Conflict.

This prevents accidental overwrites of tokens created for external LLM clients (e.g. `claude-desktop`, `copilot-vscode`) while allowing nize-chat to rotate its token freely.

Rust changes:
- `CreateMcpTokenRequest` (TypeSpec + generated model): add `overwrite?: boolean`
- `nize_core::auth::mcp_tokens::create_mcp_token()`: accept `overwrite: bool` param; when true, `UPDATE mcp_tokens SET revoked_at = now() WHERE user_id = $1 AND name = $2 AND revoked_at IS NULL` before insert; when false, check for existing active token with same name and return error if found.

### MCP Port Discovery

The MCP server runs on `NIZE_MCP_PORT` (default 19560). nize-chat needs this value:

- **Desktop mode:** nize_desktop passes `--api-port` to nize-web sidecar; add `--mcp-port` similarly. nize-web sets `NIZE_MCP_PORT` env var for nize-chat.
- **Dev mode:** `NIZE_MCP_PORT` env var (default 19560 matches `nize_desktop_server` default).
- **Fallback:** hardcoded 19560 (same fixed default as MCP server).

## Design

### `@ai-sdk/mcp` Integration

`@ai-sdk/mcp` provides `createMCPClient()` which connects to an MCP server and exposes its tools in AI SDK format. Usage:

```ts
import { createMCPClient } from "@ai-sdk/mcp";

const mcpClient = await createMCPClient({
  transport: new StreamableHTTPClientTransport(
    new URL(`http://127.0.0.1:${mcpPort}/mcp`),
    { requestInit: { headers: { Authorization: `Bearer ${mcpToken}` } } },
  ),
});

const tools = await mcpClient.tools();

streamText({
  model,
  messages,
  tools,
  maxSteps: 10,
});

// Cleanup when done
await mcpClient.close();
```

This automatically exposes all 5 meta-tools (`discover_tools`, `get_tool_schema`, `execute_tool`, `list_tool_domains`, `browse_tool_domain`) to the LLM with their full schemas — no manual tool definitions needed.

### MCP Token Rotation in nize-chat

New module `src/mcp-client.ts`:

```ts
// Sketch — not final code
const MCP_TOKEN_NAME = "nize-desktop-chat";

async function createMcpSession(apiBaseUrl: string, cookie: string, mcpBaseUrl: string): Promise<MCPClient> {
  // Create/overwrite token — API revokes any existing nize-desktop-chat token atomically
  const res = await fetch(`${apiBaseUrl}/api/auth/mcp-tokens`, {
    method: "POST",
    headers: { "Content-Type": "application/json", cookie },
    body: JSON.stringify({ name: MCP_TOKEN_NAME, overwrite: true }),
  });
  const { token } = await res.json();

  return createMCPClient({
    transport: new StreamableHTTPClientTransport(
      new URL(mcpBaseUrl),
      { requestInit: { headers: { Authorization: `Bearer ${token}` } } },
    ),
  });
}
```

Exactly 1 `nize-desktop-chat` token per user at any time. No cleanup callback needed — next request overwrites.

### streamText Integration

`processChat()` creates an MCP client, gets tools, passes to `streamText()`, and closes the client after the stream completes:

```ts
const mcpClient = config.toolsEnabled
  ? await createMcpSession(apiBaseUrl, cookie, mcpBaseUrl)
  : null;

const tools = mcpClient ? await mcpClient.tools() : undefined;

const result = streamText({
  model,
  messages: modelMessages,
  tools,
  maxSteps: config.toolsMaxSteps,
  temperature: config.temperature,
  onFinish: async () => {
    await mcpClient?.close();
  },
});
```

No token revocation in `onFinish` — the token lives until the next chat request overwrites it.

### Config

New config keys to enable/disable tool calling and control max steps:

| Key | Category | Display Type | Default | Label | Scope |
|---|---|---|---|---|---|
| `agent.tools.enabled` | agent | boolean | `true` | Enable MCP tool calling | admin default, user override |
| `agent.tools.maxSteps` | agent | number | `10` | Max tool-call steps per message | admin default, user override |
| `agent.tools.systemPrompt` | agent | longText | *(see below)* | MCP tools system prompt | admin default, user override |

All keys follow the existing config system pattern: admin sets system-scope defaults, users can override via Settings → Agent.

When `agent.tools.enabled` is false, `processChat()` skips MCP session creation (current behavior).

### Frontend Tool Result Rendering

The AI SDK `useChat` hook already handles tool call/result message parts in the `UIMessage` format. nize-web needs components to render:

1. **Tool call indicator** — shows the tool name and a "calling..." spinner while in progress
2. **Tool result display** — renders the JSON result (collapsible, syntax-highlighted)
3. **Error display** — shows tool execution errors inline

These are standard `part.type === "tool-invocation"` parts in the AI SDK message format.

### System Prompt Injection

When tools are enabled, prepend the value of `agent.tools.systemPrompt` as a system message. Default:

> You have access to tools for discovering and executing external MCP tools. Use `discover_tools` to find relevant tools, `get_tool_schema` to understand parameters, and `execute_tool` to run them. Use `list_tool_domains` and `browse_tool_domain` to explore available categories.

This guides the LLM to use the meta-tools effectively. Admin-configurable via settings; future template system (e.g. `{{tool_names}}`, `{{domain_list}}`) is out of scope for this plan.

## Steps

### Phase 1: API — Token Overwrite Flag

- [x] 1.1 — Add `overwrite` field to `CreateMcpTokenRequest` in TypeSpec contract (`API-NIZE-auth.tsp`): `overwrite?: boolean` (default false)
- [x] 1.2 — Regenerate API code (`bun run generate:api`)
- [x] 1.3 — Update `nize_core::auth::mcp_tokens::create_mcp_token()`: accept `overwrite: bool`; when true, revoke existing active token with same name for user before insert; when false, return error if active token with same name exists
- [x] 1.4 — Update handler in `nize_api::handlers::mcp_tokens::create_mcp_token_handler()`: pass `overwrite` from request body
- [ ] 1.5 — Unit tests: overwrite=true replaces existing; overwrite=false rejects duplicate; overwrite=true with no existing token works; revoked tokens don't block creation

### Phase 2: MCP Port Plumbing

- [x] 2.1 — Pass `--mcp-port` to nize-web sidecar in `nize_desktop::start_nize_web_sidecar()` (same pattern as `--api-port`)
- [x] 2.2 — nize-web server script: accept `--mcp-port` arg, set `NIZE_MCP_PORT` env var for Next.js process
- [x] 2.3 — nize-chat `app.ts`: resolve MCP base URL from `NIZE_MCP_URL` or `NIZE_MCP_PORT` env var (fallback `http://127.0.0.1:19560`)

### Phase 3: nize-chat MCP Client

- [x] 3.1 — Add `@ai-sdk/mcp` dependency to `packages/nize-chat/package.json`
- [x] 3.2 — Create `packages/nize-chat/src/mcp-client.ts`: `createMcpSession(apiBaseUrl, cookie, mcpBaseUrl)` calls `POST /auth/mcp-tokens` with `{ name: "nize-desktop-chat", overwrite: true }`, creates `@ai-sdk/mcp` client with returned token
- [x] 3.3 — Extend `ChatConfig`: add `toolsEnabled: boolean`, `toolsMaxSteps: number`
- [x] 3.4 — Extend `fetchChatConfig()`: read `agent.tools.enabled`, `agent.tools.maxSteps`, `agent.tools.systemPrompt` from Rust API
- [x] 3.5 — Update `processChat()`: when `config.toolsEnabled`, create MCP session, get tools, pass `tools` and `maxSteps` to `streamText()`; close MCP client in `onFinish`; prepend system prompt from config
- [x] 3.6 — Unit tests: token overwrite call, MCP client creation, processChat with/without tools enabled

### Phase 4: Config Definitions

- [x] 4.1 — Migration `0010_tool_config.sql`: add `agent.tools.enabled` (boolean, default true), `agent.tools.maxSteps` (number, default 10), `agent.tools.systemPrompt` (longText, default prompt) to config definitions
- [x] 4.2 — Verify settings UI renders the new config items (boolean → toggle, number → input, longText → textarea)

### Phase 5: Frontend Rendering

- [x] 5.1 — Already existed: `packages/nize-web/components/chat/tool-renderer.tsx` renders `tool-invocation` message parts — spinner during "running", collapsible JSON result on "completed", error message on "failed"
- [x] 5.2 — Already existed: `packages/nize-web/components/chat/message-bubble.tsx` delegates to ToolRenderer for tool parts

### Phase 6: Validation

- [x] 6.1 — `cargo check` + `cargo test` (Rust — token overwrite flag): 15/15 tests pass, cargo check clean
- [x] 6.2 — `cd packages/nize-chat && bun run test` (MCP client, processChat with tools): 33/33 tests pass (5 new)
- [ ] 6.3 — `cd packages/nize-web && bun run build` (frontend components)
- [ ] 6.4 — Manual test: start app → open chat → ask LLM to use a tool → verify discover → schema → execute flow works end-to-end
- [ ] 6.5 — Manual test: disable tools via settings → verify chat works without tool calling

## Dependencies

| Dependency | Status | Blocking? |
|---|---|---|
| PLAN-024 (tool discovery & hooks) | in-progress | Yes — meta-tools and hook pipeline must work |
| PLAN-025 (stdio MCP servers) | in-progress | Partially — HTTP servers work; stdio extends coverage |
| PLAN-027 (chat backend) | in-progress | Yes — `processChat()` must exist |
| PLAN-028 (API key management) | in-progress | No — tools work independently of AI proxy |
| Registered MCP servers with indexed tools | Required | Yes — need at least one server with tools to test |

## Risks

| Risk | Mitigation |
|---|---|
| LLM uses meta-tools poorly (wrong params, loops) | System prompt guidance; `maxSteps` cap; clear tool descriptions |
| Tool execution latency degrades chat UX | Frontend shows "calling..." indicator; tool results stream incrementally |
| `@ai-sdk/mcp` API instability | Pin version; `@ai-sdk/mcp` is part of the official AI SDK ecosystem and uses standard MCP protocol |
| MCP bearer token leaked in logs | Token rotated on each request (overwrite); never logged; same security as existing MCP client tokens |
| Large tool results exceed context window | Compaction already handles tool output summarization (PLAN-027) |
| MCP connection overhead per chat request | Streamable HTTP is stateless POST-based — no persistent connection needed |
| Overwrite flag used accidentally on LLM client tokens | Default is `overwrite: false` — explicit opt-in required; existing create calls unchanged |

## Resolved Questions

1. **`maxSteps` scope:** Admin default with user override (same as other agent config keys).
2. **System prompt:** Admin-configurable via `agent.tools.systemPrompt` config key (longText, admin default with user override). Future template system (e.g. `{{tool_names}}`) out of scope.
3. **Tool call persistence:** Yes — `persistMessages()` already saves full `UIMessage[]` which includes tool call/result parts. Tool interactions are automatically persisted.
4. **Token strategy:** Fixed name `nize-desktop-chat` with `overwrite: true`. API atomically revokes existing token with same name before creating new one. Exactly 1 token per user at any time. No cleanup callbacks, no accumulation. `overwrite` flag added to `POST /auth/mcp-tokens` (default false — existing callers unaffected).

## Completion Criteria

- `streamText()` receives MCP tools (via `@ai-sdk/mcp`) when `agent.tools.enabled` is true
- LLM can discover tools, inspect schemas, and execute tools within a chat conversation
- Tool results render in the chat UI (spinner → result → collapsible JSON)
- nize-chat connects to existing MCP server via `@ai-sdk/mcp` with bearer token auth
- No new Rust REST endpoints needed — reuses existing MCP server
- Single `nize-desktop-chat` MCP token per user, rotated via `overwrite: true` on each chat request
- Config keys (admin default, user override) control enablement, max steps, and system prompt
- Works end-to-end: user asks "find me a tool for X" → LLM discovers → executes → shows result
